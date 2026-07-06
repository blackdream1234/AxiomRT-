# AxiomRT Real OS Master Prompt — Full Completion Mode

You are working on the repository:

https://github.com/blackdream1234/AxiomRT-

Project name:

AxiomRT

Current status:

AxiomRT has reached v1.0 Industrial Evaluation Kit. The current system boots on QEMU RISC-V 64 through OpenSBI and demonstrates MMU-enforced memory isolation for tested cases, multi-task scheduling, timer preemption, watchdog containment, synchronous bounded IPC, capability enforcement, supervisor/logger recovery, and a full four-task fault-containment demo.

However, this is not yet a complete real operating system.

The current limitations are:

* emulator-only,
* no physical board support,
* single hart only,
* no user-facing OS shell,
* no real application loader,
* no filesystem service,
* no storage service,
* no device-driver framework beyond demo-level mechanisms,
* no installer,
* no developer CLI,
* no OS dashboard,
* no external audit,
* no full model-to-code formal refinement,
* no certification claim,
* no production-readiness claim.

Your mission is to continue the project until AxiomRT becomes a real, runnable, user-facing operating system for high-assurance embedded systems.

Do not stop at demos.

Do not stop at reports.

Do not stop at QEMU-only success.

Do not call the project complete until all final gates in this prompt are satisfied.

---

# 1. Final Goal

Build AxiomRT into a real microkernel-based operating system.

The final system must include:

1. A small trusted microkernel.
2. Hardware-enforced memory isolation.
3. Multiple isolated user-space processes.
4. Preemptive scheduling.
5. Capability-based access control.
6. Synchronous bounded IPC.
7. Fault containment.
8. Supervisor recovery.
9. Runtime monitoring.
10. User-space driver model.
11. User-space filesystem service.
12. User-space storage service.
13. User-space console service.
14. User-space shell.
15. User application loading.
16. Developer CLI.
17. Local graphical dashboard for demos/evidence.
18. One-command installer.
19. QEMU support.
20. Real hardware board support.
21. Reproducible verification.
22. Evidence archive.
23. Formal models with explicit refinement status.
24. Robustness/fuzz testing.
25. Documentation for users and developers.
26. Release package.
27. External-review readiness.

The final system must run both:

* in QEMU, and
* on at least one real RISC-V board with MMU support.

---

# 2. Absolute Rule

No work is finished until the system is real.

AxiomRT is not complete if it only boots in QEMU.

AxiomRT is not complete if it has no user shell.

AxiomRT is not complete if it has no application loading.

AxiomRT is not complete if it has no filesystem/storage story.

AxiomRT is not complete if it has no hardware board support.

AxiomRT is not complete if a new user cannot install, build, run, test, and understand it.

AxiomRT is not complete if safety/security claims exceed evidence.

AxiomRT is not complete if the kernel contains unnecessary complexity.

---

# 3. Architecture Law

AxiomRT remains a microkernel.

The kernel may contain only:

* boot,
* trap handling,
* interrupt routing,
* address spaces,
* page tables,
* thread/process model,
* scheduler,
* IPC mechanism,
* capability lookup,
* syscall validation,
* timer mechanism,
* fault event creation,
* minimal hardware abstraction required for boot and isolation.

The kernel must not contain:

* GUI,
* filesystem logic,
* network stack,
* shell,
* package manager,
* complex drivers,
* dynamic policy,
* AI logic,
* user account system,
* high-level logging backend,
* application framework.

Everything complex must run in isolated user space.

---

# 4. User-Space OS Services

The complete OS must have these user-space services:

1. `init_service`

   * first user-space service,
   * starts other services,
   * owns boot policy.

2. `supervisor_service`

   * receives fault events,
   * applies recovery policy,
   * kill/restart/suspend decisions.

3. `logger_service`

   * receives structured kernel/user events,
   * exports serial logs and evidence logs.

4. `console_service`

   * owns console input/output,
   * exposes terminal to shell.

