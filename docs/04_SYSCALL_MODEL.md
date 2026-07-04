# AxiomRT Syscall Model

Document ID: AXIOM-DOC-005
Status: Approved for Phase 0

## General Rules

* Syscalls are the only user→kernel service interface. They enter through
  the trap path (RISC-V `ecall` from user mode).
* Every syscall validates **all** arguments before any state is touched.
* No syscall operates on raw object pointers. All object references are
  capability indexes into the calling task's capability table.
* Every syscall returns an explicit result code. There are no silent
  failures.
* An invalid syscall never crashes the kernel; it fails in a controlled way
  and may raise a fault against the caller.

ABI (v0.1): syscall number in `a7`; arguments in `a0..a5`; result code in
`a0`; secondary return value in `a1`. Any ABI change requires updating this
document first.

Syscall numbers (a7), fixed by AXIOM-TRAP-003:

```text
1 sys_yield   2 sys_exit   3 sys_send      4 sys_recv
5 sys_reply   6 sys_cap_query   7 sys_fault_ack
```

Result codes (a0, signed): `OK`=0, `ERR_INVALID_SYSCALL`=-1,
`ERR_INVALID_CAP`=-2, `ERR_INSUFFICIENT_RIGHTS`=-3,
`ERR_WRONG_OBJECT_TYPE`=-4, `ERR_INVALID_ARG`=-5, `ERR_MSG_TOO_LARGE`=-6,
`ERR_PEER_KILLED`=-7, `ERR_NO_PENDING_FAULT`=-8, plus the transitional
`ERR_NOT_IMPLEMENTED`=-9 returned by the Phase 3 stub layer until the
implementing phase of each syscall lands.

---

## sys_yield

* **Purpose:** Voluntarily give up the CPU so the scheduler can select the
  highest-priority ready thread (round-robin effect among equal priority).
* **Arguments:** none.
* **Required capability:** none (yielding affects only the caller).
* **Success result:** `OK` when the thread is scheduled again.
* **Failure result:** none (yield cannot fail).
* **Validation rule:** syscall number must be valid; extra argument
  registers are ignored, never interpreted.
* **Fault behavior:** none.
* **Security rule:** Yield must not allow priority inversion abuse: it never
  changes priorities, only requeues the caller at the tail of its own
  priority level.

## sys_exit

* **Purpose:** Terminate the calling thread permanently.
* **Arguments:** `a0` = exit code (informational, recorded in event log).
* **Required capability:** none (a thread may always end itself).
* **Success result:** does not return; thread state becomes Killed;
  TASK_EXITED event is emitted.
* **Failure result:** none (exit cannot fail).
* **Validation rule:** exit code is recorded as-is; it is data, never
  interpreted.
* **Fault behavior:** none. If the exiting thread is blocked in an IPC
  rendezvous bookkeeping error, that is a kernel invariant violation.
* **Security rule:** Exit releases the thread's scheduling slot; its
  capabilities become unusable; its endpoint peers are unblocked with
  `ERR_PEER_KILLED`. No authority survives thread death.

## sys_send

* **Purpose:** Synchronously send one bounded message to an endpoint;
  blocks until a receiver takes it.
* **Arguments:** `a0` = capability index of endpoint; `a1` = message buffer
  address (caller address space); `a2` = message length in bytes.
* **Required capability:** Endpoint capability with **Send** right.
* **Success result:** `OK` after the message is copied to the receiver.
* **Failure result:** `ERR_INVALID_CAP` (bad index / revoked),
  `ERR_WRONG_OBJECT_TYPE` (not an endpoint), `ERR_INSUFFICIENT_RIGHTS`
  (no Send right), `ERR_INVALID_ARG` (buffer range invalid),
  `ERR_MSG_TOO_LARGE` (length > MSG_MAX_BYTES), `ERR_PEER_KILLED`
  (receiver died while sender was blocked).
* **Validation rule:** in order: (1) capability index in table range and
  Held; (2) object type == Endpoint; (3) rights include Send; (4) length ≤
  MSG_MAX_BYTES; (5) [buffer, buffer+length) entirely inside caller's mapped
  user memory with read permission. All checks pass before any copy starts.
* **Fault behavior:** Failing checks (1)–(3) raises an InvalidCapability
  fault (CAP_DENIED / IPC_DENIED event). Failing (4)–(5) raises an
  IPCViolation fault. The endpoint and receiver are never affected by a
  failed send.
* **Security rule:** Message content is data only; no capability material
  crosses in message bytes. Send never reveals whether the endpoint exists
  when the capability check fails (single uniform error surface).

## sys_recv

* **Purpose:** Synchronously receive one message from an endpoint; blocks
  until a sender arrives.
* **Arguments:** `a0` = capability index of endpoint; `a1` = receive buffer
  address; `a2` = receive buffer capacity in bytes.
* **Required capability:** Endpoint capability with **Receive** right.
* **Success result:** `OK`; `a1` = received message length; sender identity
  delivered in message header (kernel-written, unforgeable).
