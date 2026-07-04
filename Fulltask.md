# AxiomRT Prompt Pack v0.1

## Final Goal

We are building AxiomRT.

AxiomRT is a formally specified microkernel-based safety runtime for high-assurance embedded systems.

Long-term target:

* automotive systems
* aerospace systems
* drones
* robotics
* critical embedded prototypes
* safety/security research labs

AxiomRT is not a Linux clone.
AxiomRT is not a desktop OS.
AxiomRT is not a full QNX clone at the beginning.
AxiomRT v0.1 is a minimal safety runtime.

The first product target is:

AxiomRT Safety Core Industrial Evaluation Kit

It must include:

* minimal microkernel
* RISC-V 64 QEMU boot
* isolated user tasks
* deterministic scheduler
* capability-based access control
* synchronous IPC
* fault containment
* supervisor-based recovery
* logging
* fault-injection tests
* verification-oriented documentation
* formal model starter files
* safety evidence package draft

## Engineering Law

No code before the corresponding document exists.

No Codex task without:

* Task ID
* requirement reference
* allowed files
* forbidden files
* expected behavior
* tests required
* documentation update
* definition of done
* rollback condition

Codex must not invent architecture.

Codex must not add features outside the task.

Codex must not touch files outside the allowed list.

Codex must not add dependencies without permission.

Codex must not create broad refactors.

Codex must not hide build errors.

Codex must not remove tests.

Codex must not weaken safety checks.

Codex must not add unsafe Rust without written justification.

---

# Required Skills

## Core Technical Skills

1. Operating systems internals

   * privilege levels
   * boot process
   * traps
   * interrupts
   * virtual memory
   * context switching
   * scheduling
   * system calls
   * IPC
   * device isolation

2. RISC-V architecture

   * machine mode
   * supervisor mode
   * OpenSBI
   * CSRs
   * page tables
   * trap vector
   * timer interrupts
   * calling convention

3. Rust no_std

   * no standard library
   * panic handler
   * linker scripts
   * unsafe boundaries
   * ownership discipline
   * volatile memory access
   * inline assembly if required
   * embedded Rust patterns

4. Assembly basics

   * boot entry
   * stack setup
   * context save/restore
   * trap entry/exit
   * register convention

5. Microkernel design

   * minimal kernel mechanisms
   * user-space services
   * user-space drivers
   * small trusted computing base
   * object model
   * kernel/user boundary

6. Capability-based security

   * object capabilities
   * access rights
   * capability lookup
   * authority transfer
   * least privilege
   * rights revocation

7. Real-time systems

   * fixed-priority scheduling
   * preemption
   * deadlines
   * watchdogs
   * timing isolation
   * mixed criticality
   * priority inversion

8. Fault tolerance

   * fault detection
   * fault containment
   * supervisor restart
   * quarantine
   * recovery policy
   * fail-safe behavior

9. Formal methods

   * Coq or Isabelle/HOL
   * invariants
   * refinement thinking
   * memory isolation theorem
   * capability theorem
   * scheduler theorem
   * model assumptions

10. Safety engineering

* hazard analysis
* FMEA
* FTA
* safety requirements
* traceability
* assumptions of use
* safety case
* verification evidence

11. Security engineering

* threat modeling
* attack surface reduction
* privilege separation
* syscall validation
* malformed input handling
* denial-of-service containment

12. Testing and verification

* unit tests
* QEMU smoke tests
* property tests
* fuzzing
* fault injection
* regression tests
* coverage
* static analysis

13. Tooling

* Git
* GitHub
* Codex
* QEMU
* OpenSBI
* cargo
* rustup
* objdump
* gdb
* CI
* shell scripting

14. Product and company skills

* technical documentation
* industrial demo design
* safety evidence packaging
* licensing
* customer discovery
* automotive supplier chain
* aerospace R&D entry path
* support model

---

# Phase Map

Phase 0: Kernel Blueprint
Phase 1: Repository Skeleton
Phase 2: Boot Kernel in QEMU
Phase 3: Trap and Exception Layer
Phase 4: Memory Isolation
Phase 5: Thread Model
Phase 6: Scheduler
Phase 7: User Mode
Phase 8: IPC
Phase 9: Capabilities
Phase 10: Fault Recovery
Phase 11: Runtime Monitoring
Phase 12: Formal Proof Starter
Phase 13: Industrial Evaluation Kit

---

# Master Codex System Prompt