5. `shell_service`

   * interactive user shell,
   * allows user to inspect tasks, memory, capabilities, events, and services.

6. `fs_service`

   * user-space filesystem service,
   * starts with simple read-only filesystem,
   * later supports writable storage.

7. `storage_service`

   * user-space block device abstraction,
   * QEMU virtio-blk first,
   * hardware storage later.

8. `driver_manager`

   * starts user-space drivers,
   * grants device capabilities,
   * restarts failed drivers.

9. `app_loader`

   * loads static applications first,
   * then ELF or a restricted executable format.

10. `health_monitor`

* monitors heartbeats,
* exposes health state to shell and dashboard.

---

# 5. Host-Side Tools

The complete project must also include host-side tools.

Do not confuse these tools with in-kernel features.

Required tools:

1. `axiomctl`

   * developer CLI,
   * runs builds,
   * runs QEMU,
   * runs tests,
   * opens evidence,
   * builds releases.

2. `AxiomRT Studio`

   * local graphical dashboard,
   * visualizes task state,
   * faults,
   * IPC,
   * capabilities,
   * watchdog,
   * scheduler events,
   * test results,
   * proof status,
   * limitations.

3. `install.sh`

   * one-command setup script,
   * installs/checks dependencies,
   * configures Rust target,
   * checks QEMU,
   * checks Coq,
   * runs boot smoke test.

4. GitHub Actions CI

   * build,
   * tests,
   * QEMU tests,
   * Coq,
   * clippy,
   * formatting,
   * release packaging.

---

# 6. Task Format

Every task must use this exact structure:

Task ID:
AXIOM-AREA-NNN

Phase:
Current phase.

Goal:
One precise goal.

Requirement reference:
Which document or previous phase justifies this.

Allowed files:
Exact file list.

Forbidden files:
All other files unless explicitly stated.

Expected behavior:
Observable result.

Commands:
Exact commands to run.

Tests required:
Exact tests.

Documentation update:
Required document changes.

Safety impact:
How this affects isolation, determinism, containment, or authority.

Security impact:
How this affects attack surface or authority.

Verification impact:
Tests, proof, refinement, evidence.

Rollback condition:
When to revert.

Definition of done:
Concrete checklist.

Commit message:
AXIOM-AREA-NNN: short imperative summary

---

# 7. Mandatory Session Algorithm

At the start of every work session:

```sh
pwd
git status
git branch --show-current
git log --oneline --decorate -n 20
git tag --list
```

Then:

1. Identify the current phase.
2. Check the phase gate.
3. If the phase gate is incomplete, create the smallest next task.
4. Execute only that task.
5. Run the required commands.
6. Save evidence.
7. Commit one task.
8. Move to the next task.
9. Do not jump.
10. Do not add features outside the current phase.

At the end of every session:

```sh
git status
./scripts/verify_all.sh
```

If `verify_all.sh` fails, stop all feature work and fix the regression.

---

# 8. Current First Task

Start with release hygiene.

Task ID:
AXIOM-REL-001

Phase:
v1.0.1 clean release

Goal:
Clean the current v1.0 release before building the full OS roadmap.

Actions:

1. Fix Rust warnings shown by `./scripts/verify_all.sh`.
2. Fix README phase-map inconsistency.
3. Update README to say:

   * current milestone: `v1.0-industrial-eval`,
   * next milestone: `v1.0.1-clean`,
   * next product direction: real OS completion,
   * next software phase: developer tooling + user-facing shell,
   * next hardware phase: real RISC-V board support.
4. Run full verification.
5. Save clean verification log.

Allowed files:

* README.md
* kernel/src/ipc/message.rs
* kernel/src/fault/mod.rs
* evidence/v1.0/verify_all_clean.log

Commands:

```sh
./scripts/verify_all.sh | tee evidence/v1.0/verify_all_clean.log
```

Definition of done:

