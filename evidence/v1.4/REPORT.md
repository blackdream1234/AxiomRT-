# AxiomRT v1.4 Evidence — Storage Service

Date: 2026-07-07. Milestone: AXIOM-STOR-001..011 (AxiomRTos.md §7).

## Demonstrated (QEMU virt, RISC-V 64)

- storage_service: user-space read-only block service (8 x 48-byte
  static image), INFO / READ / READ_RANGE protocol over bounded IPC
  (endpoint 5); malformed input answers ERR, never crashes.
- Shell: storage info / storage read <n>; capability table grown
  6 -> 8 with every pre-existing capability preserved (asserted).
- cat /storage/version travels shell -> fs_service -> storage_service
  -> fs_service -> shell; the kernel routes and checks capabilities
  only (its whole contribution: endpoint id + two rights bits).
- Live containment evidence: during bring-up an LLVM switch lookup
  table in kernel .rodata made storage_service page-fault; it was
  contained and supervisor-killed, then fixed (branch-per-arm rule).
- Apps still run after storage errors; controlled shutdown exit 0.

## Verification

- tests/storage_service_qemu_test.sh: 13 assertions PASS.
- ./scripts/verify_all.sh: VERIFY ALL: PASS — 13/13 QEMU tests, all
  host suites, 3 Coq files, zero warnings (verify_all.log).
- fmt clean; clippy -D warnings clean (default, os_boot, host).

## Boundary

Static backing store (no virtio driver yet — the three missing kernel
mechanisms are documented in docs/30 and scoped to phase v1.5);
read-only; single-block replies (block_size=48, disclosed deviation
from the roadmap's 64-byte example); emulator-only; no certification
claim.
