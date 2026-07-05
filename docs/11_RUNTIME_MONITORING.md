# AxiomRT Runtime Monitoring

Document ID: created by AXIOM-MON-001 (Phase 11)
Requirement reference: Project Description §18, docs/06_FAULT_MODEL.md.

(Naming note: the file number 11 collides with 11_VERIFICATION_PLAN.md;
both names are fixed verbatim by the task pack.)

## 1. Principle

AxiomRT produces structured runtime events as *evidence*: every
security- or safety-relevant kernel action is observable, attributable,
and machine-parseable. Events are facts, not logs: they are never
interpreted, filtered, or stored by the kernel (no logging storage
backend in the kernel, docs/02 §4). In v0.1 events leave the system
through the serial port only; no filesystem exists.

## 2. Event Types (AXIOM-MON-001)

```text
TASK_STARTED     Info      task began execution
TASK_EXITED      Info      task ended voluntarily (sys_exit)
TASK_FAULTED     Error     task was contained after a fault
CAP_DENIED       Error     capability check failed
IPC_DENIED       Error     IPC attempt rejected
PAGE_FAULT       Error     invalid memory access trapped
DEADLINE_MISSED  Error     periodic task overran its deadline
WATCHDOG_TIMEOUT Critical  supervised task missed its liveness window
RECOVERY_APPLIED Info      supervisor decision was applied
```

Severity is derived from the event type (never chosen by the caller),
using the fault-model severity scale (docs/06_FAULT_MODEL.md).

## 3. Event Fields

Each event carries (docs/11 field list; Project Description §18):

| Field | Presence | Content |
|---|---|---|
| `ts` | always | timestamp (v0.1: caller-provided monotonic value; hardware time source wired with the timer phase) |
| `task` | always | thread ID (0 = kernel) |
| `type` | always | one of the nine names above |
| `sev` | always | `info` / `error` / `critical` / `fatal` |
| `phase` | always | kernel phase: boot, trap, syscall, sched, ipc, fault, user |
| `policy` | optional | recovery decision (RECOVERY_APPLIED) |
| `cap` | optional | related capability index (CAP_DENIED / IPC_DENIED) |
| `syscall` | optional | related syscall number |

Absent optional fields are omitted from the export — never zero-filled
or faked.

Model: `kernel/src/monitor/event.rs` (host-tested).

## 4. Serial Export (AXIOM-MON-002)

`kernel/src/monitor/serial.rs` renders one event per line in a fixed
key=value format and hands it to a caller-provided sink (on target: the
QEMU UART writer, docs/09_BUILD_AND_BOOT.md):

```text
EVT type=TASK_FAULTED ts=128 task=3 sev=error phase=trap
EVT type=CAP_DENIED ts=129 task=3 sev=error phase=syscall cap=0 syscall=3
EVT type=RECOVERY_APPLIED ts=130 task=3 sev=info phase=fault policy=kill
```

Rendering is `no_std`, allocation-free (fixed stack buffer, events
that would overflow it are truncated with an explicit `!truncated`
marker — never silently). The line format is stable: the evaluation
kit's expected-output documentation and any external log consumers
parse exactly this format.

## 5. Explicitly Absent in v0.1

* storage backend, ring buffers, or file output (no filesystem)
* event filtering or rate limiting inside the kernel
* timestamps from a calibrated wall clock (monotonic counter only)
* remote/network export
