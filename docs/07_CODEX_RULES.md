# AxiomRT Codex Rules

Document ID: AXIOM-DOC-008
Status: Approved for Phase 0

These rules govern every AI-assisted implementation task (Codex or any other
code assistant) on AxiomRT. They make the assistant an implementation
assistant only — never an architect.

## 1. Role of Codex

Codex implements precisely specified tasks. Nothing else.

* Codex receives a task with a Task ID and executes exactly that task.
* Codex does not design. Architecture comes from the Phase 0 documents.
* Codex does not extend scope. If a task seems to require an architectural
  decision that is not specified, Codex must stop and report — not decide.
* Codex output is always reviewed by a human against the checklist in
  section 4 before it is accepted.

## 2. Forbidden Actions

Codex must not:

* invent architecture
* add features outside the task
* modify files outside the allowed list
* touch files on the forbidden list
* add dependencies without explicit approval
* remove tests
* weaken safety checks or assertions
* silence or hide compiler errors and warnings
* create broad refactors
* add unsafe Rust without written justification (see section 6)
* add heap allocation inside the kernel after boot
* change the syscall ABI without updating docs/04_SYSCALL_MODEL.md first

## 3. Required Task Format

No Codex task may run unless it contains all of:

* **Task ID** (format `AXIOM-AREA-NNN`)
* **requirement reference** (which Phase 0 document/section it implements)
* **allowed files** (exhaustive list)
* **forbidden files** (explicit, or "all other files")
* **expected behavior** (observable outcome)
* **tests required** (or explicit "no tests for this task" with reason)
* **documentation update** (which doc, or explicit "none")
* **definition of done** (checkable conditions)
* **rollback condition** (when and how to revert)

A task missing any element is rejected before execution.

## 4. Review Checklist

After each Codex result, the reviewer checks:

1. Did Codex modify only allowed files?
2. Did Codex touch forbidden files?
3. Did Codex invent architecture?
4. Did Codex add dependencies?
5. Did Codex add unsafe code?
6. Did Codex add a broad refactor?
7. Did Codex remove tests?
8. Did Codex weaken a check?
9. Did Codex update docs if required?
10. Did Codex implement exactly the task and nothing more?
11. Does the change build?
12. Are errors visible?
13. Is rollback simple?

Reject the change if any answer is wrong.

## 5. Commit Rules

* One commit per task; never commit multiple phases together.
* Commit message format: `AXIOM-AREA-NNN: short imperative summary`.
* The commit must leave the build green (or reach the documented expected
  state for that phase).
* A rejected change is reverted, not patched over.

## 6. Unsafe Code Policy

* Default: no `unsafe` Rust.
* `unsafe` is allowed only when the task explicitly permits it **and** the
  hardware/ABI forces it (MMIO access, CSR access, inline assembly, linker
  symbols, context switching).
* Every `unsafe` block carries a `// SAFETY:` comment stating: why unsafe is
  required, what invariant makes it sound, and which document defines that
  invariant.
* `unsafe` blocks are kept minimal — one obligation per block; no broad
  unsafe functions where an unsafe block suffices.
* Undocumented unsafe code is rejected in review, regardless of whether it
  works.

## 7. Dependency Policy

* Zero external runtime dependencies in the kernel by default.
* Any new dependency (including build tooling) requires explicit approval
  recorded in the task text before Codex may add it.
* Approved dependencies must be pinned to exact versions.
* Dev-dependencies for host-side tests follow the same approval rule.
* No dependency may introduce hidden allocation, threads, or I/O into the
  kernel.

## 8. Documentation Policy

* No code before the corresponding document exists.
* If a task changes behavior described in a document, the document is
  updated in the same task (documentation update is part of the allowed
  files list).
* Documentation states facts and rules, not aspirations: no certification
  claims, no "bug-free"/"never fails" language (docs/01_SCOPE_AND_NON_GOALS.md,
  charter section 2).
* Every implementation file must be traceable to a Phase 0 document; if a
  reviewer cannot trace it, it is rejected (charter section 8, Fundamental
  Rule).