Use this before every Codex task.

You are working on AxiomRT, a formally specified microkernel-based safety runtime for high-assurance embedded systems.

AxiomRT is not a general-purpose OS.

AxiomRT v0.1 targets:

* RISC-V 64
* QEMU
* OpenSBI
* Rust no_std
* minimal RISC-V assembly
* microkernel architecture
* capability-based access control
* deterministic scheduling
* controlled fault recovery

Long-term goal:
Build a safety-oriented microkernel runtime that can later support automotive and aerospace industrial evaluation.

Your role:
You are an implementation assistant only.

You must not invent architecture.

You must not add features not requested.

You must not touch files outside the allowed list.

You must not add dependencies without explicit approval.

You must not create broad refactors.

You must not remove tests.

You must not weaken checks.

You must not silence errors.

You must not add unsafe Rust unless the task explicitly allows it and you document the safety reason.

For every task:

* modify only allowed files
* respect forbidden files
* keep changes minimal
* update documentation if requested
* add tests if requested
* stop if the task requires an architectural decision not specified

---

# Phase 0 — Kernel Blueprint

Goal:
Create the engineering foundation before writing code.

Allowed output:
Documentation only.

Forbidden output:
Kernel code, Rust crates, assembly, QEMU scripts, linker scripts.

## AXIOM-DOC-001

Task ID: AXIOM-DOC-001

Goal:
Create docs/00_PROJECT_CHARTER.md.

Files allowed:
docs/00_PROJECT_CHARTER.md

Files forbidden:
all other files

Required sections:

1. Mission
2. Product boundary
3. Final goal
4. First target
5. Core guarantees
6. Non-goals v0.1
7. First demonstration
8. Engineering rule

Project facts:

* AxiomRT is a formally specified microkernel-based safety runtime.
* It is not a general-purpose OS.
* First target is RISC-V 64 on QEMU through OpenSBI.
* Kernel language is Rust no_std with minimal RISC-V assembly.
* Security model is capability-based access control.
* First guarantees are memory isolation, capability-controlled access, deterministic scheduling, and controlled user-space fault recovery.
* No GUI, filesystem, network, POSIX, dynamic drivers, multicore, or AI inside kernel in v0.1.

Definition of done:

* Only docs/00_PROJECT_CHARTER.md is modified.
* No code is created.
* No architecture outside the given scope is invented.

## AXIOM-DOC-002

Task ID: AXIOM-DOC-002

Goal:
Create docs/01_SCOPE_AND_NON_GOALS.md.

Files allowed:
docs/01_SCOPE_AND_NON_GOALS.md

Files forbidden:
all other files

Required sections:

1. Scope v0.1
2. Explicit non-goals v0.1
3. Future scope v0.2+
4. Forbidden early features
5. Rationale

Scope v0.1:

* RISC-V 64 QEMU boot
* minimal microkernel
* isolated user tasks
* synchronous IPC
* capabilities
* fixed-priority scheduler
* watchdog supervisor
* fault events

Non-goals v0.1:

* GUI
* filesystem
* network stack
* POSIX
* dynamic drivers
* desktop use
* multicore
* hardware certification claim
* AI inside kernel

Definition of done:

* The document prevents scope creep.
* No code is created.

## AXIOM-DOC-003

Task ID: AXIOM-DOC-003

Goal:
Create docs/02_KERNEL_BLUEPRINT.md.

Files allowed:
docs/02_KERNEL_BLUEPRINT.md

Files forbidden:
all other files

Required sections:

1. Kernel identity
2. Kernel rule
3. Kernel responsibilities
4. Kernel non-responsibilities
5. Target platform
6. Kernel object model
7. Trust boundary
8. Memory principle
9. IPC principle
10. Scheduling principle
11. Fault principle
12. First demonstration
13. Forbidden early design choices
14. Phase 0 exit criteria

Kernel responsibilities v0.1:

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

Kernel non-responsibilities v0.1:

* filesystem
* network stack
* GUI
* shell
* package manager
* POSIX layer
* AI logic
* logging storage backend

Definition of done:

* The file defines the kernel blueprint clearly.
* No implementation is added.

## AXIOM-DOC-004

Task ID: AXIOM-DOC-004

Goal:
Create docs/03_KERNEL_OBJECTS.md.

Files allowed:
docs/03_KERNEL_OBJECTS.md

Files forbidden:
all other files

Objects:

