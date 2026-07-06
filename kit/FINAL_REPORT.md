# AxiomRT v1.0 — Final Report (Industrial Evaluation Kit)

*(Delivered as Markdown; a PDF can be produced from this file with any
Markdown-to-PDF tool. No PDF is generated in the emulator-only build
environment.)*

## 1. What AxiomRT Is

AxiomRT is a formally specified microkernel-based safety runtime for
high-assurance embedded systems that require strong isolation,
deterministic execution, controlled fault recovery, and
certification-oriented evidence. v1.0 is an **evaluation-stage** kit.

**No certification claim. No production-readiness claim.** Precise
positioning: formally specified, safety-oriented, microkernel-based,
high-assurance, evaluation-stage, certification-oriented, designed for
isolation and controlled recovery.

## 2. What v1.0 Demonstrates (on QEMU RISC-V 64)

Built document-first across ten tagged milestones (`v0.1-final` …
`v0.9-demo`), one commit per task, evidence archived per version:

* **Boots** on QEMU `virt` through OpenSBI (RISC-V 64, Rust `no_std`,
  zero external dependencies).
* **Hardware memory isolation** (Sv39/MMU): user access to kernel or
  unmapped memory, and execution of non-executable pages, take a
  hardware page fault that is contained.
* **Multiple U-mode tasks** with fixed-priority preemptive scheduling;
  a low-priority infinite loop cannot freeze the kernel.
* **Watchdog** detects CPU exhaustion and contains the offending task.
* **Synchronous, bounded, copy-based IPC** between address spaces — no
  shared memory.
* **Capability-based access control** on every IPC operation
  (deny-by-default).
* **Supervisor + logger** receive fault events over capability-checked
  IPC; the supervisor applies a recovery policy.
* **Full four-task fault-containment demo** (the charter's first
  demonstration): a faulty task's illegal IPC is denied and its CPU
  exhaustion contained; the supervisor recovers; the logger records
  evidence; the critical task keeps running; the kernel stays alive.

## 3. Evidence

* **9/9 QEMU** serial-assertion tests pass; **129 host** tests pass;
  **3 Coq** model files compile. See TEST_REPORT.md and
  VERIFICATION_REPORT.md.
* Per-version evidence archives in `evidence/v0.1 … v0.9` (serial
  transcripts, test logs, tool versions, git commit).
* Traceability: every file references a Phase 0 document; every commit
  maps to one `AXIOM-*` task; every stage is a git tag.

## 4. What Is Included in the Kit

`source/` (git-archive tarball), `docs/` (blueprint + per-phase design),
`proofs/` (Coq starters), `evidence/` (per-version archives), `demo/`
(fault-containment demo), `scripts/` (run + verify + kit build),
`tests/` (host + QEMU suites), and the kit documents: LIMITATIONS,
ASSUMPTIONS_OF_USE, SAFETY_CONCEPT, SECURITY_CONCEPT, VERIFICATION_REPORT,
TEST_REPORT, and this report.

Assemble with `./scripts/build_eval_kit.sh`; verify everything with
`./scripts/verify_all.sh`; run the flagship demo with
`cargo build --release --features demo_full && ./scripts/run_qemu.sh`.

## 5. Limitations and Assumptions

Read **LIMITATIONS.md** and **ASSUMPTIONS_OF_USE.md** before drawing any
conclusion. In short: emulator-only, single-hart, memory isolation
MMU-enforced for the tested cases, recovery = Kill on target, and the
model↔code formal refinement is stated but not yet discharged.

## 6. Roadmap Beyond v1.0

v1.1 real-hardware board support → v1.2 user-space drivers → v1.3
stronger real-time scheduling → v1.4 formal refinement → v1.5 robustness
campaign → v1.6/1.7 safety/security evidence packages → v1.8
documentation freeze → v2.0 external pilot → v2.1 BSP product → v3.0
certification-path preparation. These require physical hardware and
external parties and are outside the emulator-only evaluation kit.
