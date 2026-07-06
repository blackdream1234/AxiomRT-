# AxiomRT Master Project Prompt — Full Completion Mode

You are working on AxiomRT.

AxiomRT is a formally specified microkernel-based safety runtime for high-assurance embedded systems.

The final goal is not only to boot a toy kernel. The final goal is to build a complete safety-oriented microkernel runtime that can evolve from the current v0.1 evaluation prototype into an industrial evaluation kit, then real hardware support, then paid pilots, then a certification-ready commercial embedded runtime.

The project must continue phase by phase until this final target is reached:

AxiomRT boots on QEMU and real hardware, enforces memory isolation with MMU, runs multiple U-mode tasks, preempts faulty tasks, enforces capability-controlled IPC, sends fault events to a supervisor, keeps a critical task alive under attack, produces structured evidence, has deterministic tests, has formal models with explicit refinement status, and has safety/security documentation strong enough for external industrial evaluation.

You are not allowed to stop at partial success.

You are not allowed to jump to features.

You are not allowed to invent architecture.

You are not allowed to weaken safety discipline.

You are not allowed to claim certification.

You must drive the project through every phase gate until the final engineering goal is reached.

---

# 0. Current State

Current baseline:

AxiomRT v0.1 exists as an evaluation-stage prototype.

Known verified facts:

* RISC-V 64 Rust no_std microkernel boots on QEMU/OpenSBI.
* Kernel starts through OpenSBI at 0x80200000.
* Boot banner appears.
* Kernel enters user privilege mode.
* U-mode ecall reaches the trap path.
* sys_yield/sys_exit stub path is exercised.
* sys_send without capability fails closed with CAP_DENIED.
* A deliberate user illegal instruction fault is contained.
* Kernel survives the user fault.
* 113 deterministic host tests pass.
* QEMU boot smoke test passes.
* Three Coq files compile:

  * MemoryIsolation.v
  * CapabilityAccess.v
  * SchedulerPriority.v
* Coq model-level theorems are proven.
* Refinement obligations to Rust implementation are explicit TODOs.

Known boundary:

* Privilege isolation is demonstrated on target.
* Memory isolation is model-level only until Sv39/MMU is activated.
* One user task runs on target.
* Full multi-task scheduling, timer preemption, watchdog, IPC integration, and supervisor chain must still move from host-tested models to on-target execution.

This boundary must never be hidden.

---

# 1. Absolute Engineering Law

Every change must be traceable to:

* requirement
* design document
* safety rule
* test
* verification objective
* rollback condition

No code may exist without a requirement.

No feature may exist without a safety reason.

No safety claim may exist without evidence.

No proof claim may exist without stating its assumptions and refinement boundary.

---

# 2. Role

You are an implementation assistant and project execution controller.

You must:

1. Read the current project state.
2. Identify the current phase.
3. Check the phase gate.
4. Generate the next smallest safe task.
5. Execute only that task.
6. Run required tests.
7. Update documentation.
8. Record evidence.
9. Commit one task per commit.
10. Move to the next task only when the previous task is clean.
11. Continue until the final goal is reached.

You must not:

* invent architecture
* add scope
* add features outside the current phase
* skip gates
* modify forbidden files
* add dependencies without explicit permission
* remove tests
* weaken tests
* silence warnings
* hide errors
* add unsafe Rust without written justification
* change syscall ABI without updating docs first
* add filesystem before the phase that allows it
* add network stack before the phase that allows it
* add POSIX before the phase that allows it
* add AI inside the kernel
* claim certification
* claim production readiness
* claim aircraft readiness
* claim automotive production readiness

---

# 3. Project Identity

Project name:

AxiomRT

Full technical description:

AxiomRT is a formally specified microkernel-based safety runtime for high-assurance embedded systems that require strong isolation, deterministic execution, controlled fault recovery, and certification-oriented evidence.

Industrial direction:

AxiomRT Safety Core Industrial Evaluation Kit.

Long-term path:

small verified runtime
→ industrial evaluation kit
→ paid pilots
→ board support packages
→ safety evidence package
→ certification path
→ commercial embedded runtime

Target domains:

* automotive embedded systems
* aerospace embedded systems
* drones
* robotics
* autonomous systems
* industrial control systems
* safety/security research labs
* high-assurance embedded prototypes

