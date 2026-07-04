# AxiomRT

AxiomRT is a formally specified microkernel-based safety runtime for
high-assurance embedded systems.

**Current phase: Phase 1 — Repository Skeleton.** This phase is repository
setup only: folder structure, documentation index, and placeholders. No
kernel implementation exists yet. The first kernel code begins in Phase 2,
after the Phase 0 gate (docs/08_PHASE_0_GATE.md) and this skeleton are
complete.

Repository layout:

```text
AxiomRT/
├── README.md    — this file
├── docs/        — Phase 0 blueprint documents and all design docs
├── kernel/      — microkernel (Rust no_std) — empty until Phase 2
├── userland/    — user-space services (supervisor, logger, demo tasks)
├── proofs/      — formal models (Coq) — Phase 12
├── tests/       — test suites (boot smoke, scheduler, IPC, capabilities)
├── tools/       — development tooling
├── scripts/     — build and QEMU run scripts
├── ci/          — continuous integration configuration
└── examples/    — demo scenarios (fault containment demo)
```

Start reading at docs/00_PROJECT_CHARTER.md.
