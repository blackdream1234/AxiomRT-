# AxiomRT v1.2 Evidence — Application Model + Loader

Date: 2026-07-07. Milestone: AXIOM-APP-001..009 (AXIOMapp.md).

## What is demonstrated (QEMU virt, RISC-V 64)

- Static application model (docs/27): app policy entirely in
  user-space app_loader_service; kernel provides only address-space
  creation, manifest capability assignment, start/kill/restart, and
  fault containment.
- Shell: `apps`, `app info <name>`, `run <name>` (raw line forwarded
  to the loader over bounded IPC, one-line reply); all previous
  commands preserved, including `run demo`.
- hello: runs isolated, prints via its manifest console capability,
  exits cleanly; re-runnable (slot re-armed).
- counter: three progress events with yields, clean exit.
- fault_demo: zero capabilities — unauthorized write CAP_DENIED, CPU
  exhaustion watchdog-contained, supervisor Kill, shell alive after.
- Controlled shutdown still exits QEMU with code 0.

## Verification

- tests/app_loader_qemu_test.sh: 19 assertions PASS (app_loader_test.log).
- ./scripts/verify_all.sh: VERIFY ALL: PASS — 11/11 QEMU tests, all
  host suites, 3 Coq files, zero warnings (verify_all.log).
- cargo fmt --check clean; clippy -D warnings clean (default riscv,
  os_boot riscv, host workspace all-targets).

## Boundary (unchanged)

Apps are compiled into the kernel image (static table stage — no
filesystem, no storage, no dynamic loading, per the phase boundary);
emulator-only; refinement TODO; no certification claim.
