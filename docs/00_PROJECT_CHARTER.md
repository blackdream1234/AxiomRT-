# AxiomRT Project Charter

Document ID: AXIOM-DOC-001
Status: Approved for Phase 0

## 1. Mission

Build AxiomRT: a formally specified microkernel-based safety runtime for
high-assurance embedded systems that require strong isolation, deterministic
execution, controlled fault recovery, and certification-oriented evidence.

AxiomRT is engineered so that every line of code is traceable to a
requirement, a design decision, a safety rule, a test, and a verification
objective. If a feature cannot be traced, it does not belong in the system.

## 2. Product Boundary

AxiomRT is a minimal safety runtime. It is not a general-purpose OS.

AxiomRT is not:

* a Linux clone
* a desktop OS
* a mobile OS
* a full QNX clone
* a POSIX system
* a certified product (no certification claim is made in v0.1)

Everything complex runs outside the kernel in isolated user space. The kernel
provides only the fundamental mechanisms needed for safety and security.

Design principle:

**Small trusted kernel, isolated user-space services, explicit authority,
controlled failure.**

## 3. Final Goal

A small, reliable, safety-oriented operating system core that can later become
an industrial product for:

* automotive embedded systems
* aerospace embedded systems
* drones
* robotics
* autonomous systems
* critical embedded prototypes
* safety/security research labs
* industrial control systems

Long-term path: small verified runtime → industrial evaluation kit → paid
pilots → board support packages → safety evidence package → certification
path → commercial embedded runtime.

## 4. First Target

The first product target is the **AxiomRT Safety Core Industrial Evaluation
Kit**, built on:

* Architecture: RISC-V 64
* Platform: QEMU emulator (emulator only in v0.1)
* Boot firmware: OpenSBI
* Kernel language: Rust `no_std`
* Assembly: minimal RISC-V assembly
* Scheduler v0.1: fixed-priority preemptive scheduler
* IPC v0.1: synchronous copy-based IPC
* Security model: capability-based access control
* Verification start: Coq or Isabelle/HOL model

## 5. Core Guarantees

AxiomRT v0.1 commits to four core guarantees:

1. **Memory isolation.** No user task can access kernel memory. No user task
   can access another task's memory without explicit mapping authority.
2. **Capability-controlled access.** No task can invoke a protected object
   without a valid capability with sufficient rights.
3. **Deterministic scheduling.** A fixed-priority scheduler with explicit,
   deterministic tie-breaking. No hidden scheduling behavior.
4. **Controlled user-space fault recovery.** A user task fault is contained,
   converted into a structured fault event, and delivered to a trusted
   user-space supervisor. A user fault never crashes the kernel.

## 6. Non-Goals v0.1

The following are explicitly excluded from v0.1:

* GUI
* filesystem
* network stack
* POSIX layer
* shell
* package manager
* dynamic drivers / dynamic kernel modules
* desktop use
* user accounts
* multicore support
* shared memory IPC
* hardware certification claim
* AI inside the kernel

These exclusions keep the trusted computing base small and verifiable.

## 7. First Demonstration

The first demonstration runs four user tasks on the kernel in QEMU:

* `critical_task` runs periodically
* `logger_task` receives structured events
* `faulty_task` attempts illegal syscalls, illegal memory access, illegal
  IPC, CPU exhaustion, and repeated crashes
* `supervisor_task` receives fault events and applies recovery policy

Expected result: the critical task continues, the faulty task is blocked,
killed, or restarted, the supervisor receives fault events, the logger
receives structured events, the kernel remains stable, no unauthorized memory
access occurs, and no invalid IPC succeeds.

## 8. Engineering Rule

**No code before the corresponding document exists.**

Every implementation task must reference this charter and the Phase 0
blueprint documents. Every change is scoped by a task ID with allowed files,
forbidden files, expected behavior, required tests, documentation updates, a
definition of done, and a rollback condition.

Phase 0 produces documentation only. The first kernel code begins in Phase 2,
and only after the Phase 0 gate (docs/08_PHASE_0_GATE.md) is fully satisfied.
