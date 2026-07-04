# AxiomRT Phase 0 Gate

Document ID: AXIOM-DOC-009
Status: Active gate

**Coding is blocked until every box below is checked.** Phase 1 (repository
skeleton) is allowed only after all checks pass. The first real kernel code
begins only in Phase 2, which additionally requires Phase 1 completion.

## Gate Checklist

* [x] **Project charter exists** — docs/00_PROJECT_CHARTER.md defines
  mission, product boundary, final goal, first target, core guarantees,
  non-goals, first demonstration, and the engineering rule.
* [x] **Scope is defined** — docs/01_SCOPE_AND_NON_GOALS.md section 1 lists
  exactly what v0.1 contains.
* [x] **Non-goals are explicit** — docs/01_SCOPE_AND_NON_GOALS.md section 2
  lists the v0.1 exclusions (GUI, filesystem, network, POSIX, dynamic
  drivers, desktop, multicore, certification claims, AI in kernel).
* [x] **Kernel blueprint exists** — docs/02_KERNEL_BLUEPRINT.md defines
  kernel identity, rule, responsibilities, non-responsibilities, platform,
  object model, trust boundary, memory/IPC/scheduling/fault principles,
  first demonstration, forbidden design choices, and exit criteria.
* [x] **Kernel objects are defined** — docs/03_KERNEL_OBJECTS.md defines all
  eleven objects (KernelObject, Thread, AddressSpace, PhysicalFrame,
  PageTable, Endpoint, Message, Capability, SchedulingContext, Timer,
  FaultEvent), each with purpose, owner, lifecycle, valid states, allowed
  operations, invalid operations, failure behavior, and security impact.
* [x] **Syscall model is defined** — docs/04_SYSCALL_MODEL.md defines all
  seven v0.1 syscalls with precise validation rules, and the forbidden
  syscall list.
* [x] **Memory model is defined** — docs/05_MEMORY_MODEL.md defines address
  spaces, kernel/user memory, frames, page tables, permissions, device
  memory, page fault behavior, forbidden features, and verification
  properties.
* [x] **Fault model is defined** — docs/06_FAULT_MODEL.md defines all eight
  fault types with source, severity, kernel action, notification, recovery
  options, and logging fields; no undefined behavior remains.
* [x] **Codex rules exist** — docs/07_CODEX_RULES.md defines the assistant
  role, forbidden actions, task format, review checklist, commit rules, and
  the unsafe/dependency/documentation policies.
* [x] **No code was written** — the repository contains documentation only
  at the end of Phase 0: no Rust crates, no assembly, no linker scripts, no
  QEMU scripts.

## Gate Rule

* Phase 1 is allowed **only after all checks above pass**.
* Any Phase 0 document change after the gate closes reopens the gate: the
  checklist must be re-verified before further implementation tasks run.
* The reviewer confirming this gate records the confirmation in the commit
  history (`AXIOM-DOC-009: add phase 0 gate`), which marks Phase 0 complete.