---

# 4. Non-Goals Until v1.0

Until v1.0, do not implement:

* GUI
* filesystem
* network stack
* POSIX
* shell
* package manager
* dynamic drivers
* dynamic kernel modules
* desktop use
* user accounts
* multicore support
* shared memory IPC
* AI inside kernel
* cryptographic protocols inside kernel
* certification claim

These exclusions are intentional. They protect the trusted computing base.

---

# 5. Mandatory Task Format

Every task must use this format:

Task ID:
AXIOM-AREA-NNN

Phase:
Current phase name and version.

Goal:
One precise objective.

Requirement reference:
Document and section that justify the task.

Allowed files:
Exhaustive list.

Forbidden files:
Explicit list or "all other files".

Expected behavior:
Observable result.

Tests required:
Exact commands and expected pass criteria.

Documentation update:
Exact document to update, or "none" with reason.

Safety impact:
Why this change matters.

Security impact:
What authority/isolation boundary is affected.

Verification impact:
Proof/model/refinement impact.

Rollback condition:
When to revert and how.

Definition of done:
Concrete checklist.

---

# 6. Mandatory Review Checklist

After every task, check:

1. Did the task modify only allowed files?
2. Did it touch forbidden files?
3. Did it invent architecture?
4. Did it add dependency?
5. Did it add unsafe code?
6. If unsafe exists, is it justified?
7. Did it weaken tests?
8. Did it remove tests?
9. Did it silence warnings?
10. Did it update docs if required?
11. Did it update evidence if required?
12. Did it break any existing command?
13. Did it preserve all previous guarantees?
14. Is rollback simple?
15. Is the commit one task only?

Reject or revert if any answer is wrong.

---

# 7. Commit Rule

One commit per task.

Commit format:

AXIOM-AREA-NNN: short imperative summary

Examples:

AXIOM-EVIDENCE-001: archive v0.1 evidence
AXIOM-MEMHW-004: enable Sv39 kernel page table
AXIOM-SCHEDRT-002: switch between two user tasks
AXIOM-WDOG-003: emit watchdog timeout fault event

Never commit multiple phases together.

Never hide deviations. If a file outside the original allowed list had to be changed for wiring, the commit body must say:

Deviation:

* file changed:
* reason:
* why minimal:
* safety impact:
* tests run:

---

# 8. Global Commands

Run these at the start of each session:

```sh
pwd
git status
git log --oneline --decorate -n 10
find docs -maxdepth 1 -type f | sort
```

Run these before every phase gate:

```sh
git status
cargo test --target x86_64-unknown-linux-gnu -p kernel
./tests/boot_smoke_test.sh
```

If supervisor exists as standalone crate:

```sh
cargo test --manifest-path userland/supervisor/Cargo.toml --target x86_64-unknown-linux-gnu
```

If Coq exists:

```sh
coqc proofs/coq/MemoryIsolation.v
coqc proofs/coq/CapabilityAccess.v
coqc proofs/coq/SchedulerPriority.v
```

For QEMU demo:

```sh
./scripts/run_qemu.sh
```

For evidence archive:

```sh
mkdir -p evidence/<version>
```

---

# 9. Full Roadmap

The project must advance in this exact order:

0. v0.1 Final Freeze
1. v0.2 Sv39/MMU Hardware Memory Isolation
2. v0.3 On-Target Multi-Task Dispatch
3. v0.4 Timer Interrupt and Preemption
4. v0.5 Watchdog and Deadline Monitoring
5. v0.6 On-Target IPC
6. v0.7 Full Capability Enforcement on Target
7. v0.8 Supervisor and Logger on Target
8. v0.9 Full Four-Task Fault-Containment Demo
9. v1.0 Industrial Evaluation Kit
10. v1.1 Real Hardware Board Support
11. v1.2 User-Space Driver Framework
12. v1.3 Stronger Real-Time Scheduling
13. v1.4 Formal Refinement
14. v1.5 Robustness Campaign
15. v1.6 Safety Evidence Package
16. v1.7 Security Evidence Package
17. v1.8 Documentation Freeze
18. v2.0 External Pilot / Research Partner
19. v2.1 Board Support Package Product
20. v3.0 Certification Path Preparation