* KernelObject
* Thread
* AddressSpace
* PhysicalFrame
* PageTable
* Endpoint
* Message
* Capability
* SchedulingContext
* Timer
* FaultEvent

For each object define:

* purpose
* owner
* lifecycle
* valid states
* allowed operations
* invalid operations
* failure behavior
* security impact

Definition of done:

* All objects are defined.
* No object has vague responsibility.
* No code is created.

## AXIOM-DOC-005

Task ID: AXIOM-DOC-005

Goal:
Create docs/04_SYSCALL_MODEL.md.

Files allowed:
docs/04_SYSCALL_MODEL.md

Files forbidden:
all other files

Define these syscalls:

* sys_yield
* sys_exit
* sys_send
* sys_recv
* sys_reply
* sys_cap_query
* sys_fault_ack

For each syscall define:

* purpose
* arguments
* required capability
* success result
* failure result
* validation rule
* fault behavior
* security rule

Forbidden syscalls v0.1:

* open
* file read
* file write
* socket
* fork
* exec
* shared mmap

Definition of done:

* Each syscall has a precise validation rule.
* No implementation is added.

## AXIOM-DOC-006

Task ID: AXIOM-DOC-006

Goal:
Create docs/05_MEMORY_MODEL.md.

Files allowed:
docs/05_MEMORY_MODEL.md

Files forbidden:
all other files

Required sections:

1. Address spaces
2. Kernel memory
3. User memory
4. Physical frames
5. Page tables
6. Permissions
7. Device memory
8. Page fault behavior
9. Forbidden memory features v0.1
10. Verification properties

Memory permissions:

* READ
* WRITE
* EXECUTE
* USER
* KERNEL
* DEVICE

Core rules:

* no user task can access kernel memory
* no user task can access another task memory without explicit mapping
* invalid access creates a page fault
* page fault kills or suspends the offending task
* shared memory is forbidden in v0.1

Definition of done:

* The document defines enforceable memory rules.
* No code is created.

## AXIOM-DOC-007

Task ID: AXIOM-DOC-007

Goal:
Create docs/06_FAULT_MODEL.md.

Files allowed:
docs/06_FAULT_MODEL.md

Files forbidden:
all other files

Fault types:

* IllegalSyscall
* InvalidCapability
* PageFault
* IllegalInstruction
* WatchdogTimeout
* DeadlineMiss
* IPCViolation
* KernelInvariantViolation

For each fault define:

* source
* severity
* kernel action
* supervisor notification
* recovery options
* logging fields

Recovery options:

* Kill
* Restart
* Suspend
* Quarantine
* Escalate
* KernelPanic

Definition of done:

* Every fault has explicit behavior.
* No undefined behavior remains.
* No code is created.

## AXIOM-DOC-008

Task ID: AXIOM-DOC-008

Goal:
Create docs/07_CODEX_RULES.md.

Files allowed:
docs/07_CODEX_RULES.md

Files forbidden:
all other files

Required sections:

1. Role of Codex
2. Forbidden actions
3. Required task format
4. Review checklist
5. Commit rules
6. Unsafe code policy
7. Dependency policy
8. Documentation policy

Definition of done:

* The document makes Codex an implementation assistant only.
* No code is created.

## AXIOM-DOC-009

Task ID: AXIOM-DOC-009

Goal:
Create docs/08_PHASE_0_GATE.md.

Files allowed:
docs/08_PHASE_0_GATE.md

Files forbidden:
all other files

Create a checklist for completing Phase 0.

Checklist must verify:

* project charter exists
* scope is defined
* non-goals are explicit
* kernel blueprint exists
* kernel objects are defined
* syscall model is defined
* memory model is defined
* fault model is defined
* Codex rules exist
* no code was written
* Phase 1 is allowed only after all checks pass

Definition of done:

* The file clearly blocks coding until Phase 0 is complete.

---

# Phase 1 — Repository Skeleton

Goal:
Create the repository structure and build discipline.

Allowed:
Repository folders, README, placeholders, CI placeholder.

Forbidden:
Real kernel logic.

## AXIOM-REPO-001

Task ID: AXIOM-REPO-001

Goal:
Create the repository skeleton for AxiomRT.

Files allowed:
README.md
docs/INDEX.md
kernel/.gitkeep
userland/.gitkeep
proofs/.gitkeep
tests/.gitkeep
tools/.gitkeep
scripts/.gitkeep
ci/.gitkeep
examples/.gitkeep

