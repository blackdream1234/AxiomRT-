# AxiomRT Timer Interrupt and Preemption

Document ID: created by AXIOM-TIMER-001 (v0.4, Stage 3)
Requirement reference: docs/13_DISPATCH.md, docs/09_SCHEDULER_MODEL.md,
Full Completion Mode §13.

## 1. Goal and Boundary

v0.4 makes the scheduler **preemptive**: a periodic supervisor timer
interrupt lets the kernel take the CPU back from a running task and
schedule the highest-priority Ready task. The safety property this
proves: **a low-priority task that never yields (infinite loop) cannot
freeze the kernel or starve a high-priority task.**

Scope: SBI timer programming, supervisor timer interrupt enable, timer
trap routing, monotonic tick counter, priority-based preemptive
selection. Not in this stage: watchdog/deadline detection (v0.5), IPC
(v0.6). The default build is unchanged; the preemption demo is the
`demo_preempt` cargo feature.

## 2. Timer Source (AXIOM-TIMER-002)

QEMU virt exposes the RISC-V time CSR (`rdtime`, 10 MHz) and the SBI
TIME extension. `kernel/src/arch/riscv64/timer.rs`:

* `read_time()` reads the `time` CSR.
* `set_timer(t)` invokes SBI TIME extension (EID `0x54494D45`, FID 0)
  via `ecall` to program the next S-mode timer interrupt at absolute
  time `t`.
* `TIMER_INTERVAL` cycles between ticks (~10 ms).

## 3. Interrupt Enable (AXIOM-TIMER-003)

`timer::init()` sets `sie.STIE` (bit 5) so supervisor timer interrupts
are delivered. S-mode interrupts are always taken while the hart runs
in U-mode, so a running user task is preemptible without setting
`sstatus.SIE`; `sstatus.SIE` stays 0 in kernel context to keep the trap
handler non-reentrant.

## 4. Trap Routing (AXIOM-TIMER-004)

`__trap_vector` already funnels every trap through `trap_handler`. When
`scause` has the interrupt bit set and the code is 5 (supervisor
timer), the handler calls `timer::on_timer_interrupt(frame)` and
returns; any other interrupt remains a controlled panic (no interrupt
sources other than the timer are enabled).

## 5. Tick Counter and Preemption (AXIOM-TIMER-005/006/007)

`on_timer_interrupt`:

1. increments a monotonic `TICKS` counter (emits `TIMER tick=N` for the
   first few ticks, then counts silently to avoid flooding the log);
2. re-arms the next timer (`set_timer(read_time() + INTERVAL)`);
3. calls `dispatch::preempt(frame)`.

`dispatch::preempt` selects the highest-priority Ready task. If it
out-ranks the running task, the running task's live frame is saved
(state → Ready), the address space is switched, and the higher task is
loaded — emitting `SCHED preempt=<current> selected=<next>`. If no
Ready task out-ranks the current one, the current task simply continues
(the tick is a no-op switch): a lone low-priority loop keeps running but
remains preemptible, and the kernel stays alive.

Selection is priority-based with round-robin tie-breaking among equal
priorities (this generalizes the v0.3 round-robin, which becomes the
equal-priority case; SCHED-P1, docs/09 §4).

## 6. Expected QEMU Output (demo_preempt)

`low_loop` (priority 0, infinite loop, never yields) enters first;
`critical_task` (priority 7) is Ready. The timer preempts the loop and
runs the critical task:

```text
MMU status=enabled mode=sv39 scope=kernel
TASK_STARTED task=low_loop
TASK_STARTED task=critical_task
TIMER tick=1
SCHED preempt=low_loop selected=critical_task
SYSCALL name=sys_yield task=critical_task
...
TASK_EXITED task=critical_task
KERNEL alive=true
```

After the critical task exits, the low-priority loop runs again but is
preempted on every tick (no higher task Ready) — the kernel never
freezes.

## 7. Test (AXIOM-TIMER-008)

`tests/timer_preemption_qemu_test.sh` builds with `--features
demo_preempt`, boots, and asserts: both tasks start, `TIMER tick=1`
appears, the infinite loop is preempted in favor of the critical task
(`SCHED preempt=low_loop selected=critical_task`), the critical task
runs, and no `PANIC` line appears (kernel stays alive).
