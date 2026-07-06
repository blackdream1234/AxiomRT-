# AxiomRT v0.1 Final Report

Evaluation baseline report for the AxiomRT Safety Core.

Requirement reference: Full Completion Mode §10 (Stage 0),
docs/INDUSTRIAL_EVALUATION_KIT.md.

## 1. Purpose

This report freezes AxiomRT v0.1 as a formal evaluation baseline. It
records exactly what was built, what was verified, how, and with which
tools — so the state is reproducible and the boundary is explicit.

**No certification claim is made. No production-readiness claim is
made.** AxiomRT v0.1 is an evaluation-stage prototype.

## 2. Baseline Identity

* Git commit: see `git_commit.txt`.
* Git history: see `git_history.txt` (one commit per task ID).
* Tool versions: `rust_version.txt`, `qemu_version.txt`,
  `coq_version.txt`.

## 3. What Was Built

A RISC-V 64 microkernel (Rust `no_std`, zero external dependencies) plus
a `no_std` user-space supervisor service crate, developed document-first
across phases 0–13 of the original prompt pack:

* Boot on QEMU `virt` through OpenSBI; UART boot banner.
* Controlled trap layer (exceptions, illegal-instruction, syscall stub).
* Memory model, thread model, fixed-priority scheduler, synchronous
  copy-based IPC, capability-based access control, fault events +
  handling policy, structured runtime monitoring — as host-tested model
  layers.
* First controlled S→U transition with user-fault containment on target.
* Three Coq starter models with proven model-level theorems.

## 4. Verified Facts (this baseline)

| Fact | Evidence file |
|---|---|
| QEMU boot + U-mode demo runs; user fault contained; kernel survives | `qemu_demo.log` |
| Boot smoke test passes (banner present) | `boot_smoke.log` |
| Kernel host test suites pass (109 tests) | `host_tests.log` |
| Supervisor crate tests pass (4 tests) | `supervisor_tests.log` |
| MemoryIsolation.v compiles | `coq_memory.log` |
| CapabilityAccess.v compiles | `coq_capability.log` |
| SchedulerPriority.v compiles | `coq_scheduler.log` |

Total deterministic automated tests: 113 (109 kernel + 4 supervisor),
all passing; QEMU boot smoke test passing; three Coq files compiling.

Demonstrated on target (from `qemu_demo.log`): U-mode entry, syscall
round-trip through the trap path, a deliberate privileged-instruction
fault contained, kernel survival.

## 5. Boundary and Limitations (explicit, never hidden)

* **Privilege isolation** (U/S mode) is hardware-enforced and
  demonstrated on target.
* **Memory isolation** is model-level only in v0.1: the MMU (Sv39) is
  NOT yet activated. Addressed first in v0.2 (Stage 1).
* One user task runs on target; multi-task scheduling, timer
  preemption, watchdog, on-target IPC, and the supervisor recovery
  chain are host-tested models, not yet dispatched on target.
* Syscalls beyond the capability gate return `ERR_NOT_IMPLEMENTED`.
* Formal proofs cover models; refinement obligations to the Rust
  implementation are explicit TODOs in each proof file.
* Emulator only; no physical board has run this kernel.

Full limitations: docs/INDUSTRIAL_EVALUATION_KIT.md §9.

## 6. Reproduction

From the repository root (host toolchain: Rust stable +
`riscv64gc-unknown-none-elf`, QEMU ≥ 7 with OpenSBI, Coq 8.20):

```sh
./scripts/run_qemu.sh
./tests/boot_smoke_test.sh
cargo test --target x86_64-unknown-linux-gnu -p kernel
cargo test --manifest-path userland/supervisor/Cargo.toml \
           --target x86_64-unknown-linux-gnu
coqc proofs/coq/MemoryIsolation.v
coqc proofs/coq/CapabilityAccess.v
coqc proofs/coq/SchedulerPriority.v
```

## 7. Next Stage

v0.2 — Sv39/MMU Hardware Memory Isolation (Full Completion Mode §11):
activate Sv39, enforce kernel/user memory separation on target, and
upgrade the memory-isolation claim from model-level to
QEMU-hardware-enforced for the tested cases.