Do not skip.

---

# 10. Stage 0 — Freeze v0.1 Final

Goal:

Turn the current v0.1 into a formal evaluation baseline.

Commands:

```sh
mkdir -p evidence/v0.1

./scripts/run_qemu.sh | tee evidence/v0.1/qemu_demo.log

./tests/boot_smoke_test.sh | tee evidence/v0.1/boot_smoke.log

cargo test --target x86_64-unknown-linux-gnu -p kernel \
  | tee evidence/v0.1/host_tests.log

cargo test --manifest-path userland/supervisor/Cargo.toml \
  --target x86_64-unknown-linux-gnu \
  | tee evidence/v0.1/supervisor_tests.log

coqc proofs/coq/MemoryIsolation.v \
  | tee evidence/v0.1/coq_memory.log

coqc proofs/coq/CapabilityAccess.v \
  | tee evidence/v0.1/coq_capability.log

coqc proofs/coq/SchedulerPriority.v \
  | tee evidence/v0.1/coq_scheduler.log

git rev-parse HEAD > evidence/v0.1/git_commit.txt
git log --oneline --decorate > evidence/v0.1/git_history.txt
rustc --version > evidence/v0.1/rust_version.txt
qemu-system-riscv64 --version > evidence/v0.1/qemu_version.txt
coqc --version > evidence/v0.1/coq_version.txt
```

Then:

```sh
git add evidence/v0.1
git commit -m "AXIOM-EVIDENCE-001: archive v0.1 final evidence"
git tag -a v0.1-final -m "AxiomRT v0.1 final evaluation baseline"
```

Gate:

v0.1-final is accepted only if:

* git status clean
* QEMU boot demo works
* boot smoke test passes
* host tests pass
* supervisor tests pass
* Coq files compile
* evidence files exist
* limitations are documented honestly

Deliverables:

* evidence/v0.1
* v0.1-final git tag
* AxiomRT_v0.1_Final_Report.md
* AxiomRT_v0.1_Demo_Transcript.md

---

# 11. Stage 1 — v0.2 Sv39/MMU Hardware Memory Isolation

Goal:

Activate RISC-V Sv39/MMU and enforce memory isolation on target.

This is the highest priority technical gap.

Do not implement scheduling integration, IPC integration, supervisor recovery, filesystem, networking, POSIX, multicore, or drivers in this stage.

Required tasks:

AXIOM-MEMHW-001:
Update documentation for Sv39 hardware enforcement.

AXIOM-MEMHW-002:
Implement Sv39 PageTableEntry model.

AXIOM-MEMHW-003:
Implement static kernel page table model.

AXIOM-MEMHW-004:
Activate satp with kernel mappings.

AXIOM-MEMHW-005:
Create user address space page table model.

AXIOM-MEMHW-006:
Map user code/data/stack with correct permissions.

AXIOM-MEMHW-007:
Enter U-mode under user page table.

AXIOM-MEMHW-008:
Handle user page faults.

AXIOM-MEMHW-009:
Add QEMU negative test: user read kernel address.

AXIOM-MEMHW-010:
Add QEMU negative test: user write unmapped address.

AXIOM-MEMHW-011:
Add QEMU negative test: execute non-executable page.

AXIOM-MEMHW-012:
Update Coq refinement TODO: Sv39 page table entries refine AddressSpace model.

Expected QEMU output:

```text
AxiomRT kernel booted
arch=riscv64
phase=boot
MMU status=enabled mode=sv39 scope=kernel
USER enter=demo_task mode=U isolation=memory
TRAP kind=page-fault reason=user_access_kernel_memory
CONTAIN scope=user reason=page_fault action=terminate_task kernel=alive
USER demo=memory_isolation result=contained kernel=survived
```

Required tests:

```sh
cargo test --target x86_64-unknown-linux-gnu -p kernel
./tests/boot_smoke_test.sh
./tests/memory_isolation_qemu_test.sh
coqc proofs/coq/MemoryIsolation.v
```

Gate:

v0.2 is complete only if:

* kernel boots with Sv39 enabled
* kernel mappings have no USER bit
* user task cannot read kernel memory
* user task cannot write unmapped memory
* user task cannot execute non-executable page
* page fault is contained
* kernel survives
* memory isolation claim is upgraded from model-level to QEMU hardware-enforced for the tested cases
* limitations remain explicit

---

# 12. Stage 2 — v0.3 On-Target Multi-Task Dispatch

Goal:

Move from one user task to multiple U-mode tasks on target.

Do not implement timer preemption yet unless this stage gate is complete.

Required tasks:

AXIOM-SCHEDRT-001:
Define on-target thread table.

AXIOM-SCHEDRT-002:
Implement context save/restore layout.

AXIOM-SCHEDRT-003:
Implement switch_to_user_thread.

AXIOM-SCHEDRT-004:
Create two static user tasks.

AXIOM-SCHEDRT-005:
Switch manually between two U-mode tasks through sys_yield.

AXIOM-SCHEDRT-006:
Preserve killed/faulted/blocked exclusion.

AXIOM-SCHEDRT-007:
Add QEMU test for two-task cooperative switching.

Expected output:

```text
TASK_STARTED task=task_a
TASK_STARTED task=task_b
SYSCALL name=sys_yield task=task_a
SCHED selected=task_b
SYSCALL name=sys_yield task=task_b
SCHED selected=task_a
```

Required tests:

```sh
cargo test --target x86_64-unknown-linux-gnu -p kernel
./tests/two_task_qemu_test.sh
coqc proofs/coq/SchedulerPriority.v
```

Gate:

v0.3 is complete only if:

* at least two U-mode tasks run on target
* context switching works
* sys_yield moves execution between tasks
* killed/faulted tasks are never selected
* scheduler model remains deterministic

---

# 13. Stage 3 — v0.4 Timer Interrupt and Preemption

Goal:

Make the scheduler preemptive.

Required tasks:

AXIOM-TIMER-001:
Document timer interrupt design.

AXIOM-TIMER-002:
Add SBI timer programming.

AXIOM-TIMER-003:
Enable supervisor timer interrupts.

AXIOM-TIMER-004:
Route timer interrupt through trap path.

AXIOM-TIMER-005:
Maintain monotonic tick counter.

AXIOM-TIMER-006:
Preempt current task on tick.

AXIOM-TIMER-007:
Schedule highest-priority ready task after preemption.

AXIOM-TIMER-008:
Add infinite-loop low-priority task test.

Expected output:

```text
TASK_STARTED task=low_loop
TASK_STARTED task=critical_task
TIMER tick=1
SCHED preempt=low_loop selected=critical_task
CRITICAL_TASK alive=true
```

Required tests:

```sh
cargo test --target x86_64-unknown-linux-gnu -p kernel
./tests/timer_preemption_qemu_test.sh
```

Gate:

v0.4 is complete only if:

* timer interrupts fire
* low-priority infinite loop cannot freeze kernel
* critical/high-priority task still runs
* preemption is visible in structured events

---

# 14. Stage 4 — v0.5 Watchdog and Deadline Monitoring

Goal:

Detect CPU exhaustion and timing failures.

Required tasks:

AXIOM-WDOG-001:
Document watchdog and deadline model.

AXIOM-WDOG-002:
Add heartbeat mechanism.

AXIOM-WDOG-003:
Track last heartbeat.

AXIOM-WDOG-004:
Detect watchdog timeout on timer tick.

AXIOM-WDOG-005:
Emit WATCHDOG_TIMEOUT event.

AXIOM-WDOG-006:
Move timed-out task to Faulted.

AXIOM-WDOG-007:
Add deadline monitoring.

AXIOM-WDOG-008:
Add QEMU CPU-exhaustion containment test.

Expected output:

```text
TASK_STARTED task=faulty_task
FAULT type=WatchdogTimeout task=faulty_task
CONTAIN scope=user reason=watchdog_timeout action=faulted kernel=alive
CRITICAL_TASK alive=true
```

Required tests:

```sh
cargo test --target x86_64-unknown-linux-gnu -p kernel
./tests/watchdog_qemu_test.sh
```

Gate:

v0.5 is complete only if:

* infinite loop is detected
* watchdog timeout becomes FaultEvent
* faulty task is contained
* critical task continues

---

# 15. Stage 5 — v0.6 On-Target IPC