Files forbidden:
all other files

Required tree:
AxiomRT/

* README.md
* docs/
* kernel/
* userland/
* proofs/
* tests/
* tools/
* scripts/
* ci/
* examples/

Definition of done:

* Folder structure exists.
* No kernel implementation exists.
* README explains that Phase 1 is repository setup only.

## AXIOM-REPO-002

Task ID: AXIOM-REPO-002

Goal:
Create README.md for AxiomRT.

Files allowed:
README.md

Files forbidden:
all other files

Required sections:

1. What is AxiomRT?
2. What AxiomRT is not
3. Current phase
4. Target platform
5. Architecture direction
6. Safety rule
7. Codex rule
8. Phase map

Definition of done:

* README is clear.
* No false certification claim is made.
* No code is created.

## AXIOM-REPO-003

Task ID: AXIOM-REPO-003

Goal:
Create docs/INDEX.md linking all documentation.

Files allowed:
docs/INDEX.md

Files forbidden:
all other files

Definition of done:

* Index links all Phase 0 docs.
* Index contains phase order.
* No code is created.

---

# Phase 2 — Boot Kernel in QEMU

Goal:
Boot a minimal RISC-V 64 kernel in QEMU and print a boot banner.

Allowed:
Minimal boot files only.

Forbidden:
Scheduler, memory manager, IPC, capabilities.

## AXIOM-BOOT-001

Task ID: AXIOM-BOOT-001

Goal:
Create minimal Rust no_std kernel crate structure for RISC-V 64.

Files allowed:
Cargo.toml
.cargo/config.toml
kernel/Cargo.toml
kernel/src/lib.rs
kernel/src/main.rs
kernel/src/panic.rs
docs/09_BUILD_AND_BOOT.md

Files forbidden:
kernel/src/memory.rs
kernel/src/sched.rs
kernel/src/ipc.rs
kernel/src/caps.rs
kernel/src/syscall.rs
all userland files

Requirements:

* Rust no_std
* no heap
* no scheduler
* no user tasks
* panic handler exists
* build target is RISC-V 64 bare metal or documented equivalent

Definition of done:

* cargo check reaches the expected bare-metal state
* no OS features are implemented
* docs/09_BUILD_AND_BOOT.md explains how to build

## AXIOM-BOOT-002

Task ID: AXIOM-BOOT-002

Goal:
Add minimal RISC-V boot entry and linker script.

Files allowed:
kernel/src/arch/riscv64/boot.S
kernel/linker.ld
kernel/src/main.rs
docs/09_BUILD_AND_BOOT.md

Files forbidden:
kernel/src/memory.rs
kernel/src/sched.rs
kernel/src/ipc.rs
kernel/src/caps.rs
kernel/src/syscall.rs

Expected behavior:

* boot entry sets stack
* boot entry calls Rust kernel_main
* kernel_main does not start scheduler
* kernel_main enters halt loop

Definition of done:

* Boot path is documented.
* No unrelated kernel logic is added.

## AXIOM-BOOT-003

Task ID: AXIOM-BOOT-003

Goal:
Add UART serial output for QEMU boot banner.

Files allowed:
kernel/src/arch/riscv64/uart.rs
kernel/src/main.rs
docs/09_BUILD_AND_BOOT.md

Files forbidden:
all scheduler, IPC, memory, capability files

Expected output:
AxiomRT kernel booted
arch=riscv64
phase=boot

Definition of done:

* Running QEMU prints the banner.
* No scheduler is added.
* No heap is added.

## AXIOM-BOOT-004

Task ID: AXIOM-BOOT-004

Goal:
Add scripts/run_qemu.sh.

Files allowed:
scripts/run_qemu.sh
docs/09_BUILD_AND_BOOT.md

Files forbidden:
kernel source files

Definition of done:

* Script runs QEMU with the built kernel.
* Documentation includes exact command.
* No kernel logic changes.

## AXIOM-BOOT-005

Task ID: AXIOM-BOOT-005

Goal:
Add boot smoke test that checks QEMU output contains the boot banner.

Files allowed:
tests/boot_smoke_test.sh
scripts/run_qemu.sh
docs/14_TEST_STRATEGY.md

Files forbidden:
kernel source files except if strictly needed to expose output

Definition of done:

* Test fails if banner is missing.
* Test passes if banner appears.
* No OS feature is added.

---

# Phase 3 — Trap and Exception Layer

