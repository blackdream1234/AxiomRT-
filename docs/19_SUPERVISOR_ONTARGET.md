# AxiomRT On-Target Supervisor and Logger

Document ID: created by AXIOM-SUPRT-001 (v0.8, Stage 7)
Requirement reference: docs/06_FAULT_MODEL.md (supervisor),
docs/17_IPC_ONTARGET.md, docs/16_WATCHDOG.md, Full Completion Mode §17.

## 1. Goal and Boundary

v0.8 moves the fault-recovery chain onto the target: a trusted
`supervisor_task` and a `logger_task` run in U-mode; when a task faults,
the kernel notifies them over dedicated IPC endpoints, the supervisor
applies a recovery decision, and the logger records a monitoring event.
Scope: the supervisor/logger tasks, the fault and event channels,
kernel→task fault notification, and `sys_fault_ack`. The full four-task
attack demo is v0.9. Demo behind the `demo_supervisor` cargo feature;
default build unchanged.

## 2. Channels

Two dedicated endpoints (ids fixed on target):

* **fault channel** (id 2): `supervisor_task` holds a Receive
  capability; the kernel pushes a fault descriptor here when a task
  faults.
* **event channel** (id 3): `logger_task` holds a Receive capability;
  the kernel pushes a monitoring event here.

The supervisor is trusted for *policy*, not for isolation: it receives
fault events **only** through the capability-checked recv path — no
capability, no events (docs/06 §17; the host supervisor tests cover the
bypass-attempt case).

## 3. Kernel Notification (AXIOM-SUPRT-005/008)

When the kernel contains a fault (v0.8: the watchdog timeout path,
docs/16 §3), it calls `notify_supervisor_and_logger`:

* if a task is blocked receiving on the fault channel, a one-byte fault
  descriptor is staged as a pending delivery and the supervisor is made
  Ready; the kernel logs `IPC delivered fault_event to=supervisor_task`;
* likewise for the logger on the event channel, logging
  `LOGGER event=TASK_FAULTED task=<name>`.

Delivery is completed by `resume_task` when the supervisor/logger next
runs (its address space active). Notifications use embedded payloads, so
they never contend with user IPC buffers.

## 4. Recovery Acknowledgement (AXIOM-SUPRT-006/007)

After receiving a fault event the supervisor decides and calls
`sys_fault_ack(a1 = decision)` (2 = Kill, 1 = Restart). The kernel
records the applied policy:

```text
SUPERVISOR decision=Kill by=supervisor_task
RECOVERY_APPLIED policy=Kill
```

The faulted task is already contained (Faulted, never rescheduled); Kill
is the terminal decision in the demo. Restart (re-create from the boot
image) is a v0.9+ elaboration.

## 5. Expected QEMU Output (demo_supervisor)

`supervisor_task` and `logger_task` block on their channels;
`faulty_task` runs an infinite loop; the watchdog contains it and the
kernel notifies both:

```text
TASK_STARTED task=supervisor_task
TASK_STARTED task=logger_task
TASK_STARTED task=faulty_task
IPC endpoint=log op=recv state=blocked
...
FAULT type=WatchdogTimeout task=faulty_task
CONTAIN scope=user reason=watchdog_timeout action=faulted kernel=alive
IPC delivered fault_event to=supervisor_task from=faulty_task
LOGGER event=TASK_FAULTED task=faulty_task
SUPERVISOR decision=Kill by=supervisor_task
RECOVERY_APPLIED policy=Kill
```

## 6. Test (AXIOM-SUPRT-008)

`tests/supervisor_qemu_test.sh` builds with `--features
demo_supervisor`, boots, and asserts: the fault event reaches the
supervisor (`IPC delivered fault_event to=supervisor_task`), the logger
records it (`LOGGER event=TASK_FAULTED`), the supervisor applies a policy
(`RECOVERY_APPLIED policy=Kill`), and no `PANIC` appears. The host
supervisor crate tests
(`cargo test --manifest-path userland/supervisor/Cargo.toml ...`)
continue to cover the model-level decision logic and the
capability-bypass rejection.
