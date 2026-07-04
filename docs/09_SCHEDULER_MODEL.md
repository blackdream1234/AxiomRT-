# AxiomRT Scheduler Model

Document ID: created by AXIOM-SCHED-001 (Phase 6)
Requirement reference: docs/02_KERNEL_BLUEPRINT.md §10 (scheduling
principle), Project Description §15.

(Naming note: the file number 09 collides with 09_BUILD_AND_BOOT.md; both
names are fixed verbatim by the task pack.)

## 1. Model

Fixed-priority preemptive scheduling. Phase 6 implements the *selection
model* (`kernel/src/sched/`): pure, deterministic, host-tested logic.
Timer-driven preemption and context switching integrate in later phases.

Normative rules (Project Description §15):

* A high-priority ready task must run before a low-priority ready task.
* Blocked tasks cannot be selected.
* Killed tasks cannot be selected.
* Faulted tasks cannot continue unless recovered (Restart = new thread).
* Deterministic tie-breaking must exist.
* A low-priority faulty task must not freeze the system.

## 2. Priorities (`sched/priority.rs`)

* 8 fixed levels, 0..=7; higher value = more urgent.
* `Priority` is a validated type: out-of-range values cannot be
  constructed (docs/03_KERNEL_OBJECTS.md §9).
* Priorities are static in v0.1 (assigned at task creation); a task can
  never raise its own priority.

## 3. Ready Queue (`sched/queue.rs`)

* One statically sized FIFO ring per priority level (capacity 16, no
  heap).
* **Tie-breaking rule:** within one level, FIFO order — among equal
  priorities, the thread that became ready earliest is selected first.
  Re-enqueueing after yield goes to the tail, giving round-robin among
  equals.
* A thread is queued at most once; double enqueue is an explicit error.
* Removal (block/kill/fault/suspend) preserves the order of the
  remaining entries.

## 4. Scheduler (`sched/mod.rs`)

`FixedPriorityScheduler::select_next(is_ready)` pops the highest
non-empty level, FIFO within the level. Selection consults the thread
state machine through the `is_ready` predicate and **discards** stale
entries whose state is no longer Ready.

Authority rule: the ready queue is an optimization; the thread state
machine (docs/03_KERNEL_OBJECTS.md §2) is the authority. A Killed,
Blocked, Faulted, or Suspended thread is never returned by
`select_next`, even if a bookkeeping removal was missed.

Invariants (verification targets, proofs/coq/SchedulerPriority.v in
Phase 12):

* **SCHED-P1 (priority):** if a ready thread of priority p exists, no
  thread of priority < p is selected.
* **SCHED-P2 (liveness of exclusion):** a thread not in state Ready is
  never selected.
* **SCHED-P3 (determinism):** selection is a deterministic function of
  the queue history — same enqueue/dequeue sequence, same selection.

## 5. Phase 6 Boundary

* No timer interrupt, no preemption trigger, no context switch (the
  selected thread is not actually dispatched yet).
* No budgets, deadlines, or mixed criticality (v0.2+,
  docs/01_SCOPE_AND_NON_GOALS.md §3).
* Test obligations: see docs/14_TEST_STRATEGY.md (AXIOM-SCHED-002).
