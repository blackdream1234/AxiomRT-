# AxiomRT v1.3 Evidence — Read-only Filesystem Service

Date: 2026-07-07. Milestone: AXIOM-FS-001..009 (axiomFX.md).

## What is demonstrated (QEMU virt, RISC-V 64)

- fs_service: user-space read-only filesystem (six files + directory
  listings as sectioned constants), LS/CAT protocol over bounded IPC
  (endpoint 4, <=64-byte request/reply, docs/28). Zero filesystem
  logic in the kernel — it only added an endpoint id and rights bits.
- Shell: ls, ls <path>, cat <path>; replies printed verbatim; all
  earlier commands preserved.
- Deny-by-default: only shell_service holds the fs endpoint
  capability (with fs_read/fs_list rights bits); apps and fault_demo
  cannot reach fs_service at all.
- Robustness: invalid and overlong paths answer ERR; the shell stays
  alive; no kernel panic.
- Controlled shutdown still exits QEMU with code 0.

## Verification

- tests/readonly_fs_qemu_test.sh: 13 assertions PASS.
- ./scripts/verify_all.sh: VERIFY ALL: PASS — 12/12 QEMU tests, all
  host suites, 3 Coq files, zero warnings (verify_all.log).
- cargo fmt --check clean; clippy -D warnings clean (default riscv,
  os_boot riscv, host workspace all-targets).

## Boundary (unchanged)

Read-only, embedded image (no storage/block devices/writes/dynamic
loading per the phase boundary); per-endpoint capability granularity
stated in docs/28 §7; emulator-only; refinement TODO; no
certification claim.
