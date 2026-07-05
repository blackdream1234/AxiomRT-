# AxiomRT Fault Containment Demo Scenario

Document ID: created by AXIOM-KIT-002 (Phase 13)
Requirement reference: docs/00_PROJECT_CHARTER.md §7, Project
Description §19, docs/INDUSTRIAL_EVALUATION_KIT.md §5.

This document is self-contained: the demo can be understood without
reading source code.

## 1. The Scenario (v0.1 target demonstration)

Four user tasks run on the AxiomRT kernel in QEMU:

| Task | Priority | Role |
|---|---|---|
| `critical_task` | highest | runs periodically; must never be disturbed |
| `supervisor_task` | high | receives fault events, applies recovery policy |
| `logger_task` | medium | receives structured events over IPC |
| `faulty_task` | lowest | deliberately attacks the system |

The faulty task attempts, in order:

1. **illegal syscall** — an unknown syscall number;
2. **illegal memory access** — touching memory outside its mappings;
3. **illegal IPC** — sending on an endpoint it holds no Send
   capability for;
4. **CPU exhaustion** — an infinite loop (bounded by preemption and
   watchdog);
5. **repeated crash** — faulting again after each restart.

Expected result (Project Description §19): the critical task
continues, the faulty task is blocked/killed/restarted per supervisor
policy, the supervisor receives fault events, the logger receives
structured events, the kernel remains stable, no unauthorized memory
access occurs, and no invalid IPC succeeds.

## 2. What v0.1 Demonstrates Today (runnable now)

The v0.1 kit demonstrates the containment core of the scenario with
one user task on target plus the full policy chain on the host:

**On target (QEMU):** `./scripts/run_qemu.sh` boots the kernel, which
starts a demo user task in U-mode. The task:

1. performs syscalls through the real trap path (`sys_yield`,
   `sys_exit` — answered by the stub layer),
2. attempts capability-less IPC → **denied** (`CAP_DENIED`, the
   illegal-IPC attack of the scenario),
3. executes a privileged instruction → **contained** (the
   illegal-instruction fault of the scenario): the task is terminated
   and the kernel continues, printing its survival banner.

**On host (deterministic tests):** the remaining scenario mechanics —
fault event → supervisor delivery over capability-checked IPC →
explicit recovery decision (Kill/Restart/…) → acknowledgement, plus
scheduler proof that the critical task is selected while the faulty
task is excluded — run as the test suites listed in
docs/14_TEST_STRATEGY.md.

The gap between §1 and §2 is exactly the integration work listed in
docs/INDUSTRIAL_EVALUATION_KIT.md §9 (MMU activation, on-target
multi-task dispatch, timer preemption).

## 3. Expected Output (QEMU serial, v0.1)

After the OpenSBI banner:

```text
AxiomRT kernel booted
arch=riscv64
phase=boot
USER enter=demo_task mode=U isolation=privilege
SYSCALL name=sys_yield status=stub result=ERR_NOT_IMPLEMENTED
SYSCALL name=sys_exit status=stub result=ERR_NOT_IMPLEMENTED
TRAP kind=illegal-instruction cause=0x0000000000000002 sepc=0x... stval=0x...
CONTAIN scope=user reason=illegal_instruction action=terminate_task kernel=alive
USER demo=first_user_task result=contained kernel=survived
phase=user-demo-complete
```

Line-by-line meaning:

* `AxiomRT kernel booted / arch / phase` — boot banner (checked by
  the smoke test).
* `USER enter=...` — the kernel drops to user privilege (U-mode).
* `SYSCALL ...` — each line is one full U-mode → trap → dispatch →
  U-mode round trip.
* `TRAP kind=illegal-instruction ...` — the user task attempted a
  privileged operation; the hardware trapped it.
* `CONTAIN ... kernel=alive` — the kernel terminated the task and
  kept running: **a user fault cannot crash the kernel.**
* `USER demo=... kernel=survived / phase=user-demo-complete` — the
  kernel continuation confirms survival.

## 4. How to Run

```sh
./scripts/run_qemu.sh          # interactive; exit with Ctrl-A then x
./tests/boot_smoke_test.sh     # automated boot check (PASS/FAIL)
cargo test --target x86_64-unknown-linux-gnu -p kernel   # host suites
cargo test --manifest-path userland/supervisor/Cargo.toml \
           --target x86_64-unknown-linux-gnu             # supervisor
```

## 5. Why This Demo Matters

The demo is the first proof that the system is meaningful: it shows
the three properties an evaluator cares about — (a) the kernel/user
privilege boundary is real, (b) authority is explicit and denied by
default, and (c) failure is contained and observable rather than
fatal — all with structured, parseable evidence on the serial port.