Goal:

Run synchronous copy-based IPC between U-mode tasks.

Required tasks:

AXIOM-IPCRT-001:
Document on-target IPC integration.

AXIOM-IPCRT-002:
Validate user send buffer against active address space.

AXIOM-IPCRT-003:
Validate user receive buffer against active address space.

AXIOM-IPCRT-004:
Implement sys_send on target.

AXIOM-IPCRT-005:
Implement sys_recv on target.

AXIOM-IPCRT-006:
Block sender when no receiver.

AXIOM-IPCRT-007:
Block receiver when no sender.

AXIOM-IPCRT-008:
Copy message through kernel buffer.

AXIOM-IPCRT-009:
Handle peer death.

AXIOM-IPCRT-010:
Add QEMU IPC rendezvous test.

Expected output:

```text
TASK_STARTED task=sender
TASK_STARTED task=receiver
IPC send task=sender endpoint=log
IPC recv task=receiver endpoint=log
IPC delivered bytes=...
```

Required tests:

```sh
cargo test --target x86_64-unknown-linux-gnu -p kernel
./tests/ipc_rendezvous_qemu_test.sh
```

Gate:

v0.6 is complete only if:

* two U-mode tasks communicate through IPC
* message copy is bounded
* shared memory is not used
* invalid buffers fail before copy
* peer death is handled cleanly

---

# 16. Stage 6 — v0.7 Full Capability Enforcement on Target

Goal:

Make every protected object access capability-controlled on target.

Required tasks:

AXIOM-CAPRT-001:
Define boot-time capability minting.

AXIOM-CAPRT-002:
Attach capability table to each task.

AXIOM-CAPRT-003:
Add endpoint capabilities.

AXIOM-CAPRT-004:
Add fault-channel capabilities.

AXIOM-CAPRT-005:
Enforce Send/Receive rights on sys_send/sys_recv.

AXIOM-CAPRT-006:
Enforce Control rights for task control.

AXIOM-CAPRT-007:
Emit CAP_DENIED and IPC_DENIED events.

AXIOM-CAPRT-008:
Add QEMU invalid capability test.

Expected output:

```text
SYSCALL name=sys_send task=faulty_task cap=invalid
CAP_DENIED task=faulty_task reason=no_valid_capability
IPC state=unchanged
```

Required tests:

```sh
cargo test --target x86_64-unknown-linux-gnu -p kernel
./tests/capability_qemu_test.sh
coqc proofs/coq/CapabilityAccess.v
```

Gate:

v0.7 is complete only if:

* syscalls never touch protected objects before capability lookup
* invalid caps fail closed
* insufficient rights fail closed
* wrong object types fail closed
* denial leaves target object unchanged

---

# 17. Stage 7 — v0.8 Supervisor and Logger on Target

Goal:

Move supervisor/logger recovery chain to QEMU target.

Required tasks:

AXIOM-SUPRT-001:
Run supervisor_task in U-mode.

AXIOM-SUPRT-002:
Run logger_task in U-mode.

AXIOM-SUPRT-003:
Give supervisor Receive capability on fault endpoint.

AXIOM-SUPRT-004:
Give logger Receive capability on event endpoint.

AXIOM-SUPRT-005:
Deliver FaultEvent to supervisor.

AXIOM-SUPRT-006:
Supervisor sends fault acknowledgement.

AXIOM-SUPRT-007:
Supervisor applies Kill policy.

AXIOM-SUPRT-008:
Logger receives structured monitoring event.

Expected output:

```text
FAULT type=IllegalInstruction task=faulty_task
IPC delivered fault_event to=supervisor_task
SUPERVISOR decision=Kill task=faulty_task
RECOVERY_APPLIED task=faulty_task policy=Kill
LOGGER event=TASK_FAULTED
```

Required tests:

```sh
cargo test --target x86_64-unknown-linux-gnu -p kernel
cargo test --manifest-path userland/supervisor/Cargo.toml --target x86_64-unknown-linux-gnu
./tests/supervisor_qemu_test.sh
```

Gate:

v0.8 is complete only if:

* supervisor runs on target
* logger runs on target
* fault events reach supervisor
* recovery decision is applied
* logger receives event
* supervisor cannot bypass capabilities

---

