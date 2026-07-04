# AxiomRT — Full Project Description

## 1. Project Name

**AxiomRT**

Temporary full name:

**AxiomRT Safety Core**

## 2. One-Line Definition

AxiomRT is a formally specified microkernel-based safety runtime for high-assurance embedded systems that require strong isolation, deterministic execution, controlled fault recovery, and certification-oriented evidence.

## 3. Final Goal

The final goal is to build a small, reliable, safety-oriented operating system core that can later become an industrial product for:

* automotive embedded systems
* aerospace embedded systems
* drones
* robotics
* autonomous systems
* critical embedded prototypes
* safety/security research labs
* industrial control systems

AxiomRT is not intended to start as a full desktop operating system.

The first commercial target is not a complete OS.

The first commercial target is:

**AxiomRT Safety Core Industrial Evaluation Kit**

This kit will allow companies, labs, and engineering teams to evaluate a small safety runtime that demonstrates:

* task isolation
* memory isolation
* deterministic scheduling
* capability-based access control
* fault containment
* supervisor-based recovery
* structured runtime monitoring
* fault-injection testing
* verification-oriented documentation
* formal proof starter models
* safety evidence preparation

## 4. What We Are Building

We are building a minimal microkernel-based safety runtime.

The kernel will be small. It will not contain complex services.

The kernel will only enforce the fundamental mechanisms needed for safety and security:

* boot
* traps
* interrupts
* address spaces
* threads
* scheduling
* IPC
* capabilities
* syscall validation
* fault events
* controlled recovery support

Everything complex must run outside the kernel in user space.

This includes:

* drivers
* file systems
* network stack
* logging services
* health monitoring
* supervision logic
* future AI-assisted diagnostics
* future update manager

The design principle is:

**Small trusted kernel, isolated user-space services, explicit authority, controlled failure.**

## 5. What AxiomRT Is Not

AxiomRT v0.1 is not:

* a Linux clone
* a Windows competitor
* a desktop OS
* a mobile OS
* a full QNX clone
* a POSIX system
* a browser platform
* a general-purpose server OS
* a certified product
* an aircraft-ready product
* a vehicle-ready product

AxiomRT v0.1 will not include:

* GUI
* filesystem
* network stack
* POSIX layer
* shell
* package manager
* dynamic kernel modules
* user accounts
* desktop environment
* multicore support
* shared memory IPC
* AI inside the kernel

These are intentionally excluded to keep the trusted computing base small and verifiable.

## 6. First Technical Target

The first implementation target is:

* Architecture: RISC-V 64
* Platform: QEMU emulator
* Boot firmware: OpenSBI
* Kernel language: Rust `no_std`
* Assembly: minimal RISC-V assembly
* Scheduler v0.1: fixed-priority preemptive scheduler
* IPC v0.1: synchronous copy-based IPC
* Security model: capability-based access control
* Verification start: Coq or Isabelle/HOL model
* Deployment target v0.1: emulator only

RISC-V and QEMU are chosen because they are simpler for early development, easier to inspect, and better for building a clean verification path.

## 7. Core Architecture

AxiomRT follows a microkernel architecture.

```text
+--------------------------------------------------+
|                User-Space Services               |
|--------------------------------------------------|
| supervisor | logger | driver mgr | health monitor|
| critical task | non-critical task | faulty task   |
+--------------------------------------------------+
                    |
                    | syscalls / IPC
                    v
+--------------------------------------------------+
|                   AxiomRT Kernel                 |
|--------------------------------------------------|
| traps | syscalls | scheduler | IPC | capabilities|
| address spaces | threads | faults | timers       |
+--------------------------------------------------+
                    |
                    v
+--------------------------------------------------+
|             RISC-V 64 / QEMU / OpenSBI           |
+--------------------------------------------------+
```

The kernel does not provide high-level policy.

The kernel provides controlled mechanisms.

Policy belongs in user space.

