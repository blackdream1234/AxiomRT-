# AxiomRT Verification Plan

Document ID: created by AXIOM-PROOF-001 (Phase 12)
Requirement reference: Project Description §20, docs/00_PROJECT_CHARTER.md §5.

(Naming note: the file number 11 collides with 11_RUNTIME_MONITORING.md;
both names are fixed verbatim by the task pack.)

## 1. Approach

AxiomRT is designed for formal verification from the beginning:

1. **Phase 0 documents** state enforceable rules (memory model,
   capability model, scheduler rules, fault model).
2. **Rust model modules** (Phases 4–11) realize those rules as typed
   state machines with explicit errors, unit-tested against positive
   and negative cases.
3. **Coq starter models** (this phase) state the same rules as
   theorems over small mathematical models, with explicit assumptions.
4. **Refinement obligations** (post-v0.1) connect the Coq models to
   the implementation — each is already stated and marked TODO in its
   proof file, so the gap is visible, never implicit.

Proofs may start as models and theorem statements, then become
complete over time (Project Description §20).

## 2. Proof Targets v0.1

| Target | Statement | File | Status |
|---|---|---|---|
| Memory isolation | a task cannot read/write an address not mapped in its address space; distinct tasks never reach the same frame under no-sharing | proofs/coq/MemoryIsolation.v | theorems proven at model level; refinement TODO |
| Capability access | a task cannot invoke a protected object without a valid capability with sufficient rights; rights never grow by derivation | proofs/coq/CapabilityAccess.v | theorems proven at model level; refinement TODO |
| Scheduler priority | if a high-priority ready task exists, a lower-priority task is not selected; non-ready tasks are never selected | proofs/coq/SchedulerPriority.v | spec-level theorems proven; concrete-function refinement TODO |
| Fault containment | a user-space fault cannot corrupt kernel state | (v0.2+, after MMU activation) | planned |
| Syscall validation | invalid syscall arguments are rejected before use | (v0.2+) | planned |

## 3. Assumptions of the v0.1 Models (explicit)

* One address space per task; one thread per task (v0.1 structure).
* Single hart: no concurrent mutation of kernel state.
* The MMU enforces exactly the model mappings once activated
  (refinement obligation; hardware correctness itself is an assumption
  of use, docs/INDUSTRIAL_EVALUATION_KIT.md when it lands).
* Capabilities exist only in kernel memory (unforgeability is
  structural, docs/06_CAPABILITY_MODEL.md §1) — the models therefore
  treat the capability table as the only source of authority.
* OpenSBI/M-mode behavior is outside the claim boundary.

## 4. Method Rules

* Every theorem names its kernel counterpart (docs section and Rust
  module) in a comment.
* `Admitted` only with an explicit `TODO` comment naming the missing
  argument; CI treats a non-compiling proof file as a broken build.
* Model changes require re-checking the corresponding Phase 0
  document in the same task (docs/07_CODEX_RULES.md §8).
