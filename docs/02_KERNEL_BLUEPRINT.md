# AxiomRT Kernel Blueprint

Document ID: AXIOM-DOC-003
Status: Approved for Phase 0

## 1. Kernel Identity

AxiomRT is a microkernel for high-assurance embedded systems. The kernel is a
mechanism provider, not a policy engine. It enforces isolation, controls
authority, schedules deterministically, and contains faults. Policy (recovery
decisions, logging, health monitoring) lives in trusted user-space services.

## 2. Kernel Rule

The kernel does not provide high-level policy. The kernel provides controlled
mechanisms. Policy belongs in user space.

Corollaries:

* The kernel never decides *whether* to restart a task; the supervisor does.
* The kernel never interprets log content; it only emits structured events.
* The kernel never grants authority implicitly; all authority is a capability.

## 3. Kernel Responsibilities

The kernel is responsible for exactly the following in v0.1:

* boot entry
* trap handling
* interrupt dispatching
* address space management
* thread management
* fixed-priority scheduling
* synchronous IPC
* capability lookup
* syscall validation
* fault event delivery

## 4. Kernel Non-Responsibilities

The kernel must not contain:

* filesystem
* network stack
* GUI
* shell
* package manager
* POSIX layer
* AI logic
* logging storage backend

These belong in user space or outside v0.1 entirely.

## 5. Target Platform

* Architecture: RISC-V 64 (RV64GC)
* Platform: QEMU `virt` machine (emulator only in v0.1)
* Boot firmware: OpenSBI (kernel runs in supervisor mode, entered from OpenSBI)
* Kernel language: Rust `no_std`, no heap after boot
* Assembly: minimal RISC-V assembly (boot entry, trap entry/exit, context
  save/restore, user entry)

## 6. Kernel Object Model

All kernel state is organized as explicit kernel objects:

```text
KernelObject
Thread
AddressSpace
PhysicalFrame
PageTable
Endpoint
Message
Capability
SchedulingContext
Timer
FaultEvent
```

Each object has an ID, an owner, a lifecycle, valid states, allowed
operations, invalid operations, security impact, and failure behavior.
Object definitions live in docs/03_KERNEL_OBJECTS.md. No object may have
vague responsibility.

## 7. Trust Boundary

* The kernel is the only code running at supervisor privilege.
* All user tasks, including the supervisor service, run at user privilege.
* The kernel/user boundary is crossed only through explicit trap paths:
  syscalls, exceptions, and interrupts.
* The supervisor task is trusted for recovery *policy* only. It cannot bypass
  capability checks or isolation. Trust in policy is not trust in mechanism.
* OpenSBI (machine mode) is a boot and SBI-service dependency; it is outside
  the AxiomRT trusted computing base claim but inside the platform
  assumptions of use.

## 8. Memory Principle

* Kernel memory is never user-accessible.
* Each task has its own address space.
* A task cannot access another task's memory (no shared memory in v0.1).
* Device memory requires an explicit capability.
* Invalid access creates a page fault, which suspends or kills the offending
  task and produces a fault event.
* Full model: docs/05_MEMORY_MODEL.md.

## 9. IPC Principle

* IPC is synchronous, bounded, and copy-based.
* Send requires a capability with Send rights; receive requires Receive
  rights.
* Messages have a fixed maximum size; the kernel copies message data between
  address spaces.
* No shared memory IPC in v0.1.

## 10. Scheduling Principle

* Fixed-priority preemptive scheduling.
* A high-priority ready thread always runs before a low-priority ready
  thread.
* Blocked, killed, and (unrecovered) faulted threads are never selected.
* Tie-breaking between equal priorities is deterministic and documented.
* A low-priority faulty task must not be able to freeze the system.

## 11. Fault Principle

* Faults are first-class structured events, never silent failures.
* A user-space fault is contained: the faulting thread is stopped or
  suspended, a FaultEvent is created, and the supervisor is notified.
* A kernel invariant violation halts the system safely (controlled panic).
* Full model: docs/06_FAULT_MODEL.md.

## 12. First Demonstration

Four user tasks run on the kernel in QEMU: `critical_task`, `logger_task`,
`faulty_task`, `supervisor_task`. The faulty task attacks the system (illegal
syscall, illegal memory access, illegal IPC, CPU exhaustion, repeated crash).
The critical task keeps running, the supervisor receives fault events, the
logger receives structured events, and the kernel stays stable.

## 13. Forbidden Early Design Choices

The following design choices are forbidden in v0.1:

* shared memory between tasks
* kernel heap allocation after boot
* dynamic driver or module loading
* asynchronous/buffered IPC
* multicore scheduling
* kernel-resident policy (recovery decisions, log storage, AI)
* raw object access by ID or pointer without capability lookup
* undocumented unsafe Rust

## 14. Phase 0 Exit Criteria

Phase 0 is complete only when all conditions in docs/08_PHASE_0_GATE.md are
satisfied: charter, scope, blueprint, kernel objects, syscall model, memory
model, fault model, and Codex rules all exist, and no code has been written.
Only then may Phase 1 (repository skeleton) begin.