* **Failure result:** `ERR_INVALID_CAP`, `ERR_WRONG_OBJECT_TYPE`,
  `ERR_INSUFFICIENT_RIGHTS`, `ERR_INVALID_ARG` (buffer range invalid),
  `ERR_MSG_TOO_LARGE` (incoming message larger than capacity),
  `ERR_PEER_KILLED`.
* **Validation rule:** same order as sys_send with Receive right; buffer
  must be writable user memory of the caller; capacity checked against the
  actual incoming message before copy.
* **Fault behavior:** capability failures raise InvalidCapability
  (IPC_DENIED); invalid buffer raises IPCViolation. A failed receive never
  consumes the sender's message.
* **Security rule:** The sender identity in the header is written by the
  kernel and cannot be spoofed. Receive grants no authority over the sender.

## sys_reply

* **Purpose:** Complete a received rendezvous by sending a bounded reply to
  the blocked sender (server side of call/reply).
* **Arguments:** `a0` = capability index of endpoint; `a1` = reply buffer
  address; `a2` = reply length.
* **Required capability:** Endpoint capability with **Receive** right (the
  replier is the service that received on this endpoint).
* **Success result:** `OK`; the original sender is unblocked with the reply.
* **Failure result:** `ERR_INVALID_CAP`, `ERR_WRONG_OBJECT_TYPE`,
  `ERR_INSUFFICIENT_RIGHTS`, `ERR_INVALID_ARG` (no pending reply-waiting
  sender, or bad buffer), `ERR_MSG_TOO_LARGE`, `ERR_PEER_KILLED`.
* **Validation rule:** capability checks as above; a reply is valid only if
  this thread has a pending, received-but-unreplied rendezvous on that
  endpoint; buffer range must be readable caller memory; length ≤
  MSG_MAX_BYTES.
* **Fault behavior:** capability failures raise InvalidCapability; replying
  with no pending rendezvous is `ERR_INVALID_ARG` (error, not fault, unless
  repeated as abuse — policy belongs to supervisor).
* **Security rule:** Reply can target only the thread that is actually
  blocked on this rendezvous; the kernel tracks the pairing, user code
  cannot redirect a reply.

## sys_cap_query

* **Purpose:** Let a task inspect one of its own capability slots (object
  type and rights) for defensive programming and testing.
* **Arguments:** `a0` = capability index.
* **Success result:** `OK`; `a1` = packed {object type, rights bits}.
* **Required capability:** the queried capability itself (a task may only
  query its own table; no cross-task query exists).
* **Failure result:** `ERR_INVALID_CAP` (index out of range or slot empty
  or revoked).
* **Validation rule:** index within the caller's capability table bounds;
  slot state is Held.
* **Fault behavior:** none. Querying an empty slot is an error, not a
  fault (query is the sanctioned way to probe safely).
* **Security rule:** Query reveals only the caller's own authority. It never
  reveals object IDs of other tasks, kernel addresses, or global state.

## sys_fault_ack

* **Purpose:** Supervisor acknowledges a delivered FaultEvent and states the
  applied recovery decision, closing the fault-handling loop.
* **Arguments:** `a0` = capability index of the fault channel endpoint;
  `a1` = fault event ID; `a2` = recovery decision code (Kill, Restart,
  Suspend, Quarantine, Escalate).
* **Required capability:** Fault channel capability with **Control** right
  (held only by the supervisor task in v0.1).
* **Success result:** `OK`; event marked Acknowledged; RECOVERY_APPLIED
  event emitted with the decision.
* **Failure result:** `ERR_INVALID_CAP`, `ERR_INSUFFICIENT_RIGHTS`,
  `ERR_WRONG_OBJECT_TYPE`, `ERR_NO_PENDING_FAULT` (unknown or already
  acknowledged event ID), `ERR_INVALID_ARG` (unknown decision code).
* **Validation rule:** capability checks first; event ID must reference a
  Delivered, unacknowledged FaultEvent; decision code must be in the defined
  recovery set for that fault type (docs/06_FAULT_MODEL.md).
* **Fault behavior:** a non-supervisor task calling sys_fault_ack raises an
  InvalidCapability fault (CAP_DENIED) — this is itself an attack signal.
* **Security rule:** Only Control-right holders can acknowledge faults.
  Acknowledgement applies policy through kernel mechanisms; it cannot bypass
  capability checks or touch memory of the faulted task directly.

---

## Forbidden Syscalls v0.1

The following must not exist in v0.1, in any form:

```text
open
file read
file write
socket
fork
exec
shared mmap
```

Rationale: no filesystem, no network, no dynamic task creation, and no
shared memory exist in v0.1 (docs/01_SCOPE_AND_NON_GOALS.md). An unknown or
forbidden syscall number returns `ERR_INVALID_SYSCALL` and raises an
IllegalSyscall fault against the caller.
