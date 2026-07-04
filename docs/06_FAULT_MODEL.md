# AxiomRT Fault Model

Document ID: AXIOM-DOC-007
Status: Approved for Phase 0

Faults are first-class structured events. Every fault type below has fully
defined behavior: source, severity, kernel action, supervisor notification,
recovery options, and logging fields. No fault has undefined behavior.

## Severity Levels

* **Info** — recorded, no containment needed.
* **Error** — user task misbehavior; contained; supervisor decides recovery.
* **Critical** — user task misbehavior that threatens timing or repeated
  abuse; contained; supervisor decides with escalation bias.
* **Fatal** — kernel integrity can no longer be trusted; controlled halt.

## Recovery Options (decided by supervisor unless stated otherwise)

* **Kill** — terminate the task permanently; peers unblocked with error.
* **Restart** — kill, then recreate the task from its boot image.
* **Suspend** — freeze the task; may be resumed by supervisor.
* **Quarantine** — suspend permanently and mark untrusted; resources held
  for analysis, never reused this boot.
* **Escalate** — supervisor forwards to external policy (v0.1: log +
  default action).
* **KernelPanic** — controlled halt of the whole system; only for Fatal
  faults; never selectable by the supervisor for user faults.

Default policy: if the supervisor is unavailable (not yet started, faulted,
or queue full), the kernel applies the **bold default** listed per fault
below, and records the delivery failure.

Common logging fields for every fault event: `event_id`, `timestamp`,
`fault_type`, `severity`, `thread_id`, `task_name`, `pc` (program counter),
`kernel_phase`, `policy_result` (filled after acknowledgement).

---

## IllegalSyscall

* **Source:** user thread invokes an unknown or forbidden syscall number,
  or a syscall from a context where it is not allowed.
* **Severity:** Error.
* **Kernel action:** reject before any state change; return
  `ERR_INVALID_SYSCALL`; mark thread Faulted; create FaultEvent.
* **Supervisor notification:** yes, via fault channel.
* **Recovery options:** **Kill**, Restart, Suspend, Quarantine, Escalate.
* **Logging fields (additional):** `syscall_number`, `arg0..arg5` (raw,
  as data).

## InvalidCapability

* **Source:** capability lookup failure during a syscall: bad index, empty
  or revoked slot, wrong object type, or insufficient rights.
* **Severity:** Error (Critical if repeated ≥ N times within a window —
  probing pattern; N defined in supervisor policy).
* **Kernel action:** deny access before touching the object; return the
  specific error code; emit CAP_DENIED / IPC_DENIED event; create
  FaultEvent; thread marked Faulted on Critical repetition, otherwise
  continues with the error result.
* **Supervisor notification:** yes (batched allowed for Error level).
* **Recovery options:** **Escalate** (log + continue) at Error; Kill,
  Restart, Suspend, Quarantine at Critical.
* **Logging fields:** `cap_index`, `required_rights`, `held_rights`,
  `object_type_expected`, `object_type_actual`, `syscall_number`.

## PageFault

* **Source:** user access to unmapped address or permission violation
  (read/write/execute) detected by MMU.
* **Severity:** Error (user); **Fatal** if the faulting context is the
  kernel itself.
* **Kernel action:** user case — thread moved to Faulted, never resumes at
  the faulting instruction, FaultEvent created, PAGE_FAULT event emitted.
  Kernel case — KernelInvariantViolation path (below).
* **Supervisor notification:** yes.
* **Recovery options:** **Kill**, Restart, Suspend, Quarantine, Escalate.
* **Logging fields:** `fault_addr`, `access_type` (R/W/X), `pc`,
  `mapped_regions_summary`.

## IllegalInstruction

* **Source:** user thread executes an invalid, privileged, or disabled
  instruction (e.g., attempts a CSR access from user mode).
* **Severity:** Error.
* **Kernel action:** thread moved to Faulted; FaultEvent created; no
  emulation of the instruction is ever attempted.
