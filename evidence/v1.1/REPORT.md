# AxiomRT v1.1 Evidence — Interactive OS Boot Flow + Shell

Date: 2026-07-07. Milestone: AXIOM-INIT-001..005 + AXIOM-SHELL-001..009
(`AxiomrtFull Completion Mode.md` Phases 7–8).

## What is demonstrated (QEMU virt, RISC-V 64)

- Boot chain: OpenSBI → kernel → Sv39 MMU → timer → init_service →
  supervisor/logger/console/shell services → interactive `axiom>`.
- Console service owns input in U-mode (echo, backspace, bounded line
  assembly, line → shell over capability-checked bounded IPC).
- Shell commands: help, version, tasks, faults, ipc, caps, memory,
  uptime, events, run demo, kill/restart <idx>, clear, shutdown.
- `run demo`: live containment on the console — CAP_DENIED →
  WatchdogTimeout → CONTAIN → supervisor Kill → RECOVERY_APPLIED,
  shell keeps running.
- Multi-task synchronous fault containment: a service page fault is
  Faulted + supervisor-notified + rescheduled (exercised during
  bring-up when the shell hit an LLVM rodata jump table and was
  contained and killed by the supervisor).
- Controlled shutdown: SBI SRST from the shell; QEMU exits 0.

## Files

- os_shell_session.log — full serial transcript of a scripted session.
- os_shell_test.log — tests/os_shell_qemu_test.sh (18 assertions, PASS).
- tool_versions.txt — toolchain + commit.

## Boundary (unchanged honesty rules)

Emulator-only; services are kernel-image-embedded constrained-Rust
(static app table stage — a loader is the next phase); model↔code
formal refinement still TODO; no certification claim.
