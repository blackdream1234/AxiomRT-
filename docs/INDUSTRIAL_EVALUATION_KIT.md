# AxiomRT Safety Core — Industrial Evaluation Kit

Document ID: created by AXIOM-KIT-001 (Phase 13)
Requirement reference: docs/00_PROJECT_CHARTER.md §4, Project
Description §24–§25.

## 1. Product Definition

The AxiomRT Safety Core Industrial Evaluation Kit is an
**evaluation-stage** package of a formally specified,
microkernel-based safety runtime for high-assurance embedded systems.
It exists so that companies, labs, and engineering teams can evaluate
the architecture, the isolation and fault-containment mechanisms, the
documentation discipline, and the verification approach.

It is **not** sold as a certified OS. No certification claim is made
anywhere in this kit. Precise positioning language: formally
specified, safety-oriented, microkernel-based, high-assurance,
evaluation-stage, certification-oriented, designed for isolation and
controlled recovery.

## 2. What Is Included

* **Source code** — RISC-V 64 microkernel in Rust `no_std`
  (zero external dependencies), minimal assembly (boot, trap
  entry/exit, user entry), user-space supervisor service crate.
* **QEMU boot path** — `scripts/run_qemu.sh` boots the kernel on the
  QEMU `virt` machine through OpenSBI; build instructions in
  docs/09_BUILD_AND_BOOT.md.
* **Demo scenario** — fault containment demonstration
  (docs/DEMO_SCENARIO.md, examples/fault_containment_demo/).
* **Architecture documentation** — complete Phase 0 blueprint:
  charter, scope, kernel blueprint, kernel objects, syscall model,
  memory model, fault model (docs/INDEX.md).
* **Test suite** — QEMU boot smoke test plus 100+ deterministic
  host-run unit/integration tests covering memory model, thread
  lifecycle, scheduler, IPC, capabilities, fault handling, monitoring
  (docs/14_TEST_STRATEGY.md).
* **Verification notes** — verification plan
  (docs/11_VERIFICATION_PLAN.md) and compiled Coq starter models with
  proven model-level theorems (proofs/).
* **Runtime monitoring** — structured serial event format for
  evidence collection (docs/11_RUNTIME_MONITORING.md).

## 3. What Is Not Included

* a certified product, or any artifact claiming compliance with
  ISO 26262, DO-178C, IEC 61508, or any other standard
* hardware board support (v0.1 is emulator-only)
* filesystem, network stack, GUI, POSIX layer, shell
* multicore support
* shared-memory IPC
* completed end-to-end formal proofs (starter models with explicit
  refinement TODOs are included)
* production support commitments

## 4. Target Users

* automotive/aerospace R&D groups evaluating microkernel safety
  runtimes for prototypes
* drone, robotics, and industrial-control teams needing strong
  isolation with a small trusted computing base
* safety and security research labs
* certification-methodology researchers interested in the
  document-first, traceable engineering process

## 5. Demo Scenario

The fault containment demo (docs/DEMO_SCENARIO.md): a faulty task
attacks the system (illegal syscall path, privileged instruction,
capability-less IPC) while the kernel contains every attempt,
produces structured evidence on the serial port, and continues
running. Expected output is documented line-by-line; the demo is
understandable without reading source code.

## 6. Safety Evidence

* Fault model with total behavior: eight fault types, each with
  source, severity, kernel action, notification, recovery options,
  and logging fields — no undefined fault behavior
  (docs/06_FAULT_MODEL.md).
* Demonstrated on target: a user-task fault is contained; the kernel
  survives and continues (docs/10_USER_MODE.md §6).
* Thread lifecycle rules enforce that killed tasks stay dead and
  faulted tasks never continue without an explicit recovery decision
  (unit-tested).
* Deterministic scheduling with proven priority exclusion at model
  level (proofs/coq/SchedulerPriority.v).

## 7. Security Evidence

* Capability-based access control: eight explicit rights, single
  enforcement point with fixed check order, diminish-only derivation
  (docs/06_CAPABILITY_MODEL.md; proofs/coq/CapabilityAccess.v).
* Demonstrated on target: syscall IPC without a capability fails
  closed with a CAP_DENIED event, from user mode, through the real
  lookup path.
* Supervisor is trusted for policy only: it receives fault events
  exclusively through capability-checked IPC — no capability, no
  events (tested).
* W^X and kernel/user memory separation rules are structural in the
  memory model (docs/05_MEMORY_MODEL.md) with negative tests.

## 8. Verification Evidence

* Verification plan with explicit method rules
  (docs/11_VERIFICATION_PLAN.md).
* Coq starter models compile (Coq 8.20) with model-level theorems
  **proven** for memory isolation, capability access, and scheduler
  priority; refinement obligations to the implementation are stated
  and marked TODO — the gap is visible, never implicit.
* 100+ deterministic host tests plus a QEMU boot smoke test; every
  kernel mechanism carries positive and negative tests.
* Traceability: every implementation file references its Phase 0
  document; every commit maps to exactly one task ID.

## 9. Known Limitations (explicit)

* **Memory isolation is not yet hardware-enforced**: the MMU (Sv39)
  is not activated in v0.1; the memory model is specification +
  tested model code. Privilege isolation (U/S mode) IS enforced and
  demonstrated (docs/10_USER_MODE.md §5).
* Single hart, single user task on target; multi-task scheduling is
  model-level (host-tested), not yet dispatched on target.
* Syscalls beyond the capability gate are stubs
  (`ERR_NOT_IMPLEMENTED`) until their integration phases land.
* IPC rendezvous, fault-event wire, and supervisor policy run and are
  tested on host; on-target integration follows MMU activation.
* Timer preemption and watchdog are modeled but not driven by a
  hardware timer yet.
* Emulator only; no physical board has ever run this kernel.
* Formal proofs cover models, not the implementation (refinement
  TODOs listed in the proof files).

## 10. Assumptions of Use

* Evaluation happens on QEMU `virt` (RISC-V 64) with the bundled
  OpenSBI firmware; OpenSBI and QEMU correctness are assumed.
* The evaluator toolchain (Rust stable + riscv64gc-unknown-none-elf,
  QEMU ≥ 7, optionally Coq 8.20) matches
  docs/09_BUILD_AND_BOOT.md.
* The kit is used for evaluation, research, and prototyping only —
  not for controlling real vehicles, aircraft, or safety-critical
  machinery.
* Security claims hold within the stated boundary: single hart, no
  DMA-capable devices exposed to user tasks, supervisor policy
  configured as documented.