* **Supervisor notification:** yes.
* **Recovery options:** **Kill**, Restart, Suspend, Quarantine, Escalate.
* **Logging fields:** `pc`, `instruction_bits`, `privilege_at_fault`.

## WatchdogTimeout

* **Source:** a supervised task fails to signal liveness within its
  watchdog window (e.g., stuck in an infinite loop, CPU exhaustion
  attempt), detected on timer tick.
* **Severity:** Critical.
* **Kernel action:** the task is forcibly descheduled (preemption already
  bounds its CPU share); thread moved to Faulted; FaultEvent created;
  WATCHDOG_TIMEOUT event emitted. Critical tasks keep running — a watchdog
  timeout of task X never delays higher-priority ready tasks.
* **Supervisor notification:** yes, priority delivery.
* **Recovery options:** **Restart**, Kill, Suspend, Quarantine, Escalate.
* **Logging fields:** `watchdog_window_ms`, `last_heartbeat_timestamp`,
  `missed_count`.

## DeadlineMiss

* **Source:** a periodic task exceeds its declared deadline (detected by
  timer bookkeeping against its SchedulingContext).
* **Severity:** Error (Critical for tasks marked deadline-critical).
* **Kernel action:** record and emit DEADLINE_MISSED event; create
  FaultEvent; the task continues unless policy says otherwise (a miss is
  a timing fault, not necessarily corruption).
* **Supervisor notification:** yes.
* **Recovery options:** **Escalate** (log + continue) at Error; Restart,
  Suspend, Kill at Critical.
* **Logging fields:** `deadline_us`, `actual_us`, `overrun_us`,
  `consecutive_misses`.

## IPCViolation

* **Source:** IPC abuse beyond capability failure: invalid buffer range
  passed to send/recv/reply, oversized message, reply without pending
  rendezvous (repeated), rendezvous protocol abuse.
* **Severity:** Error.
* **Kernel action:** the IPC operation is rejected with no partial
  transfer (the peer observes nothing or a clean `ERR_PEER_KILLED`);
  IPC_DENIED event emitted; FaultEvent created; thread marked Faulted for
  buffer-range violations (memory-adjacent abuse).
* **Supervisor notification:** yes.
* **Recovery options:** **Kill**, Restart, Suspend, Quarantine, Escalate.
* **Logging fields:** `endpoint_id`, `operation` (send/recv/reply),
  `msg_len`, `buffer_addr`, `violation_kind`.

## KernelInvariantViolation

* **Source:** the kernel detects that its own state is inconsistent:
  kernel-mode page fault, corrupted object state, impossible state
  transition, failed internal assertion, double frame ownership.
* **Severity:** Fatal.
* **Kernel action:** **KernelPanic** — controlled halt: (1) disable
  interrupts; (2) emit a final structured panic record over serial with all
  available context; (3) halt the hart (no reboot loop, no silent restart,
  no continuation). User recovery options do not apply: continuing on a
  broken kernel invariant would be unsafe.
* **Supervisor notification:** attempted best-effort in the panic record
  (the supervisor cannot act — the system is halting), so external systems
  reading the serial log are the real consumers.
* **Recovery options:** KernelPanic only.
* **Logging fields:** `invariant_id`, `subsystem`, `detail`, full register
  snapshot, `kernel_phase`.

---

## Fault Handling Invariants

1. A user-space fault never crashes the kernel (Fatal is reserved for
   kernel self-detected inconsistency).
2. Every fault produces exactly one FaultEvent; events are immutable.
3. A Faulted thread is never scheduled until an explicit recovery decision
   re-enables it (Restart creates a fresh thread; Faulted state itself is
   terminal for the original thread).
4. Fault handling never allocates memory dynamically (static event pool;
   on pool exhaustion the oldest unacknowledged Error-level event is
   overwritten and an overflow marker is recorded — Critical/Fatal events
   are never dropped).
5. The supervisor's decision is applied through the same capability-checked
   kernel mechanisms as any other operation.