## 8. Kernel Responsibilities

The kernel is responsible for:

### Boot

The kernel must start from a controlled boot path and initialize only the minimum required state.

### Trap Handling

The kernel must handle exceptions, interrupts, page faults, illegal instructions, and syscalls through explicit trap paths.

### Address Space Management

The kernel must separate memory between the kernel and user tasks.

Each task must have its own address space.

### Thread Management

The kernel must represent execution contexts as thread objects with explicit states.

Thread states include:

* Ready
* Running
* Blocked
* Faulted
* Killed
* Suspended

### Scheduling

The kernel must select which thread runs.

The first scheduler is a fixed-priority preemptive scheduler.

Later versions may include mixed-criticality scheduling and budget-based temporal isolation.

### IPC

The kernel must allow controlled communication between tasks.

The first IPC model is synchronous and copy-based.

Shared memory is forbidden in v0.1.

### Capabilities

The kernel must control access to every protected object through capabilities.

A task cannot access an object simply because it knows an ID or address.

It must own a valid capability with sufficient rights.

### Fault Handling

The kernel must contain user-space faults.

A user task fault must not crash the kernel.

The kernel must create structured fault events and notify the supervisor.

### Syscall Validation

Every syscall must validate all arguments before use.

Invalid syscalls must fail in a controlled way.

## 9. Kernel Non-Responsibilities

The kernel must not contain:

* filesystem logic
* network logic
* GUI logic
* shell logic
* package management
* AI decision-making
* logging storage backend
* cryptographic protocols
* user account system
* dynamic driver loading

This keeps the kernel small enough to verify and audit.

## 10. Kernel Object Model

The first kernel objects are:

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

Each object must have:

* ID
* owner
* lifecycle
* valid states
* allowed operations
* invalid operations
* security impact
* failure behavior

No kernel object should have vague responsibility.

## 11. Capability-Based Security

AxiomRT uses capability-based access control.

A capability is an explicit authority token.

Protected objects include:

* threads
* endpoints
* address spaces
* physical frames
* timers
* scheduling contexts
* fault channels

Capability rights include:

* Read
* Write
* Execute
* Send
* Receive
* Grant
* Map
* Control

Example:

A task cannot send a message to an endpoint unless it owns a capability with `Send` rights for that endpoint.

A task cannot map memory unless it owns a capability with `Map` rights for the frame or region.

A task cannot control another task unless it owns a capability with `Control` rights.

This enforces least privilege.

## 12. Memory Model

The memory model must enforce:

* kernel memory is never user-accessible
* each task has its own address space
* a task cannot access another task’s memory
* device memory requires an explicit capability
* invalid memory access creates a page fault
* page faults are handled in a controlled way
* shared memory is forbidden in v0.1

The first verification target is:

**No user task can access kernel memory.**

The second verification target is:

**No user task can access another task’s memory without explicit mapping authority.**

## 13. Syscall Model

The first syscalls are:

```text
sys_yield
sys_exit
sys_send
sys_recv
sys_reply
sys_cap_query
sys_fault_ack
```

Forbidden syscalls in v0.1:

```text
open
read file
write file
socket
fork
exec
shared mmap
```

Each syscall must define:

* purpose
* arguments
* required capability
* validation rule
* success result
* failure result
* fault behavior
* security rule

No syscall may operate directly on raw object pointers.

All object access must go through capability lookup.

## 14. IPC Model

IPC v0.1 is:

* synchronous
* bounded
* copy-based
* capability-controlled

A task can send only if it owns a `Send` capability.

A task can receive only if it owns a `Receive` capability.

Messages have a fixed maximum size.

Shared memory IPC is forbidden in v0.1 because it increases proof complexity.

## 15. Scheduling Model

The first scheduler is:

**Fixed-priority preemptive scheduler**

Rules:

* high-priority ready task must run before low-priority ready task
* blocked tasks cannot be selected
* killed tasks cannot be selected
* faulted tasks cannot continue unless recovered
* deterministic tie-breaking must exist
* low-priority faulty tasks must not freeze the system

