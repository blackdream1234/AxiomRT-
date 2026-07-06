# AxiomRT v0.3 Report — On-Target Multi-Task Dispatch

Requirement reference: Full Completion Mode §12 (Stage 2),
docs/13_DISPATCH.md.

## 1. Goal Achieved

Two U-mode tasks run on target, each in its own Sv39 address space, and
cooperatively switch via `sys_yield` / `sys_exit`. Context save/restore
uses the trap frame; each switch also switches `satp`.

**No certification claim. No production-readiness claim.**

## 2. Verified Facts (evidence in this directory)

| Fact | Evidence |
|---|---|
| Two tasks start and alternate | `two_task_demo.log` |
| Cooperative switch both directions | `two_task_demo.log` (`SCHED selected=task_a` and `task_b`) |
| Both tasks exit; dispatcher idles cleanly | `two_task_demo.log` (`SCHED idle=all_tasks_done`) |
| Killed task never reselected | `two_task_demo.log` (only task_b runs after task_a exits) |
| Automated QEMU test passes | `two_task_test.log` |

## 3. Boundary and Limitations (explicit)

* Switching is **cooperative only** — a task keeps the CPU until it
  yields or exits. Timer preemption is v0.4 (next stage): until then a
  task that never yields would keep the CPU.
* Still no on-target IPC, no on-target capability enforcement, no
  supervisor recovery on target (later stages).
* Two tasks share one position-independent code page (mapped into each
  address space); a real system loads separate user images.
* Default build remains the v0.2 single-task memory-isolation demo; the
  multitask demo is the `demo_multitask` feature.

## 4. Next Stage

v0.4 — Timer Interrupt and Preemption (Full Completion Mode §13): make
the scheduler preemptive so a low-priority infinite loop cannot freeze
the kernel and a high-priority task still runs.