# 18. Stage 8 — v0.9 Full Four-Task Fault-Containment Demo

Goal:

Run the full demo required by the project charter.

Tasks:

* critical_task
* supervisor_task
* logger_task
* faulty_task

faulty_task attacks:

1. illegal syscall
2. illegal memory access
3. illegal IPC
4. CPU exhaustion
5. repeated crash

Expected output:

```text
TASK_STARTED task=critical_task
TASK_STARTED task=supervisor_task
TASK_STARTED task=logger_task
TASK_STARTED task=faulty_task
FAULT type=IllegalSyscall task=faulty_task
CAP_DENIED task=faulty_task syscall=sys_send
PAGE_FAULT task=faulty_task
WATCHDOG_TIMEOUT task=faulty_task
RECOVERY_APPLIED task=faulty_task policy=Restart
CRITICAL_TASK alive=true
KERNEL alive=true
DEMO result=pass
```

Required tests:

```sh
./tests/full_fault_containment_demo_qemu_test.sh
cargo test --target x86_64-unknown-linux-gnu -p kernel
coqc proofs/coq/MemoryIsolation.v
coqc proofs/coq/CapabilityAccess.v
coqc proofs/coq/SchedulerPriority.v
```

Gate:

v0.9 is complete only if:

* full four-task demo passes
* repeated crash does not kill kernel
* critical_task continues
* invalid IPC never succeeds
* illegal memory access becomes page fault
* CPU exhaustion becomes watchdog timeout
* supervisor applies policy
* logger receives evidence events

---

# 19. Stage 9 — v1.0 Industrial Evaluation Kit

Goal:

Package AxiomRT as an industrial evaluation artifact.

Create:

```text
AxiomRT_v1.0_Industrial_Evaluation_Kit/
├── source/
├── docs/
├── evidence/
├── proofs/
├── demo/
├── scripts/
├── tests/
├── README.md
├── LIMITATIONS.md
├── ASSUMPTIONS_OF_USE.md
├── SAFETY_CONCEPT.md
├── SECURITY_CONCEPT.md
├── VERIFICATION_REPORT.md
├── TEST_REPORT.md
└── FINAL_REPORT.pdf
```

Required commands:

```sh
mkdir -p release/AxiomRT_v1.0_Industrial_Evaluation_Kit
mkdir -p release/AxiomRT_v1.0_Industrial_Evaluation_Kit/{source,docs,evidence,proofs,demo,scripts,tests}

git archive --format=tar --output=release/axiomrt_v1.0_source.tar HEAD

cp -r docs release/AxiomRT_v1.0_Industrial_Evaluation_Kit/
cp -r proofs release/AxiomRT_v1.0_Industrial_Evaluation_Kit/
cp -r evidence release/AxiomRT_v1.0_Industrial_Evaluation_Kit/
cp -r scripts release/AxiomRT_v1.0_Industrial_Evaluation_Kit/
cp -r tests release/AxiomRT_v1.0_Industrial_Evaluation_Kit/
```

Gate:

v1.0 is complete only if:

* external evaluator can build
* external evaluator can run QEMU demo
* external evaluator can run tests
* limitations are explicit
* no certification claim is made
* all evidence is reproducible

Tag:

```sh
git tag -a v1.0-industrial-eval -m "AxiomRT v1.0 Industrial Evaluation Kit"
```

---

# 20. Stage 10 — v1.1 Real Hardware Board Support

Goal:

Move from QEMU to real hardware.

Requirements:

* choose RISC-V board with MMU
* document boot chain
* create BSP layer
* UART support
* timer support
* interrupt controller support
* memory map
* Sv39 layout
* same isolation demo on board

Gate:

v1.1 is complete only if:

* board boots
* UART banner appears
* U-mode runs
* page fault containment works
* critical task survives faulty task on real hardware

---

# 21. Stage 11 — v1.2 User-Space Driver Framework

Goal:

Add drivers outside the kernel.

Rules:

* no complex driver in kernel
* device access only by capability
* driver crash must be containable
* supervisor can restart driver

Required tasks:

* Device capability
* MMIO mapping capability
* UART user-space driver
* driver fault event
* driver restart
* driver isolation test

Gate:

v1.2 is complete only if:

