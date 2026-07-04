# AxiomRT Scope and Non-Goals

Document ID: AXIOM-DOC-002
Status: Approved for Phase 0

This document exists to prevent scope creep. Any task that proposes work
outside "Scope v0.1" must be rejected or deferred to v0.2+.

## 1. Scope v0.1

AxiomRT v0.1 contains exactly the following:

* RISC-V 64 QEMU boot (through OpenSBI)
* minimal microkernel (Rust `no_std`, minimal RISC-V assembly)
* isolated user tasks (one address space per task)
* synchronous IPC (bounded, copy-based)
* capabilities (capability-based access control on all protected objects)
* fixed-priority scheduler (preemptive, deterministic tie-breaking)
* watchdog supervisor (trusted user-space recovery service)
* fault events (structured, delivered to the supervisor)

Supporting deliverables in scope:

* build and boot documentation
* QEMU run scripts and boot smoke test
* unit tests for scheduler, IPC, and capabilities
* fault-injection demo scenario
* formal proof starter models (theorem statements with explicit assumptions)
* industrial evaluation kit documentation

## 2. Explicit Non-Goals v0.1

The following are not goals of v0.1 and must not be implemented:

* GUI
* filesystem
* network stack
* POSIX
* dynamic drivers
* desktop use
* multicore
* hardware certification claim
* AI inside kernel

## 3. Future Scope v0.2+

Candidates for later versions, only after v0.1 is complete and gated:

* user-space driver framework
* shared-memory IPC with proof-backed rules
* mixed-criticality and budget-based scheduling
* temporal partitioning and WCET-aware scheduling
* multicore support
* board support packages for real hardware
* completed formal proofs (beyond theorem statements)
* safety evidence package aligned with certification standards

## 4. Forbidden Early Features

These features are forbidden until explicitly re-scoped, because they enlarge
the trusted computing base or invalidate the verification approach:

* shell, package manager, user accounts
* dynamic kernel modules or dynamic driver loading
* shared memory IPC in v0.1
* heap allocation inside the kernel after boot
* cryptographic protocols inside the kernel
* logging storage backend inside the kernel
* AI decision-making inside the kernel
* any POSIX compatibility layer

## 5. Rationale

The value of AxiomRT is a small, verifiable, deterministic trusted computing
base with certification-oriented evidence. Every excluded feature either:

1. enlarges the kernel beyond what can be audited and formally modeled,
2. introduces nondeterminism that breaks scheduling and fault analysis, or
3. creates certification claims that v0.1 cannot support honestly.

Scope discipline is a safety mechanism. A small kernel that provably enforces
isolation is worth more than a large kernel with unverifiable features.
