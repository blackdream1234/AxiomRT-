# AxiomRT Test Strategy

Document ID: created by AXIOM-BOOT-005 (Phase 2); grows with each phase.
Requirement reference: docs/00_PROJECT_CHARTER.md §8, Project Description §21.

## Principles

* Every kernel mechanism gets tests before the next phase starts.
* Tests are deterministic: no timing-dependent flakiness is accepted.
* Negative tests matter most: the test suite must show that forbidden
  behavior actually fails (fault injection philosophy).
* A test failure blocks the phase gate; tests are never removed or
  weakened to make a change pass (docs/07_CODEX_RULES.md §2).

## Test Levels

| Level | Runs on | Purpose |
|---|---|---|
| QEMU smoke tests | qemu-system-riscv64 | Boot path and end-to-end kernel behavior |
| Unit tests | host (cargo test) | Pure-logic modules: scheduler, IPC, capabilities |
| Property tests | host | Invariants over generated inputs (later phase) |
| Fault-injection tests | QEMU | Forbidden behavior is contained (Phase 10+) |
| Regression tests | host + QEMU | Every fixed bug gets a pinned test |
| Fuzzing / static analysis | host | Later (Project Description §21) |

## Boot Smoke Test (AXIOM-BOOT-005) — first mandatory test

Script: `tests/boot_smoke_test.sh`

What it does:

1. Builds the release kernel (`cargo build --release`).
2. Boots it on QEMU virt with OpenSBI (`-bios default`), serial to stdio,
   bounded by a 15 s timeout (the kernel halts in a `wfi` loop by design,
   so QEMU never exits on its own; timeout exit 124 is the expected path).
3. Greps the captured serial log for the exact banner lines:
   `AxiomRT kernel booted`, `arch=riscv64`, `phase=boot`.

Pass/fail contract:

* **PASS** (exit 0): all three banner lines appear.
* **FAIL** (exit 1): any banner line missing, or QEMU failed to run.
  The full serial log path is printed for diagnosis.

Run from the repository root:

```sh
./tests/boot_smoke_test.sh
```

This test is the Phase 2 gate: Phase 3 (trap layer) may not start until it
passes.