Later schedulers may include:

* deadline scheduling
* budget-based scheduling
* mixed-criticality scheduling
* temporal partitioning
* WCET-aware scheduling

## 16. Fault Model

AxiomRT must treat faults as first-class events.

Fault types include:

```text
IllegalSyscall
InvalidCapability
PageFault
IllegalInstruction
WatchdogTimeout
DeadlineMiss
IPCViolation
KernelInvariantViolation
```

For each fault, the system must define:

* source
* severity
* kernel action
* supervisor notification
* recovery options
* logging fields

Recovery options include:

```text
Kill
Restart
Suspend
Quarantine
Escalate
KernelPanic
```

Kernel faults and user faults are treated differently.

A user fault should be contained.

A kernel invariant violation should halt safely.

## 17. Supervisor Model

The supervisor is a trusted user-space service.

It receives fault events from the kernel.

It applies recovery policy.

It may decide to:

* restart a task
* kill a task
* suspend a task
* quarantine a task
* escalate the fault
* preserve critical task execution

The supervisor must not bypass kernel capability checks.

It is trusted for policy, not for violating isolation.

## 18. Runtime Monitoring

AxiomRT must produce structured runtime events.

Events include:

```text
TASK_STARTED
TASK_EXITED
TASK_FAULTED
CAP_DENIED
IPC_DENIED
PAGE_FAULT
DEADLINE_MISSED
WATCHDOG_TIMEOUT
RECOVERY_APPLIED
```

Each event should include:

* timestamp
* task ID
* event type
* severity
* kernel phase
* policy result
* related capability if relevant
* related syscall if relevant

In v0.1, events may be exported through serial output in QEMU.

No filesystem is required.

## 19. First Demonstration

The first full demo must include:

```text
critical_task
logger_task
faulty_task
supervisor_task
```

The faulty task must attempt:

* illegal syscall
* illegal memory access
* illegal IPC
* CPU exhaustion
* repeated crash

Expected result:

* critical_task continues
* faulty_task is blocked, killed, or restarted
* supervisor receives fault event
* logger receives structured event
* kernel remains stable
* no unauthorized memory access occurs
* no invalid IPC succeeds

This demo is the first proof that the system is meaningful.

## 20. Formal Verification Direction

AxiomRT must be designed for formal verification from the beginning.

The first proof targets are:

### Memory Isolation

A task cannot read or write memory outside its mapped address space.

### Capability Access

A task cannot invoke a protected object without a valid capability with sufficient rights.

### Scheduler Priority

If a high-priority ready task exists, a lower-priority task is not selected.

### Fault Containment

A user-space fault cannot corrupt kernel state.

### Syscall Validation

Invalid syscall arguments are rejected before use.

First proof files:

```text
proofs/coq/MemoryIsolation.v
proofs/coq/CapabilityAccess.v
proofs/coq/SchedulerPriority.v
proofs/coq/FaultContainment.v
```

Proofs may start as models and theorem statements, then become complete over time.

## 21. Testing Strategy

Testing must include:

* unit tests
* model tests
* QEMU boot tests
* syscall tests
* IPC tests
* capability tests
* scheduler tests
* memory fault tests
* fault-injection tests
* regression tests
* fuzzing later
* static analysis later

First mandatory test:

**QEMU boot smoke test**

It must verify that the kernel prints:

```text
AxiomRT kernel booted
arch=riscv64
phase=boot
```

## 22. Codex Role

Codex is an implementation assistant only.

Codex must not:

* invent architecture
* add features outside the task
* modify forbidden files
* add dependencies without approval
* remove tests
* weaken checks
* silence compiler errors
* create broad refactors
* add unsafe Rust without written justification
* add heap allocation inside the kernel after boot
* change syscall ABI without updating documentation

Every Codex task must include:

