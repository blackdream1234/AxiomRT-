# AxiomRT Watchdog and Deadline Monitoring

Document ID: created by AXIOM-WDOG-001 (v0.5, Stage 4)
Requirement reference: docs/15_TIMER_PREEMPTION.md,
docs/06_FAULT_MODEL.md (WatchdogTimeout, DeadlineMiss),
docs/11_RUNTIME_MONITORING.md, Full Completion Mode §14.

## 1. Goal and Boundary

v0.5 detects CPU exhaustion and timing failures. The safety property:
**a task that stops making progress (an infinite loop that never checks
in) is detected within a bounded window, faulted, and contained, while
a critical task continues and the kernel stays alive.**

Scope: heartbeat/check-in, per-task liveness tracking on the timer
tick, `WATCHDOG_TIMEOUT` detection and containment, and a minimal
deadline-miss path. Not in this stage: on-target IPC delivery of the
fault event to a supervisor (v0.8) — the watchdog here emits structured
evidence and moves the task to Faulted directly. The demo is the
`demo_watchdog` cargo feature; the default build is unchanged.

## 2. Heartbeat / Check-In (AXIOM-WDOG-002/003)

A supervised task proves liveness by *checking in*: any syscall from a
task counts as a check-in (a well-behaved task periodically yields or
performs IPC). The dispatcher tracks a miss counter for the running
task; a check-in resets it to zero, and scheduling a new task resets it
(each task gets a fresh window). A task stuck in a pure compute loop
never checks in, so its miss counter grows every tick.

## 3. Timeout Detection (AXIOM-WDOG-004/005/006)

On each timer tick, before preemption, the dispatcher increments the
running task's miss counter. If it exceeds `WATCHDOG_WINDOW` ticks the
task is judged stuck (CPU exhaustion):

1. emit `FAULT type=WatchdogTimeout task=<name>` (fault-model event,
   docs/06 — Critical severity);
2. emit `CONTAIN scope=user reason=watchdog_timeout action=faulted
   kernel=alive`;
3. move the task to **Faulted** (never scheduled again — docs/09 §4,
   docs/06 invariant 3);
4. select the highest-priority Ready task and switch to it.

A watchdog timeout of task X never delays a higher-priority Ready task
(docs/06, WatchdogTimeout kernel action). Because the check runs on the
timer tick, even an equal-or-higher-priority hog (which preemption
alone would not displace) is contained.

## 4. Deadline Monitoring (AXIOM-WDOG-007)

A task may declare a deadline window (`deadline_ticks`). If it has not
completed (exited or checked in) within that window it is a
`DeadlineMiss` (docs/06): the dispatcher emits
`FAULT type=DeadlineMiss task=<name>` and a `DEADLINE_MISSED` monitoring
event. In v0.5 a missed deadline is recorded (Error severity, task
continues) — escalation to Restart/Kill is supervisor policy (v0.8).
The watchdog (Critical, contains) and the deadline monitor (Error,
records) share the same per-task tick bookkeeping.

## 5. Expected QEMU Output (demo_watchdog)

`faulty_task` (an infinite loop that never checks in) and
`critical_task` (periodically yields). The watchdog contains the loop:

```text
MMU status=enabled mode=sv39 scope=kernel
TASK_STARTED task=faulty_task
TASK_STARTED task=critical_task
TIMER tick=1
...
FAULT type=WatchdogTimeout task=faulty_task
CONTAIN scope=user reason=watchdog_timeout action=faulted kernel=alive
SCHED selected=critical_task
SYSCALL name=sys_yield task=critical_task
...
```

The faulty task never runs again; the critical task continues; the
kernel stays alive.

## 6. Test (AXIOM-WDOG-008)

`tests/watchdog_qemu_test.sh` builds with `--features demo_watchdog`,
boots, and asserts the watchdog timeout is detected and contained
(`FAULT type=WatchdogTimeout task=faulty_task`,
`CONTAIN ... reason=watchdog_timeout`), that the critical task then runs
(`SCHED selected=critical_task`), and that no `PANIC` line appears.
