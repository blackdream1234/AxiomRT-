# 26 — Console Service and Shell

Document ID: created by AXIOM-SHELL-001 (Real OS Phase 8).
Requirement reference: `AxiomrtFull Completion Mode.md` §16,
docs/25_OS_BOOT_FLOW.md, docs/20_REAL_OS_PRODUCT_DEFINITION.md §5.

## 1. Split of responsibility

* **Kernel**: byte-level console mechanism only (sys_con_read /
  sys_con_write, capability-gated; polled NS16550A).
* **console_service** (U-mode): owns console *input*. Polls
  sys_con_read, echoes, handles backspace, assembles a line, sends the
  completed line to the shell over the line endpoint (synchronous
  bounded IPC, ≤ 64 bytes). Yields between polls.
* **shell_service** (U-mode): blocks on recv for a line, parses it,
  executes the command, prints via sys_con_write, prints the next
  `axiom>` prompt. Reads system state exclusively through sys_info;
  changes system state exclusively through capability-gated task
  syscalls.

Both are constrained-Rust user-region services (docs/25 §2): no
string literals, no core fmt, no panics, manual byte loops.

## 2. Commands (roadmap §16 list)

| command | behavior |
|---|---|
| `help` | list commands |
| `version` | AxiomRT version + stage line |
| `tasks` | sys_info(tasks): index, name, prio, state per task |
| `faults` | sys_info(faults): fault event ring |
| `ipc` | sys_info(ipc): endpoint states + delivery counters |
| `caps` | sys_info(caps): per-task capability summary |
| `memory` | sys_info(memory): kernel/user window layout, MMU mode |
| `uptime` | sys_info(uptime): timer ticks since boot |
| `events` | sys_info(events): recent kernel event ring |
| `run demo` | sys_task_start(faulty demo task): capability denial + watchdog containment + recovery, live on the console |
| `kill <idx>` | sys_task_kill(idx) |
| `restart <idx>` | sys_task_restart(idx) |
| `clear` | ANSI clear screen |
| `shutdown` | sys_shutdown (controlled SBI SRST) |

Unknown input prints an error and a fresh prompt; empty input prints
a fresh prompt. Line length is bounded (64 bytes); overlong lines are
truncated by the console service, never overflowed.

## 3. Failure containment

A shell or console crash is an ordinary contained user fault: the
kernel survives, the supervisor is notified over the fault channel,
and `restart` (from a restarted shell) or the supervisor policy can
bring the service back. The kernel never parses command text.

## 4. Gate

```text
axiom> help
axiom> tasks
axiom> run demo
```

work interactively in QEMU (tests/os_shell_qemu_test.sh pipes these
over stdin and asserts the responses; manual interaction matches).