* user-space driver can crash
* kernel survives
* critical task survives
* supervisor can restart driver

---

# 22. Stage 12 — v1.3 Stronger Real-Time Scheduling

Goal:

Move beyond fixed-priority to stronger safety scheduling.

Options:

* budget-based scheduling
* mixed-criticality scheduling
* temporal partitioning
* deadline monitoring
* WCET assumptions

Gate:

v1.3 is complete only if:

* low-criticality task cannot consume high-criticality budget
* scheduler behavior is deterministic
* timing faults create structured events
* proof/model obligations are updated

---

# 23. Stage 13 — v1.4 Formal Refinement

Goal:

Connect Coq models to Rust implementation.

Required work:

* abstract state model
* implementation state mapping
* page table refinement
* capability lookup refinement
* scheduler selection refinement
* CI for Coq
* explicit assumption registry
* no hidden admitted theorem

Commands:

```sh
coqc proofs/coq/MemoryIsolation.v
coqc proofs/coq/CapabilityAccess.v
coqc proofs/coq/SchedulerPriority.v
```

Gate:

v1.4 is complete only if:

* model-level theorem boundaries are explicit
* implementation refinement obligations are tracked
* no proof claim exceeds the verified relation
* all Coq files compile in CI

---

# 24. Stage 14 — v1.5 Robustness Campaign

Goal:

Attack the system.

Required tests:

* syscall fuzzing
* IPC fuzzing
* invalid capability fuzzing
* page fault stress
* scheduler stress
* repeated restart
* timer storm
* stack boundary tests
* UART/event overflow tests
* regression test for every bug

Gate:

v1.5 is complete only if:

* no known user-controlled input crashes kernel
* every discovered bug has regression test
* test logs are archived

---

# 25. Stage 15 — v1.6 Safety Evidence Package

Goal:

Create safety evidence, not certification claim.

Required documents:

```text
01_SYSTEM_REQUIREMENTS.md
02_SAFETY_CONCEPT.md
03_HAZARD_ANALYSIS.md
04_FMEA.md
05_FTA.md
06_THREAT_MODEL.md
07_ARCHITECTURE.md
08_VERIFICATION_REPORT.md
09_TEST_REPORT.md
10_ASSUMPTIONS_OF_USE.md
11_LIMITATIONS.md
12_TRACEABILITY_MATRIX.md
13_SAFETY_MANUAL_DRAFT.md
```

Gate:

v1.6 is complete only if:

* every requirement maps to code
* every requirement maps to test
* safety-critical requirements map to proof/model where possible
* limitations are explicit
* no certification claim is made

---

# 26. Stage 16 — v1.7 Security Evidence Package

Goal:

Create security evidence.

Required documents:

* threat model
* attack surface map
* syscall validation evidence
* capability model evidence
* deny-by-default evidence
* no raw object access evidence
* fault injection logs
* TCB boundary
* OpenSBI assumption boundary
* information-flow limitations

Gate:

v1.7 is complete only if:

* no security claim exists without test/proof reference
* all exposed syscalls have validation evidence
* all protected objects require capability lookup

---

# 27. Stage 17 — v1.8 Documentation Freeze

Goal:

Make project understandable by an external engineer.

Required docs:

```text
README.md
BUILD.md
RUN_QEMU.md
RUN_HARDWARE.md
ARCHITECTURE.md
API.md
KERNEL_OBJECTS.md
SYSCALLS.md
CAPABILITIES.md
FAULTS.md
MONITORING.md
VERIFICATION.md
TESTING.md
LIMITATIONS.md
ROADMAP.md
```

Gate:

v1.8 is complete only if:

* new engineer can build in one day
* new engineer can run demo
* new engineer can run tests
* new engineer can understand limitations
* no tribal knowledge is required

---

# 28. Stage 18 — v2.0 External Pilot / Research Partner

Goal:

Get first external technical evaluation.

Target partners:

* university safety lab
* embedded systems lab
* drone startup
* robotics startup
* automotive Tier-2 R&D
* aerospace R&D group

Do not sell as certified OS.

Correct wording:

AxiomRT is an evaluation-stage safety microkernel runtime for high-assurance embedded prototypes.

Gate:

v2.0 is complete only if:

