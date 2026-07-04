# AxiomRT Kernel Objects

Document ID: AXIOM-DOC-004
Status: Approved for Phase 0

Every kernel object is defined here with: purpose, owner, lifecycle, valid
states, allowed operations, invalid operations, failure behavior, and
security impact. No object has vague responsibility. All object access from
user space goes through capability lookup — never through raw IDs or
pointers.

Common rules for all objects:

* Every object has a unique kernel-assigned ID (never reused within a boot).
* Every object has exactly one owner at any time.
* Object memory comes from static kernel pools sized at boot (no heap).
* An operation not listed as allowed is invalid by definition.

---

## 1. KernelObject

* **Purpose:** Common base concept for all kernel objects: identity (ID),
  type tag, owner reference, and lifecycle state. It exists so that
  capability lookup, auditing, and fault reporting are uniform.
* **Owner:** The kernel itself; concrete ownership is defined by the derived
  object.
* **Lifecycle:** Created → Active → Retired. Retired IDs are not reused
  within a boot session.
* **Valid states:** Created, Active, Retired.
* **Allowed operations:** identify (read ID/type), audit (report in events),
  capability lookup resolution.
* **Invalid operations:** direct user access, type confusion (using an
  object as a different type), ID forgery.
* **Failure behavior:** Any type/ID mismatch during lookup is rejected and
  raises an InvalidCapability fault; the object is untouched.
* **Security impact:** Uniform identity prevents confused-deputy access by
  guessed IDs; type tags prevent type-confusion attacks.

## 2. Thread

* **Purpose:** Represents one execution context (registers, privilege,
  scheduling state) of a user task.
* **Owner:** The task's AddressSpace (v0.1: one thread per task); managed by
  the kernel; controllable by a holder of a Control capability (supervisor).
* **Lifecycle:** Created → Ready → (Running ↔ Ready ↔ Blocked) → Faulted /
  Killed / Suspended → Retired.
* **Valid states:** Ready, Running, Blocked, Faulted, Killed, Suspended.
* **Allowed operations:** create (kernel, boot-time in v0.1), start, yield
  (sys_yield), exit (sys_exit), block on IPC, unblock, mark Faulted,
  suspend/resume (Control right), kill (Control right).
* **Invalid operations:** selecting a Killed/Blocked/Faulted thread to run,
  modifying another thread's registers without Control right, resurrecting a
  Killed thread, running in kernel mode.
* **Failure behavior:** A fault in the thread moves it to Faulted, produces a
  FaultEvent, and never propagates into the kernel. Invalid state
  transitions are kernel invariant violations (controlled panic).
* **Security impact:** The thread is the subject of all access control;
  its capability set defines its entire authority.

## 3. AddressSpace

* **Purpose:** Defines the complete virtual memory view of one task; the
  memory isolation unit.
* **Owner:** The kernel; associated one-to-one with a task in v0.1.
* **Lifecycle:** Created (boot) → Active → Torn down (when task is killed) →
  Retired.
* **Valid states:** Created, Active, TearingDown, Retired.
* **Allowed operations:** map frame (kernel, with Map authority), unmap
  frame, resolve address (kernel-internal), destroy on task kill.
* **Invalid operations:** mapping kernel memory as user-accessible, mapping
  a frame owned by another task (no shared memory in v0.1), user-initiated
  arbitrary mapping.
* **Failure behavior:** Invalid mapping requests are rejected with an error
  and raise a fault; the address space is unchanged (mapping is atomic:
  applied fully or not at all).
* **Security impact:** Primary memory isolation boundary; a defect here
  violates the core guarantee "no user task can access kernel memory."

## 4. PhysicalFrame

* **Purpose:** Represents one physical memory frame (allocation, ownership,
  and mapping tracking unit).
* **Owner:** Exactly one of: Kernel, one AddressSpace, or Free pool.
* **Lifecycle:** Free → Allocated (to kernel or one address space) → Mapped
  → Unmapped → Freed (returned to pool, scrubbed).
* **Valid states:** Free, Allocated, Mapped, Quarantined (after fault, if
  policy requires), Retired.
* **Allowed operations:** allocate (kernel, boot/setup only in v0.1), map
  into owning address space, unmap, scrub, free.
* **Invalid operations:** mapping one frame into two address spaces (no
  shared memory in v0.1), freeing a mapped frame, user access to frame
  metadata.
* **Failure behavior:** Double-map or double-free attempts are kernel
  invariant violations (controlled panic in v0.1 — they indicate a kernel
  bug, not a user fault).
* **Security impact:** Frame ownership uniqueness is what makes "no task can
  access another task's memory" provable.

## 5. PageTable