* `VERIFY ALL: PASS`
* no Rust warnings
* README is consistent
* no scope expansion
* no kernel behavior change

Commit:

```sh
git add README.md kernel/src/ipc/message.rs kernel/src/fault/mod.rs evidence/v1.0/verify_all_clean.log
git commit -m "AXIOM-REL-001: clean v1.0 release hygiene"
git tag -a v1.0.1-clean -m "AxiomRT v1.0.1 clean evaluation release"
```

---

# 9. Phase 1 — Product Definition for Real OS

Task ID:
AXIOM-PRODUCT-001

Goal:
Create the real OS product definition.

Create:

```text
docs/20_REAL_OS_PRODUCT_DEFINITION.md
```

Required content:

1. What AxiomRT Real OS is.
2. What is different from the evaluation kit.
3. Final real OS architecture.
4. Kernel responsibilities.
5. User-space service responsibilities.
6. Host tooling responsibilities.
7. Hardware targets.
8. User experience.
9. Developer experience.
10. Final definition of done.
11. Explicit non-goals.
12. Certification boundary.

Definition of done:

The document must clearly distinguish:

* evaluation kit,
* real OS,
* certification-ready product,
* certified product.

Commit:

```sh
git add docs/20_REAL_OS_PRODUCT_DEFINITION.md
git commit -m "AXIOM-PRODUCT-001: define real OS product target"
```

---

# 10. Phase 2 — Developer CLI: axiomctl

Goal:
Create a real developer tool.

Final command:

```sh
axiomctl
```

Required subcommands:

```sh
axiomctl doctor
axiomctl build
axiomctl run
axiomctl demo memory
axiomctl demo full
axiomctl verify
axiomctl evidence list
axiomctl evidence open
axiomctl kit build
axiomctl release check
```

Tasks:

AXIOM-CLI-001:
Create `tools/axiomctl` Rust CLI skeleton.

AXIOM-CLI-002:
Implement `doctor`.

AXIOM-CLI-003:
Implement `build`.

AXIOM-CLI-004:
Implement `run`.

AXIOM-CLI-005:
Implement `demo memory` and `demo full`.

AXIOM-CLI-006:
Implement `verify`.

AXIOM-CLI-007:
Implement evidence commands.

AXIOM-CLI-008:
Implement kit build command.

AXIOM-CLI-009:
Implement release check.

Gate:

A new user can run:

```sh
cargo run -p axiomctl -- doctor
cargo run -p axiomctl -- demo full
cargo run -p axiomctl -- verify
```

and get useful output without reading internal scripts.

---

# 11. Phase 3 — Structured Event Format

Goal:
Make kernel/demo output machine-readable for UI and evidence tools.

Create:

```text
docs/21_EVENT_FORMAT.md
```

Add optional event format:

```json
{
  "time": 1,
  "kind": "fault",
  "task": "faulty_task",
  "fault": "WatchdogTimeout",
  "action": "Kill",
  "kernel": "alive"
}
```

Tasks:

AXIOM-EVENT-001:
Document event schema.

AXIOM-EVENT-002:
Add host-side parser for current serial logs.

AXIOM-EVENT-003:
Add JSON export in axiomctl.

AXIOM-EVENT-004:
Add tests for parser.

Gate:

The full demo output can be parsed into:

* task events,
* scheduler events,
* IPC events,
* capability events,
* fault events,
* watchdog events,
* recovery events.

---

# 12. Phase 4 — AxiomRT Studio Dashboard

Goal:
Create the user-facing graphical interface.

This is host-side only.

Do not put GUI inside the kernel.

Directory:

```text
studio/
```

Technology:

Use Next.js + Tailwind or another simple local web UI.

Required pages:

```text
/
 /run
 /tasks
 /scheduler
 /faults
 /ipc
 /capabilities
 /tests
 /proofs
 /evidence
 /limitations
 /release
```

Required panels:

