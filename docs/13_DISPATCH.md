# AxiomRT On-Target Task Dispatch

Document ID: created by AXIOM-SCHEDRT-001 (v0.3, Stage 2)
Requirement reference: docs/09_SCHEDULER_MODEL.md, docs/10_USER_MODE.md,
docs/12_MMU_SV39.md, Full Completion Mode §12.

## 1. Goal and Boundary

v0.3 runs **multiple** U-mode tasks on target, switching between them
cooperatively via `sys_yield`. Scope: task control blocks, context
save/restore, the switch routine, two tasks, and the killed/faulted/
blocked exclusion. **Not** in this stage: timer preemption (v0.4),
IPC (v0.6), capability enforcement on target (v0.7). Switching is
cooperative only — a task keeps the CPU until it yields or exits.

The default build is unchanged (single-task memory-isolation demo of
v0.2); the two-task demo is selected by the `demo_multitask` cargo
feature so the v0.2 gate tests keep passing.

## 2. Task Control Block (AXIOM-SCHEDRT-001)

Each task has a control block (`kernel/src/arch/riscv64/dispatch.rs`):

* `state` — on-target run state: Empty, Ready, Running, Blocked,
  Faulted, Killed. The state machine is the scheduling authority
  (docs/09 §4): only a Ready task is ever selected.
* `satp_root` — physical root of the task's Sv39 address space; each
  task has its own (docs/12_MMU_SV39.md §5).
* `frame` — the task's full saved register context (a `TrapFrame`).
* `name` — a stable label for structured events.

Tasks live in a fixed static array (no heap); v0.3 supports
`MAX_TASKS = 4`.

## 3. Context Save/Restore (AXIOM-SCHEDRT-002)

The trap frame pushed by `__trap_vector` (trap.S) already captures the
full user register set plus `sepc` and `sstatus`. A context switch is
therefore a memory copy:

* **save:** the live trap frame is copied into the current task's TCB;
* **restore:** the next task's saved frame is copied into the live trap
  frame, so the ordinary trap-return path (`trap.S` restore + `sret`)
  resumes the next task.

A fresh task's frame is synthesized (`TrapFrame::new_user`): entry PC in
`sepc`, user stack in `x2`, `sstatus.SPP=0` (→ U on `sret`) with the
live `UXL` field preserved.

## 4. Switch Routine (AXIOM-SCHEDRT-003/005)

On `sys_yield` (or `sys_exit`) from user mode, the trap layer advances
`sepc` past the `ecall` and calls the dispatcher:

1. emit `SYSCALL name=sys_yield task=<name>`;
2. yield → snapshot the live frame into the current TCB, mark it Ready;
   exit → mark the current TCB Killed, emit `TASK_EXITED`;
3. select the next Ready task (round-robin, §5);
4. mark it Running, emit `SCHED selected=<name>`;
5. **switch `satp`** to the next task's address space and `sfence.vma`;
6. load the next task's saved frame into the live trap frame;
7. return — `trap.S` restores it and `sret`s into the next task.

Because every user address space also maps the kernel (U=0), the trap
handler code, trap stack, and the live frame remain valid across the
`satp` switch (docs/12_MMU_SV39.md §5).

## 5. Selection and Exclusion (AXIOM-SCHEDRT-006)

Selection is round-robin over Ready tasks starting after the current
index. Killed, Faulted, and Blocked tasks are never selected — this is
the on-target realization of SCHED-P2 (docs/09 §4). When no task is
Ready the dispatcher emits `SCHED idle=all_tasks_done` and halts.

## 6. Expected QEMU Output (two tasks, each yields twice then exits)

```text
MMU status=enabled mode=sv39 scope=kernel
TASK_STARTED task=task_a
TASK_STARTED task=task_b
SYSCALL name=sys_yield task=task_a
SCHED selected=task_b
SYSCALL name=sys_yield task=task_b
SCHED selected=task_a
...
TASK_EXITED task=task_a
TASK_EXITED task=task_b
SCHED idle=all_tasks_done
phase=multitask-demo-complete
```

## 7. Test (AXIOM-SCHEDRT-007)

`tests/two_task_qemu_test.sh` builds with `--features demo_multitask`,
boots, and asserts that both tasks start, that execution alternates
(`SCHED selected=task_b` and `SCHED selected=task_a` both appear), and
that both tasks exit and the demo completes.
