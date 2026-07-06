# AxiomRT Full Four-Task Fault-Containment Demo

Document ID: created by AXIOM-DEMO-001 (v0.9, Stage 8)
Requirement reference: docs/00_PROJECT_CHARTER.md §7, Project
Description §19, Full Completion Mode §18.

## 1. Goal

Run the demonstration the charter defines: four tasks — `critical_task`,
`supervisor_task`, `logger_task`, `faulty_task` — where the faulty task
attacks the system and the kernel contains every attempt while the
critical task keeps running and the kernel stays alive. This composes
every on-target capability built in v0.2–v0.8.

## 2. Tasks and Priorities

| Task | Priority | Role |
|---|---|---|
| supervisor_task | 7 | recv on the fault channel (Receive+Control), applies recovery |
| logger_task | 6 | recv on the event channel (Receive), records evidence |
| faulty_task | 5 | attacks: illegal IPC, then CPU exhaustion |
| critical_task | 4 | yields periodically; must keep running |

Entry order lets the supervisor and logger block on their channels
first, so they are waiting when the faulty task is contained.

## 3. Attack Sequence and Containment

The faulty task performs two attacks that a single contained task can
exhibit in sequence (a fault terminates a task, so the fault-inducing
attack comes last):

1. **Illegal IPC** — `sys_send` with no capability → **CAP_DENIED**,
   endpoint unchanged (invalid IPC never succeeds, docs/18).
2. **CPU exhaustion** — an infinite loop that never checks in → the
   **watchdog** contains it (`WatchdogTimeout` → Faulted, docs/16).

On containment the kernel notifies the supervisor (fault event) and the
logger (`TASK_FAULTED`); the supervisor applies **Kill**
(`RECOVERY_APPLIED`). The critical task, now the only Ready task, runs
on — proving it survived the attack.

Coverage of the other charter attacks:

* **Illegal memory access → page fault** is demonstrated and tested
  separately by the v0.2 memory-isolation QEMU tests
  (`tests/memory_isolation_qemu_test.sh`): a user access to kernel or
  unmapped memory is contained.
* **Illegal syscall** returns `ERR_INVALID_SYSCALL` (docs/04); the
  denied-IPC path exercises the deny-by-default syscall surface.
* **Repeated crash / Restart recovery**: v0.9 applies **Kill** on
  target; on-target **Restart** (re-create the task from its boot
  image) is a documented v0.9+ elaboration — the kernel already
  survives the faulting task (the containment property), which is the
  safety-relevant guarantee.

## 4. Expected QEMU Output (demo_full)

```text
TASK_STARTED task=supervisor_task
TASK_STARTED task=logger_task
TASK_STARTED task=faulty_task
TASK_STARTED task=critical_task
CAP_DENIED task=faulty_task reason=no_valid_capability
IPC state=unchanged
FAULT type=WatchdogTimeout task=faulty_task
CONTAIN scope=user reason=watchdog_timeout action=faulted kernel=alive
IPC delivered fault_event to=supervisor_task from=faulty_task
LOGGER event=TASK_FAULTED task=faulty_task
SUPERVISOR decision=Kill by=supervisor_task
RECOVERY_APPLIED policy=Kill
SCHED selected=critical_task
SYSCALL name=sys_yield task=critical_task
... (critical_task continues; kernel alive)
```

The repeated scheduling of `critical_task` after the faulty task is
killed is the evidence that the critical task continues and the kernel
never freezes.

## 5. Test (AXIOM-DEMO-002)

`tests/full_fault_containment_demo_qemu_test.sh` builds with `--features
demo_full`, boots, and asserts: all four tasks start, the illegal IPC is
denied, the CPU-exhaustion loop is contained as a watchdog timeout, the
fault reaches the supervisor and logger, the supervisor applies a
recovery policy, the critical task continues running after the faulty
task is killed, and no `PANIC` appears (kernel alive).