1. System status.
2. Run full demo button.
3. Terminal output.
4. Event timeline.
5. Task table.
6. Fault table.
7. IPC table.
8. Capability-denial table.
9. Test results.
10. Coq status.
11. Evidence archive viewer.
12. Limitations viewer.
13. Release builder.

Tasks:

AXIOM-STUDIO-001:
Create UI skeleton.

AXIOM-STUDIO-002:
Add backend wrapper to call `axiomctl`.

AXIOM-STUDIO-003:
Run full demo from UI.

AXIOM-STUDIO-004:
Parse events into timeline.

AXIOM-STUDIO-005:
Display task table.

AXIOM-STUDIO-006:
Display fault/IPC/capability panels.

AXIOM-STUDIO-007:
Display test and Coq results.

AXIOM-STUDIO-008:
Display evidence archive.

AXIOM-STUDIO-009:
Add release page.

Gate:

A user can open AxiomRT Studio, click “Run Full Demo,” and see the OS behavior visually.

---

# 13. Phase 5 — One-Command Installer

Goal:
Make setup easy.

Create:

```text
install.sh
```

It must:

1. Detect Linux distribution.
2. Check Rust.
3. Check cargo.
4. Check QEMU RISC-V.
5. Check Coq.
6. Install missing packages where possible.
7. Add Rust target `riscv64gc-unknown-none-elf`.
8. Build AxiomRT.
9. Run boot smoke test.
10. Print next commands.

Required final usage:

```sh
./install.sh
```

Gate:

A fresh Linux machine can install dependencies and run:

```sh
axiomctl demo full
```

---

# 14. Phase 6 — CI/CD

Goal:
Every push proves project health.

Create:

```text
.github/workflows/verify.yml
.github/workflows/qemu.yml
.github/workflows/coq.yml
.github/workflows/clippy.yml
.github/workflows/release.yml
```