Goal:
Create controlled entry paths for exceptions, interrupts, and syscalls.

Forbidden:
Full syscall implementation, scheduler, IPC, capabilities.

## AXIOM-TRAP-001

Task ID: AXIOM-TRAP-001

Goal:
Add RISC-V trap vector skeleton.

Files allowed:
kernel/src/arch/riscv64/trap.S
kernel/src/arch/riscv64/trap.rs
kernel/src/main.rs
docs/10_TRAP_MODEL.md

Files forbidden:
kernel/src/sched.rs
kernel/src/ipc.rs
kernel/src/caps.rs

Requirements:

* trap vector exists
* trap handler decodes basic cause
* unknown trap leads to controlled panic for now
* no user task support yet

Definition of done:

* Trap path is documented.
* No scheduler is implemented.

## AXIOM-TRAP-002

Task ID: AXIOM-TRAP-002

Goal:
Add illegal instruction handler skeleton.

Files allowed:
kernel/src/arch/riscv64/trap.rs
docs/10_TRAP_MODEL.md

Expected behavior:

* illegal instruction is identified
* kernel prints a structured trap message
* system halts safely for now

Definition of done:

* No undefined trap behavior remains for illegal instruction.

## AXIOM-TRAP-003

Task ID: AXIOM-TRAP-003

Goal:
Add syscall trap stub.

Files allowed:
kernel/src/arch/riscv64/trap.rs
kernel/src/syscall/mod.rs
docs/04_SYSCALL_MODEL.md
docs/10_TRAP_MODEL.md

Forbidden:
Actual syscall logic beyond stub dispatch.

Expected behavior:

* syscall trap is recognized
* unknown syscall returns or logs controlled error
* no user mode yet

Definition of done:

* syscall trap path exists as a stub
* no IPC or capability logic is implemented yet

---

# Phase 4 — Memory Isolation

Goal:
Define and implement the first memory separation layer.

Forbidden:
Shared memory, device memory mapping for user tasks, dynamic allocation after boot.

## AXIOM-MEM-001

Task ID: AXIOM-MEM-001

Goal:
Create memory module skeleton and address constants.

Files allowed:
kernel/src/memory/mod.rs
kernel/src/memory/address.rs
docs/05_MEMORY_MODEL.md

Requirements:

* define VirtAddr and PhysAddr wrappers
* define kernel address range constants
* define user address range constants
* no page table implementation yet

Definition of done:

* Address types prevent raw integer confusion.
* No unsafe block unless justified.

## AXIOM-MEM-002

Task ID: AXIOM-MEM-002

Goal:
Add physical frame model.

Files allowed:
kernel/src/memory/frame.rs
kernel/src/memory/mod.rs
docs/05_MEMORY_MODEL.md

Requirements:

* PhysicalFrame type
* FrameState enum
* owner field
* no allocator yet

Definition of done:

* Physical frame lifecycle is represented.
* No dynamic heap is introduced.

## AXIOM-MEM-003

Task ID: AXIOM-MEM-003

Goal:
Add page table model skeleton.

Files allowed:
kernel/src/memory/pagetable.rs
kernel/src/memory/mod.rs
docs/05_MEMORY_MODEL.md

Requirements:

* PageTable type
* Mapping type
* permissions enum
* no full MMU activation yet

Definition of done:

* Mapping rules match docs/05_MEMORY_MODEL.md.

---

# Phase 5 — Thread Model

Goal:
Represent kernel thread objects and thread states.

## AXIOM-THREAD-001

Task ID: AXIOM-THREAD-001

Goal:
Create Thread object model.

Files allowed:
kernel/src/thread/mod.rs
kernel/src/thread/state.rs
kernel/src/thread/id.rs
docs/03_KERNEL_OBJECTS.md

Requirements:

* ThreadId type
* ThreadState enum
* Thread struct skeleton
* states: Ready, Running, Blocked, Faulted, Killed, Suspended

Definition of done:

* Thread model exists.
* No context switching yet.

## AXIOM-THREAD-002

Task ID: AXIOM-THREAD-002

Goal:
Add RISC-V register context structure.

Files allowed:
kernel/src/arch/riscv64/context.rs
kernel/src/thread/context.rs
docs/03_KERNEL_OBJECTS.md

Requirements:

* define saved registers
* no context switch assembly yet
* document assumptions

Definition of done:

* Context layout is explicit.

---

# Phase 6 — Scheduler

Goal:
Add fixed-priority preemptive scheduler.