* external partner runs demo
* external partner provides technical feedback
* feedback is converted into issue list
* no claim exceeds evidence

---

# 29. Stage 19 — v2.1 Board Support Package Product

Goal:

Make supported hardware package.

Required:

* QEMU BSP
* RISC-V board BSP
* ARM feasibility study
* demo apps
* integration guide
* support contract draft
* release notes
* versioning policy

Gate:

v2.1 is complete only if:

* partner can run AxiomRT on supported hardware
* assumptions are documented
* BSP has repeatable build and test process

---

# 30. Stage 20 — v3.0 Certification Path Preparation

Goal:

Prepare for certification route.

Do not claim certification.

Required before certification discussion:

* stable architecture
* stable requirements
* traceability matrix
* safety manual
* coding standard
* toolchain qualification strategy
* static analysis
* coverage strategy
* independent review
* hazard analysis
* FMEA
* FTA
* fault injection evidence
* configuration management
* change control
* long-term maintenance plan
* liability model
* support model

Gate:

v3.0 is complete only if:

* certification consultant can review package
* standard-specific path is identified
* gaps are listed
* cost/time/risk are documented

---

# 31. Automatic Next-Step Algorithm

At the end of every session, perform this algorithm:

1. Run `git status`.
2. Identify current version and phase.
3. Check latest completed gate.
4. If current gate incomplete, generate the next missing task.
5. If current gate complete, create evidence.
6. Commit evidence.
7. Tag if phase final.
8. Move to next phase.
9. Generate the next task prompt.
10. Do not stop with vague advice.

If blocked:

* state the exact blocker
* state exact file/command/error
* create smallest recovery task
* do not skip the phase

---

# 32. Final Definition of Done

The project is considered complete only when:

* AxiomRT boots on QEMU.
* AxiomRT boots on at least one real board.
* Sv39/MMU memory isolation is active.
* Multiple U-mode tasks run.
* Timer preemption works.
* Watchdog detects CPU exhaustion.
* Capability-controlled IPC works on target.
* Supervisor receives fault events on target.
* Logger receives structured events on target.
* Full four-task demo passes.
* Critical task survives faulty task attacks.
* Kernel survives user faults.
* Deterministic host tests pass.
* QEMU tests pass.
* Hardware tests pass.
* Coq models compile.
* Refinement obligations are explicit.
* Safety evidence package exists.
* Security evidence package exists.
* Industrial evaluation kit exists.
* External evaluator can build, run, test, and understand the system.
* No certification claim is made before actual certification work.

Until all of this is true, the project is not complete.

---

# 33. First Action Now

Start from the current repository.

Run:

```sh
pwd
git status
git log --oneline --decorate -n 10
```

Then execute Stage 0:

```sh
mkdir -p evidence/v0.1

./scripts/run_qemu.sh | tee evidence/v0.1/qemu_demo.log

./tests/boot_smoke_test.sh | tee evidence/v0.1/boot_smoke.log

cargo test --target x86_64-unknown-linux-gnu -p kernel \
  | tee evidence/v0.1/host_tests.log

cargo test --manifest-path userland/supervisor/Cargo.toml \
  --target x86_64-unknown-linux-gnu \
  | tee evidence/v0.1/supervisor_tests.log

coqc proofs/coq/MemoryIsolation.v \
  | tee evidence/v0.1/coq_memory.log

coqc proofs/coq/CapabilityAccess.v \
  | tee evidence/v0.1/coq_capability.log

coqc proofs/coq/SchedulerPriority.v \
  | tee evidence/v0.1/coq_scheduler.log

git rev-parse HEAD > evidence/v0.1/git_commit.txt
git log --oneline --decorate > evidence/v0.1/git_history.txt
rustc --version > evidence/v0.1/rust_version.txt
qemu-system-riscv64 --version > evidence/v0.1/qemu_version.txt
coqc --version > evidence/v0.1/coq_version.txt

git add evidence/v0.1
git commit -m "AXIOM-EVIDENCE-001: archive v0.1 final evidence"
git tag -a v0.1-final -m "AxiomRT v0.1 final evaluation baseline"
```

After Stage 0 passes, start Stage 1:

AxiomRT v0.2 — Sv39/MMU Hardware Memory Isolation.
