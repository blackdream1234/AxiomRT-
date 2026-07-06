# AxiomRT

## 1. What is AxiomRT?

AxiomRT is a formally specified microkernel-based safety runtime for
high-assurance embedded systems that require strong isolation, deterministic
execution, controlled fault recovery, and certification-oriented evidence.

The design principle is: **small trusted kernel, isolated user-space
services, explicit authority, controlled failure.**

The first product target is the **AxiomRT Safety Core Industrial Evaluation
Kit** — an evaluation-stage kit for safety-oriented embedded runtime
research and prototype evaluation. It is not sold as a certified OS, and no
certification claim is made.

## 2. What AxiomRT is not

AxiomRT v0.1 is not a Linux clone, not a desktop OS, not a mobile OS, not a
full QNX clone, not a POSIX system, and not a certified product. It is not
aircraft-ready and not vehicle-ready.

v0.1 deliberately excludes: GUI, filesystem, network stack, POSIX layer,
shell, package manager, dynamic kernel modules, user accounts, multicore
support, shared memory IPC, and AI inside the kernel. See
docs/01_SCOPE_AND_NON_GOALS.md.

## 3. Current phase

**v0.2 — Sv39/MMU hardware memory isolation** (on the completion
roadmap, `Full Completion Mode.md`).

The kernel boots on QEMU/OpenSBI, activates the Sv39 MMU, and runs a
user task under its own page table: a user attempt to read kernel
memory, write an unmapped address, or execute a non-executable page
takes a hardware page fault that is contained while the kernel
survives (tests/memory_isolation_qemu_test.sh). Model layers (threads,
scheduler, IPC, capabilities, fault recovery, monitoring) are
host-tested; Coq starter models compile with core theorems proven.
Remaining gaps (multi-task dispatch, timer preemption, watchdog,
on-target IPC/supervisor): tracked stage by stage in
`Full Completion Mode.md` §12+. Demo: docs/DEMO_SCENARIO.md; evidence:
evidence/v0.1, evidence/v0.2.

## 4. Target platform

* Architecture: RISC-V 64 (RV64GC)
* Platform: QEMU `virt` machine (emulator only in v0.1)
* Boot firmware: OpenSBI
* Kernel language: Rust `no_std`, minimal RISC-V assembly
* Deployment target v0.1: emulator only

## 5. Architecture direction

Microkernel architecture. The kernel provides only controlled mechanisms:
boot, traps, interrupts, address spaces, threads, fixed-priority scheduling,
synchronous copy-based IPC, capability-based access control, syscall
validation, and fault events. Everything complex — drivers, logging, health
monitoring, supervision policy — runs isolated in user space.

```text
+--------------------------------------------------+
|                User-Space Services               |
| supervisor | logger | critical task | faulty task|
+--------------------------------------------------+
                    | syscalls / IPC
+--------------------------------------------------+
|                   AxiomRT Kernel                 |
| traps | syscalls | scheduler | IPC | capabilities|
| address spaces | threads | faults | timers       |
+--------------------------------------------------+
|             RISC-V 64 / QEMU / OpenSBI           |
+--------------------------------------------------+
```

## 6. Safety rule

Every line of code must be traceable to a requirement, a design decision, a
safety rule, a test, and a verification objective. If a feature cannot be
traced, it does not belong in the system.

Core v0.1 guarantees: memory isolation, capability-controlled access,
deterministic scheduling, and controlled user-space fault recovery. A user
task fault must never crash the kernel.

## 7. Codex rule

No code before the corresponding document exists. AI assistants are
implementation assistants only: they execute precisely scoped tasks (Task
ID, allowed files, forbidden files, definition of done) and never invent
architecture, add dependencies, remove tests, or weaken checks. Full rules:
docs/07_CODEX_RULES.md.

## 8. Phase map

```text
Phase 0:  Kernel Blueprint            (complete — gated)
Phase 1:  Repository Skeleton         (current)
Phase 2:  Boot Kernel in QEMU
Phase 3:  Trap and Exception Layer
Phase 4:  Memory Isolation
Phase 5:  Thread Model
Phase 6:  Scheduler
Phase 7:  User Mode
Phase 8:  IPC
Phase 9:  Capabilities
Phase 10: Fault Recovery
Phase 11: Runtime Monitoring
Phase 12: Formal Proof Starter
Phase 13: Industrial Evaluation Kit
```

Documentation index: docs/INDEX.md. Start at docs/00_PROJECT_CHARTER.md.