## AXIOM-SCHED-001

Task ID: AXIOM-SCHED-001

Goal:
Create scheduler model skeleton.

Files allowed:
kernel/src/sched/mod.rs
kernel/src/sched/priority.rs
kernel/src/sched/queue.rs
docs/09_SCHEDULER_MODEL.md

Requirements:

* FixedPriorityScheduler type
* priority levels
* ready queue abstraction
* no timer preemption yet

Definition of done:

* scheduler can select highest-priority ready thread in tests.

## AXIOM-SCHED-002

Task ID: AXIOM-SCHED-002

Goal:
Add scheduler unit tests.

Files allowed:
kernel/src/sched/*
tests/scheduler_tests.rs
docs/14_TEST_STRATEGY.md

Tests:

* highest priority task selected
* killed task not selected
* blocked task not selected
* equal priority uses deterministic rule

Definition of done:

* tests pass
* no hardware dependency

---

# Phase 7 — User Mode

Goal:
Run first user task outside kernel privilege.

## AXIOM-USER-001

Task ID: AXIOM-USER-001

Goal:
Define user task image model.

Files allowed:
kernel/src/user/mod.rs
kernel/src/user/image.rs
docs/03_KERNEL_OBJECTS.md

Requirements:

* user image descriptor
* entry point
* stack region
* address space reference

Definition of done:

* model exists
* no actual user jump yet

## AXIOM-USER-002

Task ID: AXIOM-USER-002

Goal:
Implement first controlled transition to user mode.

Files allowed:
kernel/src/arch/riscv64/user_entry.S
kernel/src/arch/riscv64/context.rs
kernel/src/user/mod.rs
docs/10_USER_MODE.md

Requirements:

* transition to user mode documented
* return through trap path
* bad return path controlled

Definition of done:

* first user task can trap back through syscall or fault
* kernel survives

---

# Phase 8 — IPC

Goal:
Add synchronous copy-based IPC.

## AXIOM-IPC-001

Task ID: AXIOM-IPC-001

Goal:
Create IPC object model.

Files allowed:
kernel/src/ipc/mod.rs
kernel/src/ipc/endpoint.rs
kernel/src/ipc/message.rs
docs/08_IPC_MODEL.md

Requirements:

* Endpoint object
* Message object
* bounded message size
* no shared memory
* send/receive states

Definition of done:

* IPC model compiles
* no syscall integration yet

## AXIOM-IPC-002

Task ID: AXIOM-IPC-002

Goal:
Add synchronous send/receive logic without capabilities.

Files allowed:
kernel/src/ipc/*
tests/ipc_tests.rs
docs/08_IPC_MODEL.md

Requirements:

* send blocks if no receiver
* receive blocks if no sender
* bounded copy
* deterministic behavior

Definition of done:

* IPC unit tests pass
* no capability bypass is claimed

---

# Phase 9 — Capabilities

Goal:
Make all object access capability-controlled.

## AXIOM-CAP-001

Task ID: AXIOM-CAP-001

Goal:
Create capability model.

Files allowed:
kernel/src/caps/mod.rs
kernel/src/caps/capability.rs
kernel/src/caps/rights.rs
docs/06_CAPABILITY_MODEL.md

Rights:

* Read
* Write
* Execute
* Send
* Receive
* Grant
* Map
* Control

Definition of done:

* Capability type exists
* rights are explicit
* no syscall uses raw object access

## AXIOM-CAP-002

Task ID: AXIOM-CAP-002

Goal:
Add capability lookup table.

Files allowed:
kernel/src/caps/table.rs
kernel/src/caps/mod.rs
tests/capability_tests.rs
docs/06_CAPABILITY_MODEL.md

Tests:

* lookup valid capability
* reject missing capability
* reject insufficient rights
* reject wrong object type

Definition of done:

* tests pass
* error behavior is explicit

## AXIOM-CAP-003

Task ID: AXIOM-CAP-003

Goal:
Integrate capabilities into IPC.

Files allowed:
kernel/src/ipc/*
kernel/src/caps/*
kernel/src/syscall/mod.rs
tests/ipc_capability_tests.rs
docs/04_SYSCALL_MODEL.md
docs/08_IPC_MODEL.md

Requirements:

* sys_send requires Send right
* sys_recv requires Receive right
* invalid capability creates InvalidCapability fault

Definition of done:

* IPC without capability fails
* IPC with capability succeeds
* tests pass

---

# Phase 10 — Fault Recovery

Goal:
Contain faulty user tasks and notify supervisor.

## AXIOM-FAULT-001

Task ID: AXIOM-FAULT-001

Goal:
Create FaultEvent model.

Files allowed:
kernel/src/fault/mod.rs
kernel/src/fault/event.rs
docs/06_FAULT_MODEL.md

Fault types:

* IllegalSyscall
* InvalidCapability
* PageFault
* IllegalInstruction
* WatchdogTimeout
* DeadlineMiss
* IPCViolation
* KernelInvariantViolation

Definition of done:

* FaultEvent is structured
* severity is explicit
* no recovery policy implemented yet

## AXIOM-FAULT-002

Task ID: AXIOM-FAULT-002

Goal:
Add basic fault handling policy.

Files allowed:
kernel/src/fault/*
kernel/src/thread/*
docs/06_FAULT_MODEL.md

Requirements:

* user fault can mark thread Faulted
* kernel fault triggers panic
* critical task behavior is preserved as documented

Definition of done:

* user-space fault does not crash kernel
* kernel-space invariant violation halts safely

## AXIOM-FAULT-003

Task ID: AXIOM-FAULT-003

Goal:
Create supervisor notification path.

Files allowed:
kernel/src/fault/*
kernel/src/ipc/*
userland/supervisor/*
docs/06_FAULT_MODEL.md

Requirements:

* supervisor can receive fault event
* supervisor cannot bypass capabilities
* recovery decision is explicit

Definition of done:

* fault event reaches supervisor task
* no raw privilege bypass exists

---

# Phase 11 — Runtime Monitoring

Goal:
Create structured evidence logs.

## AXIOM-MON-001

Task ID: AXIOM-MON-001

Goal:
Create kernel event model.

Files allowed:
kernel/src/monitor/mod.rs
kernel/src/monitor/event.rs
docs/11_RUNTIME_MONITORING.md

Events:

* TASK_STARTED
* TASK_EXITED
* TASK_FAULTED
* CAP_DENIED
* IPC_DENIED
* PAGE_FAULT
* DEADLINE_MISSED
* WATCHDOG_TIMEOUT
* RECOVERY_APPLIED

Definition of done:

* event format is structured
* no storage backend is added

## AXIOM-MON-002

Task ID: AXIOM-MON-002

Goal:
Add serial event export for QEMU.

Files allowed:
kernel/src/monitor/*
kernel/src/arch/riscv64/uart.rs
docs/11_RUNTIME_MONITORING.md

Definition of done:

* monitor events print in structured text format
* no filesystem is added

---

# Phase 12 — Formal Proof Starter

Goal:
Create first formal models.

## AXIOM-PROOF-001

Task ID: AXIOM-PROOF-001

Goal:
Create initial Coq model for memory isolation.

Files allowed:
proofs/coq/MemoryIsolation.v
proofs/README.md
docs/11_VERIFICATION_PLAN.md

Required theorem shape:
A task cannot read an address that is not mapped in its address space.

Definition of done:

* Coq file contains definitions and theorem statement
* proof may be admitted only if marked as TODO
* assumptions are explicit

## AXIOM-PROOF-002

Task ID: AXIOM-PROOF-002

Goal:
Create initial Coq model for capability access.

Files allowed:
proofs/coq/CapabilityAccess.v
proofs/README.md
docs/11_VERIFICATION_PLAN.md

Required theorem shape:
A task cannot invoke a protected object without a valid capability with sufficient rights.

Definition of done:

* definitions exist
* theorem statement exists
* assumptions are explicit

## AXIOM-PROOF-003

Task ID: AXIOM-PROOF-003

Goal:
Create initial Coq model for scheduler priority.

Files allowed:
proofs/coq/SchedulerPriority.v
proofs/README.md
docs/11_VERIFICATION_PLAN.md

Required theorem shape:
If a high-priority ready task exists, a lower-priority task is not selected.

Definition of done:

* scheduler model exists
* theorem statement exists
* assumptions are explicit

---

# Phase 13 — Industrial Evaluation Kit

Goal:
Package the prototype for serious technical review.

## AXIOM-KIT-001

Task ID: AXIOM-KIT-001

Goal:
Create docs/INDUSTRIAL_EVALUATION_KIT.md.

Files allowed:
docs/INDUSTRIAL_EVALUATION_KIT.md

Required sections:

1. Product definition
2. What is included
3. What is not included
4. Target users
5. Demo scenario
6. Safety evidence
7. Security evidence
8. Verification evidence
9. Known limitations
10. Assumptions of use

Definition of done:

* no certification claim is made
* limitations are explicit

## AXIOM-KIT-002

Task ID: AXIOM-KIT-002

Goal:
Create demo scenario documentation.

Files allowed:
examples/fault_containment_demo/README.md
docs/DEMO_SCENARIO.md

Scenario:

* critical_task runs periodically
* logger_task receives events
* faulty_task attempts illegal memory access
* faulty_task attempts illegal IPC
* faulty_task loops forever
* supervisor receives fault events
* critical_task continues

Definition of done:

* demo can be understood without reading source code
* expected output is documented

---

# Review Checklist After Every Codex Task

After each Codex result, check:

1. Did Codex modify only allowed files?
2. Did Codex touch forbidden files?
3. Did Codex invent architecture?
4. Did Codex add dependencies?
5. Did Codex add unsafe code?
6. Did Codex add broad refactor?
7. Did Codex remove tests?
8. Did Codex weaken a check?
9. Did Codex update docs if required?
10. Did Codex implement exactly the task and nothing more?
11. Does the change build?
12. Are errors visible?
13. Is rollback simple?

Reject the change if any answer is wrong.

---

# Commit Format

Each commit must map to one task.

Format:

AXIOM-AREA-NNN: short imperative summary

Examples:

AXIOM-DOC-001: add project charter

AXIOM-DOC-002: define scope and non-goals

AXIOM-BOOT-003: add QEMU UART boot banner

AXIOM-CAP-002: add capability lookup tests

Never commit multiple phases together.

---

# First Local Commands

mkdir -p AxiomRT/docs
cd AxiomRT
git init

touch README.md
touch docs/00_PROJECT_CHARTER.md
touch docs/01_SCOPE_AND_NON_GOALS.md
touch docs/02_KERNEL_BLUEPRINT.md
touch docs/03_KERNEL_OBJECTS.md
touch docs/04_SYSCALL_MODEL.md
touch docs/05_MEMORY_MODEL.md
touch docs/06_FAULT_MODEL.md
touch docs/07_CODEX_RULES.md
touch docs/08_PHASE_0_GATE.md

git status

---

# First Codex Task To Run

Paste this first:

You are working on AxiomRT, a formally specified microkernel-based safety runtime for high-assurance embedded systems.

AxiomRT is not a general-purpose OS.

AxiomRT v0.1 targets RISC-V 64 on QEMU through OpenSBI, using Rust no_std and minimal RISC-V assembly.

Your role is implementation assistant only. Do not invent architecture. Do not create code. Do not modify files outside the allowed list.

Task ID: AXIOM-DOC-001

Goal:
Create docs/00_PROJECT_CHARTER.md.

Files allowed:
docs/00_PROJECT_CHARTER.md

Files forbidden:
all other files

Required sections:

1. Mission
2. Product boundary
3. Final goal
4. First target
5. Core guarantees
6. Non-goals v0.1
7. First demonstration
8. Engineering rule

Project facts:

* AxiomRT is a formally specified microkernel-based safety runtime.
* It is not a general-purpose OS.
* First target is RISC-V 64 on QEMU through OpenSBI.
* Kernel language is Rust no_std with minimal RISC-V assembly.
* Security model is capability-based access control.
* First guarantees are memory isolation, capability-controlled access, deterministic scheduling, and controlled user-space fault recovery.
* No GUI, filesystem, network, POSIX, dynamic drivers, multicore, or AI inside kernel in v0.1.

Definition of done:

* Only docs/00_PROJECT_CHARTER.md is modified.
* No code is created.
* No architecture outside the given scope is invented.

---

# Execution Rule

Run the tasks in this order:

1. AXIOM-DOC-001
2. AXIOM-DOC-002
3. AXIOM-DOC-003
4. AXIOM-DOC-004
5. AXIOM-DOC-005
6. AXIOM-DOC-006
7. AXIOM-DOC-007
8. AXIOM-DOC-008
9. AXIOM-DOC-009
10. AXIOM-REPO-001
11. AXIOM-REPO-002
12. AXIOM-REPO-003
13. AXIOM-BOOT-001

Do not start AXIOM-BOOT-001 until Phase 0 and Phase 1 are complete.

The first real kernel code begins only in Phase 2.
