# AxiomRT Formal Proofs

Requirement reference: docs/11_VERIFICATION_PLAN.md, Project
Description §20.

Phase 12 starter models: small, self-contained Coq developments that
state the v0.1 verification targets with explicit assumptions. They
model the *specifications* of the corresponding kernel modules; the
refinement obligations connecting them to the implementation are
stated explicitly and marked TODO.

## Files

| File | Target | Kernel module |
|---|---|---|
| `coq/MemoryIsolation.v` | a task cannot read an address not mapped in its address space (MEM-P2/P3) | `kernel/src/memory/` |
| `coq/CapabilityAccess.v` | no protected-object invocation without a valid capability with sufficient rights (CAP-P1..P3) | `kernel/src/caps/` |
| `coq/SchedulerPriority.v` | if a high-priority ready task exists, a lower-priority task is not selected (SCHED-P1..P3) | `kernel/src/sched/` |

## Status policy

* Theorem statements are the deliverable of Phase 12.
* A proof may be `Admitted` only when marked with an explicit
  `TODO` comment naming what is missing.
* Refinement obligations (model ↔ implementation) are placeholders
  until the concrete integration phases land; each is marked TODO in
  its file.

## Build

Compiled with Coq (tested: 8.20):

```sh
cd proofs/coq
coqc MemoryIsolation.v
coqc CapabilityAccess.v
coqc SchedulerPriority.v
```

A file that fails to compile is a broken build (same rule as kernel
code, docs/07_CODEX_RULES.md).
