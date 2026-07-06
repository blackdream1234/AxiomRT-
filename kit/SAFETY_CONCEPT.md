# AxiomRT v1.0 — Safety Concept

Evaluation-stage safety concept. Not a safety case; not a certification
artifact (see LIMITATIONS.md).

## 1. Safety Goal

Keep a critical task running and the kernel stable in the presence of a
misbehaving or malicious task: **a fault in one task must not propagate
to the kernel or to unrelated tasks.**

## 2. Hazards Addressed (evaluation scope)

| Hazard | AxiomRT mechanism | Evidence |
|---|---|---|
| A task reads/writes/executes memory it must not | Sv39 MMU isolation, W^X, kernel U=0 | memory_isolation_qemu_test |
| A runaway task starves others / freezes the CPU | fixed-priority preemption + watchdog | timer_preemption_qemu_test, watchdog_qemu_test |
| A task performs an unauthorized operation | capability-based access control (deny-by-default) | capability_qemu_test |
| A task fault crashes the kernel | fault containment (task → Faulted, kernel continues) | full_fault_containment_demo |
| A fault goes unnoticed / unhandled | structured fault events to a supervisor + logger | supervisor_qemu_test |

## 3. Safety Mechanisms

1. **Memory isolation (MMU).** Each task runs under its own Sv39 page
   table; kernel pages carry no U bit; user pages are W^X. A forbidden
   access takes a hardware page fault that is contained
   (docs/12_MMU_SV39.md).
2. **Deterministic scheduling.** Fixed-priority preemptive scheduling
   with explicit round-robin tie-breaking; a higher-priority ready task
   always runs (docs/09_SCHEDULER_MODEL.md, proofs/coq/SchedulerPriority.v).
3. **Temporal protection.** A periodic timer preempts the running task;
   a task that stops checking in is caught by the watchdog and moved to
   Faulted (docs/15, docs/16).
4. **Fault containment.** A user fault is converted into a structured
   event; the faulting task is stopped; the kernel and other tasks
   continue (docs/06_FAULT_MODEL.md, docs/10_USER_MODE.md).
5. **Controlled recovery.** A trusted user-space supervisor receives
   fault events over capability-checked IPC and applies a recovery
   decision; the kernel records the applied policy (docs/19).

## 4. Freedom-from-Interference Argument (evaluation-level)

* **Spatial:** distinct address spaces + single-frame ownership give
  spatial separation; the MMU enforces it for the tested accesses.
* **Temporal:** preemption bounds any task's CPU share; the watchdog
  bounds detection latency for a stuck task.
* **Communication:** the only inter-task channel is capability-checked,
  bounded, copy-based IPC — no shared memory, no covert authority.

This argument is demonstrated by the QEMU tests and modeled by the Coq
starters; it is **not** a completed safety case (refinement obligations
and hardware validation remain — LIMITATIONS.md).

## 5. Residual Risk (evaluation scope)

* Untested memory-access patterns (only three negative cases proven on
  target).
* Model↔implementation refinement not discharged.
* No real-hardware validation; single-hart only.
* Recovery limited to Kill on target.