* **Purpose:** Hardware-facing translation structure (RISC-V Sv39) realizing
  one AddressSpace's mappings and permissions.
* **Owner:** Its AddressSpace.
* **Lifecycle:** Created with address space → Populated by map/unmap →
  Destroyed with address space.
* **Valid states:** Inactive (not loaded), Active (loaded in satp), Retired.
* **Allowed operations:** insert mapping (kernel only), remove mapping,
  activate on context switch, walk (kernel only).
* **Invalid operations:** user-writable page table memory, entries marking
  kernel memory user-accessible, entries pointing to frames not owned by the
  address space.
* **Failure behavior:** Any attempt to insert a forbidden entry is rejected
  before the hardware sees it; inconsistency between PageTable and
  AddressSpace model is a kernel invariant violation.
* **Security impact:** The hardware enforcement point of memory isolation;
  its contents must always refine the AddressSpace model.

## 6. Endpoint

* **Purpose:** Rendezvous object for synchronous IPC between exactly one
  sender and one receiver at a time.
* **Owner:** The task that the endpoint was created for (service side);
  usable by holders of Send/Receive capabilities.
* **Lifecycle:** Created (boot) → Idle → (SenderWaiting | ReceiverWaiting →
  Transfer) → Idle → Retired.
* **Valid states:** Idle, SenderWaiting, ReceiverWaiting, Transferring.
* **Allowed operations:** send (requires Send right; blocks until receiver),
  receive (requires Receive right; blocks until sender), reply (sys_reply on
  a pending rendezvous).
* **Invalid operations:** send without Send right, receive without Receive
  right, buffering more than one in-flight message, broadcast.
* **Failure behavior:** Capability failures raise InvalidCapability faults
  and leave the endpoint state unchanged. If a blocked party is killed, the
  rendezvous is cancelled and the peer is unblocked with an error result.
* **Security impact:** The only lawful communication channel between tasks;
  denies covert data flow paths that bypass capability rights.

## 7. Message

* **Purpose:** Bounded, copy-based data unit transferred through an
  Endpoint.
* **Owner:** The sending thread until the copy completes; then the receiving
  thread's buffer holds the data.
* **Lifecycle:** Composed (in sender registers/buffer) → Validated → Copied
  by kernel → Delivered → Consumed.
* **Valid states:** Composed, Validated, Delivered.
* **Allowed operations:** compose (user), validate length and source range
  (kernel), copy (kernel), deliver.
* **Invalid operations:** messages exceeding the fixed maximum size,
  passing pointers as authority (pointers are data, never authority),
  partial/streaming transfer.
* **Failure behavior:** Oversized or invalid-range messages are rejected
  before any copy; the receiver observes nothing. A fault mid-copy delivers
  nothing (no partial messages visible to the receiver).
* **Security impact:** Bounded copying prevents kernel buffer abuse and
  keeps information flow explicit and auditable.

## 8. Capability

* **Purpose:** Explicit, unforgeable authority token: (object reference,
  object type, rights set). The only way user code reaches any protected
  object.
* **Owner:** Stored in a task's capability table in kernel memory; the task
  is the holder, the kernel is the custodian.
* **Lifecycle:** Minted (kernel, boot-time in v0.1) → Held → (Granted to
  another task, if Grant right, v0.2+) → Revoked / Deleted.
* **Valid states:** Held, Revoked.
* **Allowed operations:** invoke (use in syscall; rights checked), query
  (sys_cap_query), grant (only with Grant right; deferred beyond v0.1
  boot-static distribution), revoke (kernel/Control).
* **Invalid operations:** forging (user memory never contains capability
  bits), amplifying rights, using after revocation, transferring without
  Grant right.
* **Failure behavior:** Any lookup failure (bad index, wrong type,
  insufficient rights, revoked) returns a distinct error and raises an
  InvalidCapability fault; the target object is never touched.
* **Security impact:** The entire security model. Least privilege holds iff
  capability checks are complete and unbypassable.

## 9. SchedulingContext

* **Purpose:** Scheduling parameters of a thread: fixed priority and
  (v0.2+) execution budget for temporal isolation.
* **Owner:** Its Thread; modifiable only via Control authority.
* **Lifecycle:** Created with thread → Active → Retired with thread.
* **Valid states:** Active, Retired.
* **Allowed operations:** read by scheduler, priority assignment at creation,
  priority change via Control right (v0.2+; static in v0.1).
* **Invalid operations:** a task raising its own priority, priority values
  outside the defined range, hidden scheduler state.
* **Failure behavior:** Invalid parameter writes are rejected; scheduler
  invariants (e.g., a Running thread has the highest ready priority) are
  checked in debug builds; violation is a kernel invariant violation.