* Task ID
* requirement reference
* allowed files
* forbidden files
* expected behavior
* tests required
* documentation update
* definition of done
* rollback condition

Bad prompt:

```text
Build the operating system.
```

Correct prompt:

```text
Implement AXIOM-BOOT-003 only.
Modify only the allowed files.
Do not add scheduler, memory manager, IPC, or capabilities.
```

## 23. Phase Plan

### Phase 0 — Kernel Blueprint

Create all documents before code.

Outputs:

```text
00_PROJECT_CHARTER.md
01_SCOPE_AND_NON_GOALS.md
02_KERNEL_BLUEPRINT.md
03_KERNEL_OBJECTS.md
04_SYSCALL_MODEL.md
05_MEMORY_MODEL.md
06_FAULT_MODEL.md
07_CODEX_RULES.md
08_PHASE_0_GATE.md
```

### Phase 1 — Repository Skeleton

Create repository structure, README, documentation index, placeholders.

No kernel logic.

### Phase 2 — Boot Kernel in QEMU

Create the minimal Rust `no_std` kernel.

Add boot entry, linker script, UART output, QEMU run script, boot smoke test.

### Phase 3 — Trap and Exception Layer

Add trap vector, exception decoder, illegal instruction handler, syscall trap stub.

### Phase 4 — Memory Isolation

Add address types, physical frame model, page table model, kernel/user memory ranges.

### Phase 5 — Thread Model

Add thread objects, thread states, register context model.

### Phase 6 — Scheduler

Add fixed-priority scheduler and deterministic selection tests.

### Phase 7 — User Mode

Run first user task outside kernel privilege.

### Phase 8 — IPC

Add synchronous copy-based IPC between tasks.

### Phase 9 — Capabilities

Add capability table, rights checking, capability-controlled IPC.

### Phase 10 — Fault Recovery

Add fault events, user fault containment, supervisor notification.

### Phase 11 — Runtime Monitoring

Add structured runtime events and serial event export.

### Phase 12 — Formal Proof Starter

Create Coq or Isabelle models for memory isolation, capability access, and scheduler priority.

### Phase 13 — Industrial Evaluation Kit

Package documentation, demo, test reports, limitations, assumptions of use, and verification notes.

## 24. Industrial Product Direction

The first product is:

**AxiomRT Safety Core Industrial Evaluation Kit**

It is not sold as a certified OS.

It is sold as an evaluation kit for safety-oriented embedded runtime research and prototype evaluation.

The kit should contain:

* source code
* QEMU image
* build instructions
* demo scenario
* API documentation
* architecture document
* threat model
* safety concept
* test suite
* fault-injection suite
* verification notes
* known limitations
* assumptions of use

## 25. Commercial Positioning

The correct positioning is:

AxiomRT is a formally specified safety microkernel runtime for embedded systems that need strong isolation, deterministic behavior, and certification-oriented evidence.

Do not claim:

* “bug-free”
* “never fails”
* “certified”
* “ready for aircraft”
* “ready for cars”
* “stronger than QNX” as a public claim

Use precise language:

* formally specified
* safety-oriented
* microkernel-based
* high-assurance
* evaluation-stage
* certification-oriented
* designed for isolation and controlled recovery

## 26. Long-Term Vision

Long-term, AxiomRT can evolve into:

* a safety-certified runtime
* an automotive embedded OS component
* an aerospace R&D platform
* a verified microkernel research product
* a secure runtime for drones and robotics
* a safety/security consulting platform
* a commercial high-assurance embedded system core

The correct path is:

```text
small verified runtime
→ industrial evaluation kit
→ paid pilots
→ board support packages
→ safety evidence package
→ certification path
→ commercial embedded runtime
```

## 27. Fundamental Rule

AxiomRT succeeds only if every line of code is traceable to:

* a requirement
* a design decision
* a safety rule
* a test
* a verification objective

If a feature cannot be traced, it does not belong in the system.