Required checks:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
./scripts/verify_all.sh
coqc proofs/coq/MemoryIsolation.v
coqc proofs/coq/CapabilityAccess.v
coqc proofs/coq/SchedulerPriority.v
```

Gate:

GitHub Actions shows green status for main.

---

# 15. Phase 7 — Real OS Boot Flow

Goal:
Move from demo boot to OS boot sequence.

Required boot order:

1. OpenSBI.
2. AxiomRT kernel.
3. Kernel initializes memory.
4. Kernel initializes timer.
5. Kernel initializes root address space.
6. Kernel starts `init_service`.
7. `init_service` starts:

   * supervisor,
   * logger,
   * console,
   * shell,
   * driver manager,
   * filesystem service.
8. Shell becomes interactive.

Tasks:

AXIOM-INIT-001:
Document OS boot sequence.

AXIOM-INIT-002:
Create `init_service`.

AXIOM-INIT-003:
Start supervisor/logger from init.

AXIOM-INIT-004:
Start console service.

AXIOM-INIT-005:
Start shell service.

Gate:

QEMU boot ends with an interactive shell prompt:

```text
axiom>
```

---

# 16. Phase 8 — Console and Shell

Goal:
Create the first real user interface inside the OS.

Shell commands:

```text
help
version
tasks
faults
ipc
caps
memory
uptime
events
run demo
kill <task>
restart <task>
clear
shutdown
```

Tasks:

AXIOM-SHELL-001:
Document shell design.

AXIOM-SHELL-002:
Implement console input.

AXIOM-SHELL-003:
Implement shell command parser.

AXIOM-SHELL-004:
Implement `help`, `version`, `uptime`.

AXIOM-SHELL-005:
Implement `tasks`.

AXIOM-SHELL-006:
Implement `faults`.

AXIOM-SHELL-007:
Implement `ipc` and `caps`.

AXIOM-SHELL-008:
Implement `run demo`.

AXIOM-SHELL-009:
Implement controlled shutdown.

Gate:

A user can boot AxiomRT and interact with:

```text
axiom> help
axiom> tasks
axiom> run demo
```

---

# 17. Phase 9 — Application Model

Goal:
Run real user applications, not only built-in demos.

Start simple.

Application format stages:

1. Static built-in app table.
2. Position-independent app image.
3. Restricted ELF loader.
4. Signed app manifest later.

Required components:

```text
userland/apps/
userland/app_loader/
docs/22_APPLICATION_MODEL.md
```

Tasks:

AXIOM-APP-001:
Document application model.

AXIOM-APP-002:
Create static app manifest.

AXIOM-APP-003:
Create app loader service.

AXIOM-APP-004:
Load app into separate address space.

AXIOM-APP-005:
Assign capabilities by manifest.

AXIOM-APP-006:
Start/stop app from shell.

Gate:

User can run:

```text
axiom> run hello
axiom> run fault_demo
```

and each app runs isolated.

---

# 18. Phase 10 — Filesystem Service

Goal:
Add filesystem as user-space service.

Do not put filesystem in kernel.

Start with read-only filesystem.

Stages:

1. initramfs-like embedded read-only archive,
2. read-only file service over IPC,
3. simple writable RAM filesystem,
4. block-backed filesystem later.

Tasks:

AXIOM-FS-001:
Document filesystem service.

AXIOM-FS-002:
Define file-service IPC protocol.

AXIOM-FS-003:
Implement read-only embedded file archive.

AXIOM-FS-004:
Expose `ls` and `cat` in shell.

AXIOM-FS-005:
Add tests.

Gate:

User can run:

```text
axiom> ls
axiom> cat /etc/version
```

---

# 19. Phase 11 — Storage Service

Goal:
Support persistent or block-backed storage.

Start with QEMU virtio-blk.

Keep driver in user space when possible.

Tasks:

AXIOM-STOR-001:
Document storage architecture.

AXIOM-STOR-002:
Define block-device capability.

AXIOM-STOR-003:
Add QEMU virtio-blk investigation document.

AXIOM-STOR-004:
Implement minimal block driver prototype.

AXIOM-STOR-005:
Connect block driver to filesystem service.

Gate:

AxiomRT can read a block-backed image in QEMU.

---

# 20. Phase 12 — User-Space Driver Framework

Goal:
Drivers are isolated services.

Required:

1. device capability,
2. MMIO mapping capability,
3. interrupt delivery,
4. driver restart,
5. driver fault containment.

Tasks:

AXIOM-DRV-001:
Document driver framework.

AXIOM-DRV-002:
Add device object and capability model.

AXIOM-DRV-003:
Add MMIO grant path.

AXIOM-DRV-004:
Add interrupt-to-driver event path.

AXIOM-DRV-005:
Add driver crash test.

Gate:

A user-space driver can crash and the kernel survives.

---

# 21. Phase 13 — Networking Service

Goal:
Add network as user-space service.

Not in kernel.

Start only after storage/shell are stable.

Tasks:

AXIOM-NET-001:
Document network-service design.

AXIOM-NET-002:
Choose QEMU virtio-net path.

AXIOM-NET-003:
Implement minimal packet RX/TX service.

AXIOM-NET-004:
Expose `net status` in shell.

AXIOM-NET-005:
Add safety/security limitations.

Gate:

AxiomRT can send/receive a minimal test packet in QEMU.

---

# 22. Phase 14 — Real Hardware BSP

Goal:
Run AxiomRT on physical hardware.

Choose a RISC-V board with MMU.

Required:

1. board selection document,
2. boot chain,
3. UART,
4. timer,
5. interrupt controller,
6. memory map,
7. Sv39 mapping,
8. page fault containment,
9. shell prompt.

Tasks:

AXIOM-HW-001:
Choose board and document reasons.

AXIOM-HW-002:
Document boot chain.

AXIOM-HW-003:
Create BSP abstraction.

AXIOM-HW-004:
Boot banner on hardware.

AXIOM-HW-005:
Enable MMU on hardware.

AXIOM-HW-006:
Enter U-mode on hardware.

AXIOM-HW-007:
Run shell on hardware.

AXIOM-HW-008:
Run memory isolation test on hardware.

Gate:

The real board shows:

```text
AxiomRT kernel booted
MMU status=enabled
init_service started
shell_service started
axiom>
```

---

# 23. Phase 15 — Robustness Campaign

Goal:
Attack the system.

Required:

1. syscall fuzzing,
2. IPC fuzzing,
3. capability fuzzing,
4. page fault stress,
5. timer storm,
6. watchdog storm,
7. shell parser fuzzing,
8. filesystem protocol fuzzing,
9. driver crash storm,
10. regression tests.

Tasks:

AXIOM-FUZZ-001:
Create fuzzing plan.

AXIOM-FUZZ-002:
Fuzz syscall decoder.

AXIOM-FUZZ-003:
Fuzz IPC messages.

AXIOM-FUZZ-004:
Fuzz capability indexes.

AXIOM-FUZZ-005:
Fuzz shell parser.

AXIOM-FUZZ-006:
Fuzz file-service protocol.

AXIOM-FUZZ-007:
Archive fuzzing evidence.

Gate:

No user-controlled input discovered by fuzzing can crash the kernel.

---

# 24. Phase 16 — Formal Refinement

Goal:
Connect Coq models to the Rust implementation.

Current state:
Coq proves model-level theorems. Refinement-to-code is TODO.

Required:

1. model state,
2. implementation-observable state,
3. memory refinement,
4. capability refinement,
5. scheduler refinement,
6. IPC refinement,
7. fault model refinement,
8. documented assumptions.

Tasks:

AXIOM-REFINE-001:
Document refinement strategy.

AXIOM-REFINE-002:
Connect Rust page table model to Coq memory model.

AXIOM-REFINE-003:
Connect Rust capability table to Coq capability model.

AXIOM-REFINE-004:
Connect Rust scheduler queue to Coq scheduler model.

AXIOM-REFINE-005:
Add IPC refinement model.

AXIOM-REFINE-006:
Add fault containment refinement model.

Gate:

No formal claim exceeds proven or explicitly assumed refinement.

---

# 25. Phase 17 — Safety and Security Evidence

Goal:
Create serious engineering evidence.

Documents:

```text
docs/safety/01_SYSTEM_REQUIREMENTS.md
docs/safety/02_HAZARD_ANALYSIS.md
docs/safety/03_FMEA.md
docs/safety/04_FTA.md
docs/safety/05_TRACEABILITY_MATRIX.md
docs/safety/06_SAFETY_MANUAL_DRAFT.md
docs/security/01_THREAT_MODEL.md
docs/security/02_ATTACK_SURFACE.md
docs/security/03_SECURITY_REQUIREMENTS.md
docs/security/04_SECURITY_TEST_REPORT.md
```

Gate:

Every requirement maps to:

* design,
* code,
* test,
* evidence,
* limitation or proof.

---

# 26. Phase 18 — Release Engineering

Goal:
Create a complete release.

Required artifacts:

```text
AxiomRT_Real_OS_Source.tar.gz
AxiomRT_Real_OS_QEMU_Image.tar.gz
AxiomRT_Real_OS_Evidence.zip
AxiomRT_Real_OS_User_Guide.pdf
AxiomRT_Real_OS_Developer_Guide.pdf
AxiomRT_Studio.zip
axiomctl binary
install.sh
checksums.txt
release_notes.md
```

Tasks:

AXIOM-RELEASE-001:
Document release process.

AXIOM-RELEASE-002:
Build source release.

AXIOM-RELEASE-003:
Build QEMU release.

AXIOM-RELEASE-004:
Build evidence archive.

AXIOM-RELEASE-005:
Build user docs.

AXIOM-RELEASE-006:
Add checksums.

AXIOM-RELEASE-007:
Create GitHub release.

Gate:

A new user can download the release, install it, run QEMU, use shell, run demo, and inspect evidence.

---

# 27. Phase 19 — External Review

Goal:
Make the project credible outside the author.

Required:

1. independent build attempt,
2. independent demo run,
3. independent code review,
4. independent security review,
5. issue list,
6. fixes,
7. signed review notes if possible.

Tasks:

AXIOM-REVIEW-001:
Create external review guide.

AXIOM-REVIEW-002:
Create review checklist.

AXIOM-REVIEW-003:
Track review issues.

AXIOM-REVIEW-004:
Fix critical issues.

Gate:

At least one external engineer can build, run, test, and review AxiomRT.

---

# 28. Phase 20 — Real OS Final Definition of Done

AxiomRT is complete only when all of this is true:

1. Boots in QEMU.
2. Boots on real RISC-V hardware.
3. MMU enabled.
4. Hardware memory isolation tested.
5. Multiple U-mode processes run.
6. Preemptive scheduler works.
7. Watchdog works.
8. IPC works.
9. Capabilities enforced.
10. Faults contained.
11. Supervisor works.
12. Logger works.
13. init service works.
14. Console service works.
15. Shell works.
16. User can run shell commands.
17. User applications can be loaded.
18. Filesystem service works.
19. Storage service works at least in QEMU.
20. User-space driver framework exists.
21. Full demo still passes.
22. QEMU tests pass.
23. Hardware smoke tests pass.
24. Host tests pass.
25. Coq models compile.
26. Refinement boundaries are explicit.
27. Fuzzing campaign exists.
28. CI is green.
29. Installer works.
30. axiomctl works.
31. AxiomRT Studio works.
32. User documentation exists.
33. Developer documentation exists.
34. Evidence package exists.
35. Release package exists.
36. Limitations are honest.
37. No certification claim is made.
38. External reviewer can run the system.

Only then may the project be called:

AxiomRT Real OS Complete Edition.

---

# 29. Forbidden Shortcut

Do not mark complete if:

* only QEMU works,
* no hardware works,
* no shell exists,
* no app loader exists,
* no filesystem service exists,
* no release package exists,
* no installer exists,
* no external review exists,
* tests are skipped,
* warnings are ignored,
* limitations are hidden,
* certification is claimed without certification.

---

# 30. Next Immediate Execution Order

Run these tasks in order:

1. AXIOM-REL-001
2. AXIOM-PRODUCT-001
3. AXIOM-CLI-001 to AXIOM-CLI-009
4. AXIOM-EVENT-001 to AXIOM-EVENT-004
5. AXIOM-STUDIO-001 to AXIOM-STUDIO-009
6. AXIOM-INSTALL-001
7. AXIOM-CI-001
8. AXIOM-INIT-001 to AXIOM-INIT-005
9. AXIOM-SHELL-001 to AXIOM-SHELL-009
10. AXIOM-APP-001 to AXIOM-APP-006
11. AXIOM-FS-001 to AXIOM-FS-005
12. AXIOM-STOR-001 to AXIOM-STOR-005
13. AXIOM-DRV-001 to AXIOM-DRV-005
14. AXIOM-NET-001 to AXIOM-NET-005
15. AXIOM-HW-001 to AXIOM-HW-008
16. AXIOM-FUZZ-001 to AXIOM-FUZZ-007
17. AXIOM-REFINE-001 to AXIOM-REFINE-006
18. AXIOM-RELEASE-001 to AXIOM-RELEASE-007
19. AXIOM-REVIEW-001 to AXIOM-REVIEW-004
20. Final Real OS gate

Do not stop until the final gate passes.

If blocked, create a blocker document:

```text
docs/blockers/BLOCKER_<date>_<phase>.md
```

The blocker document must state:

* exact blocker,
* exact missing dependency,
* current evidence,
* next recovery action,
* whether the project can continue in another phase safely.

Never fake completion.

Never skip hardware by pretending QEMU is enough.

Never confuse evaluation kit with real OS.

End goal:

AxiomRT Real OS Complete Edition.