* **Security impact:** Prevents priority forgery and starvation attacks;
  basis of the deterministic scheduling guarantee.

## 10. Timer

* **Purpose:** Kernel time source: drives preemption ticks, watchdog
  deadlines, and event timestamps. Backed by the RISC-V timer via SBI.
* **Owner:** The kernel. Watchdog configuration readable by the supervisor
  with a capability (v0.2+; static in v0.1).
* **Lifecycle:** Initialized at boot → Armed → Fired → Re-armed (periodic).
* **Valid states:** Disarmed, Armed, Fired.
* **Allowed operations:** arm next tick (kernel), read current time
  (kernel), deliver timer interrupt to scheduler and watchdog logic.
* **Invalid operations:** user-visible raw timer writes, disabling the
  preemption tick from user space, missing-deadline suppression.
* **Failure behavior:** A missed or late tick beyond tolerance raises a
  WatchdogTimeout/DeadlineMiss fault path; timer hardware failure is a
  kernel invariant violation (halt safely).
* **Security impact:** Guarantees that a looping user task cannot keep the
  CPU forever (preemption) and that hangs are detected (watchdog).

## 11. FaultEvent

* **Purpose:** Structured record of a fault: what happened, to whom, why,
  and with what severity. The unit of fault containment and evidence.
* **Owner:** Created and owned by the kernel; delivered to the supervisor
  task through its fault endpoint; a copy goes to runtime monitoring.
* **Lifecycle:** Created at fault time → Queued for supervisor → Delivered →
  Acknowledged (sys_fault_ack) → Archived (event log) → Retired.
* **Valid states:** Created, Queued, Delivered, Acknowledged.
* **Allowed operations:** create (kernel only), deliver to supervisor,
  acknowledge (supervisor, sys_fault_ack), export to monitor stream.
* **Invalid operations:** creation from user space, modification after
  creation (events are immutable), suppression or dropping without record.
* **Failure behavior:** If the supervisor cannot receive (queue full,
  supervisor faulted), the kernel applies the documented default policy for
  the fault type (docs/06_FAULT_MODEL.md) and records the delivery failure.
* **Security impact:** Guarantees faults are visible and attributable —
  the basis of controlled recovery and of the safety evidence trail.

---

## Object Relationship Summary

```text
Thread ──1:1── SchedulingContext
Thread ──n:1── AddressSpace ──1:1── PageTable ──*── PhysicalFrame
Thread ──holds──> Capability ──refers──> {Thread, Endpoint, AddressSpace,
                                          PhysicalFrame, Timer,
                                          SchedulingContext, FaultEvent chan}
Endpoint ──transfers──> Message (bounded copy)
Kernel ──creates──> FaultEvent ──delivered──> supervisor Thread
```

In v0.1 all objects are created at boot from static pools; there is no
dynamic object creation from user space.

## Implementation Notes (kept current per phase)

* **Thread (Phase 5, AXIOM-THREAD-001):** realized in
  `kernel/src/thread/` — `ThreadId` (id.rs), `ThreadState` and the
  complete legal transition relation (state.rs), `Thread` skeleton
  (mod.rs). The relation makes Killed terminal, keeps Faulted out of
  execution permanently (Restart creates a fresh thread,
  docs/06_FAULT_MODEL.md invariant 3), and admits Running only from
  Ready. Invalid transitions return an explicit `IllegalTransition`
  error and leave the thread unchanged. No context switching in this
  task.
* **Thread context (Phase 5, AXIOM-THREAD-002):**
  `kernel/src/arch/riscv64/context.rs` defines `ArchContext`, the
  `#[repr(C)]` callee-saved register set (ra, sp, s0..s11 — fixed
  offsets for the future switch assembly). Documented assumptions:
  switches happen at call boundaries (caller-saved registers are dead
  by the ABI); full interrupted-thread state lives in the trap frame,
  a distinct structure; no FP state (FP off in kernel); satp joins the
  context only when the MMU is activated. `kernel/src/thread/context.rs`
  wraps it arch-independently and rejects contexts with a null resume
  address or null stack at construction. No context switch assembly
  exists in Phase 5.
* **User image (Phase 7, AXIOM-USER-001):** `kernel/src/user/image.rs`
  defines `UserImage`: entry point, downward-growing stack region
  (top + size), and owning `AddressSpaceId` (1:1 with the task in
  v0.1). Construction is validated: the entry and the whole stack
  region must lie inside the user virtual window
  (docs/05_MEMORY_MODEL.md §3/§11) and the stack must be page-aligned
  and at least one page — a descriptor violating these cannot exist.
  Images are static boot-time descriptors in v0.1. No user-mode jump
  in this task.
