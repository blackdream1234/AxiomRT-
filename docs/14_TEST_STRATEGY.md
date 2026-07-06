# AxiomRT Test Strategy

Document ID: created by AXIOM-BOOT-005 (Phase 2); grows with each phase.
Requirement reference: docs/00_PROJECT_CHARTER.md §8, Project Description §21.

## Principles

* Every kernel mechanism gets tests before the next phase starts.
* Tests are deterministic: no timing-dependent flakiness is accepted.
* Negative tests matter most: the test suite must show that forbidden
  behavior actually fails (fault injection philosophy).
* A test failure blocks the phase gate; tests are never removed or
  weakened to make a change pass (docs/07_CODEX_RULES.md §2).

## Test Levels

| Level | Runs on | Purpose |
|---|---|---|
| QEMU smoke tests | qemu-system-riscv64 | Boot path and end-to-end kernel behavior |
| Unit tests | host (cargo test) | Pure-logic modules: scheduler, IPC, capabilities |
| Property tests | host | Invariants over generated inputs (later phase) |
| Fault-injection tests | QEMU | Forbidden behavior is contained (Phase 10+) |
| Regression tests | host + QEMU | Every fixed bug gets a pinned test |
| Fuzzing / static analysis | host | Later (Project Description §21) |

## Boot Smoke Test (AXIOM-BOOT-005) — first mandatory test

Script: `tests/boot_smoke_test.sh`

What it does:

1. Builds the release kernel (`cargo build --release`).
2. Boots it on QEMU virt with OpenSBI (`-bios default`), serial to stdio,
   bounded by a 15 s timeout (the kernel halts in a `wfi` loop by design,
   so QEMU never exits on its own; timeout exit 124 is the expected path).
3. Greps the captured serial log for the exact banner lines:
   `AxiomRT kernel booted`, `arch=riscv64`, `phase=boot`.

Pass/fail contract:

* **PASS** (exit 0): all three banner lines appear.
* **FAIL** (exit 1): any banner line missing, or QEMU failed to run.
  The full serial log path is printed for diagnosis.

Run from the repository root:

```sh
./tests/boot_smoke_test.sh
```

This test is the Phase 2 gate: Phase 3 (trap layer) may not start until it
passes.

## Host Unit and Integration Tests (Phases 4+)

Pure-logic kernel modules (memory model, thread model, scheduler, later
IPC and capabilities) are tested on the host — no hardware dependency,
fully deterministic. Run from the repository root:

```sh
cargo test --target x86_64-unknown-linux-gnu -p kernel
```

(The explicit `--target` overrides the default bare-metal target in
`.cargo/config.toml`; unit tests live next to their modules, integration
suites live in `tests/` and are wired as `[[test]]` targets of the
kernel crate.)

## Scheduler Tests (AXIOM-SCHED-002)

Suite: `tests/scheduler_tests.rs` — drives `FixedPriorityScheduler`
together with the Thread state machine (the readiness authority,
docs/09_SCHEDULER_MODEL.md §4). Mandatory cases:

* highest-priority task selected (SCHED-P1)
* killed task not selected — including with a deliberately stale ready
  queue entry (SCHED-P2 defense in depth)
* blocked task not selected
* faulted task not selected
* equal priority uses the deterministic FIFO rule, reproducibly
  (SCHED-P3)

All cases must pass with no hardware dependency before Phase 7.

## Memory Isolation QEMU Tests (AXIOM-MEMHW-009..011, v0.2)

Script: `tests/memory_isolation_qemu_test.sh` — boots the kernel under
Sv39 and asserts that forbidden user memory accesses take the expected
page fault, are contained, and the kernel survives. Cases (each selects
the demo probe via a cargo feature; the default build is restored at the
end):

* read of kernel memory → load page fault,
  `reason=user_access_kernel_memory` (MEMHW-009);
* write of an unmapped user address → store page fault,
  `reason=user_access_unmapped` (MEMHW-010);
* execute of a non-executable user page → instruction page fault,
  `reason=user_execute_nonexecutable` (MEMHW-011).

Each case asserts `MMU status=enabled` and `kernel=survived`. This is
the v0.2 gate evidence that memory isolation is MMU-enforced for the
tested cases.

## Two-Task Dispatch QEMU Test (AXIOM-SCHEDRT-007, v0.3)

Script: `tests/two_task_qemu_test.sh` — builds with `--features
demo_multitask`, boots, and asserts both tasks start, execution
alternates (both `SCHED selected=task_a` and `SCHED selected=task_b`
appear), both tasks exit, and the demo completes
(`phase=multitask-demo-complete`). Restores the default build
afterwards. This is the v0.3 gate evidence that two U-mode tasks run
and cooperatively switch on target.

## Timer Preemption QEMU Test (AXIOM-TIMER-008, v0.4)

Script: `tests/timer_preemption_qemu_test.sh` — builds with `--features
demo_preempt`, boots, and asserts a low-priority infinite loop is
preempted by the timer (`SCHED preempt=low_loop selected=critical_task`)
in favour of a high-priority task, that the critical task runs and
exits, and that **no `PANIC` line appears** (the kernel stays alive
despite the infinite loop). This is the v0.4 gate evidence that the
scheduler is preemptive and a runaway task cannot freeze the kernel.

## Watchdog QEMU Test (AXIOM-WDOG-008, v0.5)

Script: `tests/watchdog_qemu_test.sh` — builds with `--features
demo_watchdog`, boots, and asserts an infinite compute loop that never
checks in is detected as a watchdog timeout
(`FAULT type=WatchdogTimeout task=faulty_task`), contained
(`CONTAIN ... reason=watchdog_timeout action=faulted`), that the
critical task then runs (`SCHED selected=critical_task`), and that no
`PANIC` appears. This is the v0.5 gate evidence that CPU exhaustion is
detected and contained while a critical task continues.

## On-Target IPC QEMU Test (AXIOM-IPCRT-010, v0.6)

Script: `tests/ipc_rendezvous_qemu_test.sh` — builds with `--features
demo_ipc`, boots, and asserts a synchronous message exchange between two
U-mode tasks in separate address spaces: the receiver blocks
(`state=blocked`), the sender sends, the message is delivered across the
address spaces (`IPC delivered bytes=4`), both tasks exit, and no
`PANIC` appears. This is the v0.6 gate evidence that bounded,
copy-based, blocking IPC works on target with no shared memory.

## On-Target Capability Enforcement QEMU Test (AXIOM-CAPRT-008, v0.7)

Script: `tests/capability_qemu_test.sh` — builds with `--features
demo_cap`, boots, and asserts a send from a task without a valid
capability is denied (`CAP_DENIED ... reason=no_valid_capability`) with
the endpoint unchanged (`IPC state=unchanged`), that a send from a task
holding the Send capability then delivers (`IPC delivered bytes=4`), and
that no `PANIC` appears. This is the v0.7 gate evidence that every IPC
operation is capability-controlled on target (deny-by-default).

## On-Target Supervisor QEMU Test (AXIOM-SUPRT-008, v0.8)

Script: `tests/supervisor_qemu_test.sh` — builds with `--features
demo_supervisor`, boots, and asserts a contained fault is delivered to
the supervisor over IPC (`IPC delivered fault_event to=supervisor_task`),
recorded by the logger (`LOGGER event=TASK_FAULTED`), and acknowledged
with a recovery policy (`RECOVERY_APPLIED policy=Kill`), with no
`PANIC`. This is the v0.8 gate evidence that the fault-recovery chain
(kernel → supervisor → recovery, plus logger evidence) runs on target;
the host supervisor crate tests still cover the decision logic and the
capability-bypass rejection.
