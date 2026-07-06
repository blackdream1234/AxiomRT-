# AxiomRT v1.0 — Verification Report

## 1. Approach

AxiomRT is verified at three levels, with the boundary between them made
explicit:

1. **Documented rules** (Phase 0 blueprint) — enforceable, testable
   statements of every mechanism.
2. **Typed Rust models + tests** — each mechanism is a typed state
   machine with explicit errors, covered by positive and negative tests.
3. **Coq starter models** — the core safety properties stated and proven
   at model level, with refinement-to-code obligations stated as
   explicit `TODO`s.

No proof claim exceeds the verified relation (LIMITATIONS.md).

## 2. Formal Models (Coq 8.20)

| File | Property proven (model level) | Refinement to code |
|---|---|---|
| proofs/coq/MemoryIsolation.v | no read/write of an unmapped address; distinct tasks never share a frame (no-sharing); Sv39 leaf-PTE keeps kernel frames non-user and user leaves W^X | TODO (stated) |
| proofs/coq/CapabilityAccess.v | no invocation without a valid capability of sufficient rights; rights never grow by derivation | TODO (stated) |
| proofs/coq/SchedulerPriority.v | a higher-priority ready task excludes a lower-priority selection; non-ready tasks never selected; a concrete selection function satisfies the spec | TODO (stated) |

All three compile cleanly with `coqc`.

## 3. On-Target Property Evidence (QEMU)

| Property | Test | Result |
|---|---|---|
| Boot + banner | boot_smoke_test | PASS |
| Memory isolation (read kernel / write unmapped / exec non-exec) | memory_isolation_qemu_test | PASS |
| Multi-task dispatch | two_task_qemu_test | PASS |
| Preemption (runaway cannot freeze) | timer_preemption_qemu_test | PASS |
| Watchdog (CPU exhaustion contained) | watchdog_qemu_test | PASS |
| Synchronous copy-based IPC | ipc_rendezvous_qemu_test | PASS |
| Capability enforcement (deny-by-default) | capability_qemu_test | PASS |
| Supervisor/logger fault recovery | supervisor_qemu_test | PASS |
| Full four-task fault-containment demo | full_fault_containment_demo | PASS |

## 4. Host Test Evidence

* Kernel host unit + integration tests: **125 passing** (memory model,
  thread lifecycle, scheduler, IPC, capabilities, fault handling,
  monitoring, wire format).
* Supervisor crate tests: **4 passing** (decision logic + capability
  bypass rejection).

## 5. Traceability

Every implementation file references its Phase 0 document; every commit
maps to exactly one task ID (`AXIOM-*`); each version stage is a git tag
(`v0.1-final` … `v0.9-demo`) with an `evidence/<version>/` archive.

## 6. Open Verification Obligations

* Discharge the model↔code refinement `TODO`s (v1.4).
* Fuzzing, coverage, static analysis, independent review (v1.5+).
* Real-hardware re-validation of the isolation and containment
  properties (v1.1).
