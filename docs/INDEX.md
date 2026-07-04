# AxiomRT Documentation Index

## Phase 0 — Kernel Blueprint (complete)

| Document | Task | Content |
|---|---|---|
| [00_PROJECT_CHARTER.md](00_PROJECT_CHARTER.md) | AXIOM-DOC-001 | Mission, boundary, goals, guarantees, engineering rule |
| [01_SCOPE_AND_NON_GOALS.md](01_SCOPE_AND_NON_GOALS.md) | AXIOM-DOC-002 | Scope v0.1, non-goals, future scope, forbidden features |
| [02_KERNEL_BLUEPRINT.md](02_KERNEL_BLUEPRINT.md) | AXIOM-DOC-003 | Kernel identity, responsibilities, principles, exit criteria |
| [03_KERNEL_OBJECTS.md](03_KERNEL_OBJECTS.md) | AXIOM-DOC-004 | All eleven kernel objects, fully specified |
| [04_SYSCALL_MODEL.md](04_SYSCALL_MODEL.md) | AXIOM-DOC-005 | Seven v0.1 syscalls, validation rules, forbidden syscalls |
| [05_MEMORY_MODEL.md](05_MEMORY_MODEL.md) | AXIOM-DOC-006 | Address spaces, frames, page tables, permissions, faults |
| [06_FAULT_MODEL.md](06_FAULT_MODEL.md) | AXIOM-DOC-007 | Eight fault types, recovery options, logging fields |
| [07_CODEX_RULES.md](07_CODEX_RULES.md) | AXIOM-DOC-008 | AI assistant rules, task format, review checklist |
| [08_PHASE_0_GATE.md](08_PHASE_0_GATE.md) | AXIOM-DOC-009 | Phase 0 completion gate checklist |

## Later-phase documents (created by their phases)

| Document | Phase | Content |
|---|---|---|
| 09_BUILD_AND_BOOT.md | Phase 2 | Build instructions, QEMU boot, run scripts |
| 10_TRAP_MODEL.md | Phase 3 | Trap vector, exception decoding, syscall trap |
| 09_SCHEDULER_MODEL.md | Phase 6 | Fixed-priority scheduler model |
| 10_USER_MODE.md | Phase 7 | User mode transition |
| 08_IPC_MODEL.md | Phase 8 | Synchronous copy-based IPC |
| 06_CAPABILITY_MODEL.md | Phase 9 | Capability table and rights checking |
| 11_RUNTIME_MONITORING.md | Phase 11 | Structured runtime events |
| 11_VERIFICATION_PLAN.md | Phase 12 | Formal proof plan (Coq starters) |
| 14_TEST_STRATEGY.md | Phase 2+ | Test strategy (grows with each phase) |
| INDUSTRIAL_EVALUATION_KIT.md | Phase 13 | Evaluation kit definition |
| DEMO_SCENARIO.md | Phase 13 | Fault containment demo scenario |

File names above follow the task pack verbatim (numbering overlaps are
intentional and preserved).

## Phase order

```text
Phase 0  → Kernel Blueprint          (docs only — complete)
Phase 1  → Repository Skeleton       (this phase)
Phase 2  → Boot Kernel in QEMU
Phase 3  → Trap and Exception Layer
Phase 4  → Memory Isolation
Phase 5  → Thread Model
Phase 6  → Scheduler
Phase 7  → User Mode
Phase 8  → IPC
Phase 9  → Capabilities
Phase 10 → Fault Recovery
Phase 11 → Runtime Monitoring
Phase 12 → Formal Proof Starter
Phase 13 → Industrial Evaluation Kit
```

Rule: a phase may start only when the previous phase's definition of done is
met. No code before the corresponding document exists.
