# AxiomRT v1.0 — Test Report

All results below were produced on the QEMU `virt` machine (RISC-V 64)
with the OpenSBI firmware bundled with QEMU, and on the host for the
unit/integration suites. Reproduce with the commands in each section.

## 1. QEMU Serial-Assertion Tests (9/9 PASS)

Each script builds the relevant demo (a cargo feature), boots it, and
asserts the expected structured serial output.

| Test | Asserts | Result |
|---|---|---|
| `tests/boot_smoke_test.sh` | boot banner present | PASS |
| `tests/memory_isolation_qemu_test.sh` | read-kernel / write-unmapped / exec-nonexec each page-fault & contained | PASS |
| `tests/two_task_qemu_test.sh` | two U-mode tasks alternate via sys_yield | PASS |
| `tests/timer_preemption_qemu_test.sh` | infinite loop preempted; critical task runs; no panic | PASS |
| `tests/watchdog_qemu_test.sh` | CPU exhaustion → WatchdogTimeout → contained | PASS |
| `tests/ipc_rendezvous_qemu_test.sh` | cross-address-space message delivered; no panic | PASS |
| `tests/capability_qemu_test.sh` | cap-less send denied, endpoint unchanged; capable send delivers | PASS |
| `tests/supervisor_qemu_test.sh` | fault reaches supervisor + logger; RECOVERY_APPLIED | PASS |
| `tests/full_fault_containment_demo_qemu_test.sh` | four-task charter demo; critical continues; no panic | PASS |

## 2. Host Tests

```sh
cargo test --target x86_64-unknown-linux-gnu -p kernel
cargo test --manifest-path userland/supervisor/Cargo.toml \
           --target x86_64-unknown-linux-gnu
```

* Kernel: **125 passing** across unit tests and the scheduler, IPC,
  capability, and IPC-capability integration suites.
* Supervisor crate: **4 passing**.

Total automated tests: **129 host + 9 QEMU = 138**, all passing.

## 3. Formal Model Compilation

```sh
coqc proofs/coq/MemoryIsolation.v
coqc proofs/coq/CapabilityAccess.v
coqc proofs/coq/SchedulerPriority.v
```

All three compile cleanly (Coq 8.20).

## 4. Build Health

The default (`no feature`) build and all seven demo-feature builds
compile with **zero warnings** for `riscv64gc-unknown-none-elf`. Zero
external runtime dependencies.

## 5. Reproduction — One Command

```sh
./scripts/verify_all.sh
```

runs the full suite (all QEMU tests, host tests, and Coq compilation)
and restores the default build.
