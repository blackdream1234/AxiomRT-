# AxiomRT Full Completion Prompt for Claude Fable 5

You are working on AxiomRT.

Repository:

```text
https://github.com/blackdream1234/AxiomRT-
```

Current verified state:

AxiomRT has reached `v1.3-readonly-fs`.

The system is no longer only a kernel demo. It is now a QEMU-bootable microkernel operating-system prototype with:

* RISC-V 64 QEMU/OpenSBI boot,
* Sv39 MMU enabled,
* U-mode services,
* interactive `axiom>` shell,
* app loader service,
* static user applications,
* read-only user-space filesystem service,
* `ls` / `cat`,
* capability-based access control,
* watchdog containment,
* supervisor/logger chain,
* `axiomctl`,
* AxiomRT Studio,
* installer script,
* evidence archives,
* QEMU test sweep,
* host tests,
* Coq model-level proofs.

Current local claim:

```text
./scripts/verify_all.sh -> VERIFY ALL: PASS
12/12 QEMU tests
zero warnings
clippy -D warnings clean
3 Coq files compile
```

Important boundary:

AxiomRT is still not a complete real operating system until it has storage, driver framework, hardware path, robustness evidence, formal refinement, release package, and external review.

Do not fake completion.

Do not call QEMU-only work “final OS.”

Do not claim certification.

Do not claim production readiness.

---

# 1. Mission

Continue AxiomRT from `v1.3-readonly-fs` to a real OS completion milestone.

The final target is:

```text
AxiomRT Real OS Complete Edition
```

AxiomRT Real OS Complete Edition must have:

1. Microkernel core.
2. Hardware memory isolation.
3. Multiple isolated U-mode tasks.
4. Preemptive scheduler.
5. Capability-based access control.
6. Bounded IPC.
7. Fault containment.
8. Supervisor recovery.
9. Logger/evidence chain.
10. Interactive shell.
11. Application loader.
12. User applications.
13. Read-only filesystem.
14. Storage service.
15. User-space driver framework.
16. Minimal block-device path.
17. Optional minimal networking path if safe.
18. QEMU reproducibility.
19. At least one real RISC-V board boot path.
20. Robustness/fuzzing campaign.
21. Formal refinement roadmap and partial discharge.
22. CI workflows.
23. Installer.
24. CLI.
25. Dashboard.
26. Evidence package.
27. Release package.
28. External review readiness.

---

# 2. Architecture Law

AxiomRT is a microkernel.

The kernel may contain only mechanisms:

* boot,
* trap handling,
* interrupt routing,
* address spaces,
* page tables,
* threads/tasks,
* scheduling,
* IPC,
* capability lookup,
* syscall validation,
* timers,
* fault event creation,
* minimal device/MMIO grant mechanism if strictly required.

The kernel must not contain:

* filesystem policy,
* path parsing,
* block cache policy,
* network stack,
* shell logic,
* application policy,
* GUI,
* package manager,
* dynamic high-level policy,
* AI logic,
* certification claims,
* complex drivers.

Everything complex runs in user space:

* `init_service`,
* `supervisor_service`,
* `logger_service`,
* `console_service`,
* `shell_service`,
* `app_loader_service`,
* `fs_service`,
* `storage_service`,
* future `driver_manager`,
* future block driver,
* future network service.

---

# 3. Mandatory Work Algorithm

At the start of every session run:

```sh
pwd
git status
git branch --show-current
git log --oneline --decorate -n 20
git tag --list
```

Then:

1. Identify the current milestone.
2. Confirm `./scripts/verify_all.sh` passes before new work.
3. Create the smallest next task.
4. Do exactly one task.
5. Run required tests.
6. Archive evidence.
7. Commit with one task ID.
8. Do not jump phases.
9. Do not hide limitations.
10. Do not weaken tests.
11. Do not remove existing commands.
12. Do not add kernel complexity unless the current phase explicitly requires a kernel mechanism.

At the end of every session run:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
./scripts/verify_all.sh
git status
```

If anything fails, stop feature work and fix the regression.

---

# 4. Required Task Format

Every task must use this exact structure:

```text
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
Effect on isolation, determinism, containment, authority.

Security impact:
Effect on attack surface and authority.

Verification impact:
Tests, proof, refinement, evidence.

Rollback condition:
When to revert.

Definition of done:
Concrete checklist.

Commit message:
AXIOM-AREA-NNN: short imperative summary
```

---

# 5. Global Regression Requirements

Every future phase must preserve:

```text
axiom> help
axiom> version
axiom> tasks
axiom> faults
axiom> ipc
axiom> caps
axiom> memory
axiom> uptime
axiom> events
axiom> apps
axiom> app info hello
axiom> run hello
axiom> run counter
axiom> run fault_demo
axiom> run demo
axiom> ls
axiom> ls /etc
axiom> ls /apps
axiom> cat /etc/version
axiom> cat /apps/hello.manifest
axiom> shutdown
```

No phase may remove these unless a replacement is explicitly documented and tested.

---

# 6. Roadmap From Current State to Final

Continue in this order:

```text
v1.4-storage-service
v1.5-user-space-driver-framework
v1.6-storage-backed-fs-and-loader
v1.7-minimal-network-service
v1.8-robustness-fuzzing
v1.9-formal-refinement
v2.0-real-hardware-bsp
v2.1-real-os-beta-release
v2.2-external-review
v3.0-real-os-complete-edition
```

Do not skip phases.

Do not rename a phase complete until its gate passes.

---

# 7. Phase v1.4 — AXIOM-STOR: Storage Service

Goal:

Introduce a storage architecture and first QEMU-backed storage path without putting storage policy or filesystem logic into the kernel.

This phase is read-only first.

No writable filesystem yet.

No dynamic app loading yet.

No hardware yet.

## AXIOM-STOR-001 — Document storage architecture

Create:

```text
docs/29_STORAGE_SERVICE.md
```

Must document:

1. Why storage is user-space.
2. Embedded filesystem vs block-backed storage.
3. Storage service responsibilities.
4. Block protocol.
5. Capability model.
6. Read-only first policy.
7. Future writable risks.
8. QEMU virtio-blk plan.
9. Kernel boundary.
10. Limitations.

Commit:

```text
AXIOM-STOR-001: document storage service architecture
```

## AXIOM-STOR-002 — Define storage IPC protocol

Protocol requests:

```text
INFO
READ block=<n>
READ_RANGE start=<n> count=<m>
```

Responses:

```text
OK block_size=64 blocks=<n> readonly=true
OK data=<safe-ascii-or-hex>
ERR bad_block
ERR too_many_blocks
ERR denied
ERR malformed
```

Rules:

* fixed block size,
* bounded response,
* read-only,
* no dynamic allocation,
* malformed input cannot crash service.

Commit:

```text
AXIOM-STOR-002: define storage IPC protocol
```

## AXIOM-STOR-003 — Static read-only block image

Create static block image:

```text
block 0: storage header/version
block 1: /etc/version mirror
block 2: /docs/about mirror
block 3: app manifest summary
block 4+: reserved
```

Must be static, bounded, and compatible with constrained U-mode service rules.

Commit:

```text
AXIOM-STOR-003: add static read-only block image
```

## AXIOM-STOR-004 — Add storage_service

Create U-mode `storage_service`.

Responsibilities:

* receive bounded IPC,
* parse `INFO` and `READ`,
* validate block numbers,
* return bounded block data,
* reject malformed requests,
* never access kernel memory,
* yield fairly.

Commit:

```text
AXIOM-STOR-004: add storage service
```

## AXIOM-STOR-005 — Storage capabilities

Add rights:

```text
storage_info
storage_read
```

Important:

The shell capability table was already full at v1.3. If capacity must increase, do it explicitly and test that no existing capability is dropped.

Must preserve:

* console capability,
* shell-line endpoint capability,
* info capability,
* task control capability,
* fs capability,
* app loader capability,
* new storage capability.

Apps do not receive storage capability.

`fault_demo` receives none.

Commit:

```text
AXIOM-STOR-005: enforce storage capabilities
```

## AXIOM-STOR-006 — Shell storage commands

Add:

```text
storage info
storage read <block>
```

Expected:

```text
storage info -> readonly=true, block_size, block_count
storage read 0 -> header
storage read invalid -> safe error
```

Commit:

```text
AXIOM-STOR-006: add shell storage commands
```

## AXIOM-STOR-007 — fs_service to storage_service path

Add path:

```text
/storage/version
```

Behavior:

```text
cat /storage/version
```

must go:

```text
shell_service -> fs_service -> storage_service -> fs_service -> shell_service
```

Kernel must not parse the path.

Kernel must not read the block.

Commit:

```text
AXIOM-STOR-007: connect filesystem service to storage service
```

## AXIOM-STOR-008 — QEMU storage test

Create:

```text
tests/storage_service_qemu_test.sh
```

Must assert:

* boot reaches `axiom>`,
* `storage info` returns `readonly=true`,
* `storage read 0` returns header,
* invalid block returns safe error,
* `cat /storage/version` works,
* shell alive after invalid request,
* apps still run after storage errors,
* shutdown exits QEMU 0.

Commit:

```text
AXIOM-STOR-008: add storage service QEMU test
```

## AXIOM-STOR-009 — Integrate into verify_all

`verify_all.sh` must now report:

```text
13/13 QEMU tests
VERIFY ALL: PASS
```

Commit:

```text
AXIOM-STOR-009: integrate storage test into verification sweep
```

## AXIOM-STOR-010 — Virtio-blk investigation

Create:

```text
docs/30_VIRTIO_BLOCK_INVESTIGATION.md
```

Must document:

1. QEMU virtio-blk model.
2. MMIO/PCI discovery path.
3. Required kernel device grant mechanism.
4. What remains in user space.
5. Minimal viable block-driver design.
6. Risks.
7. Why full driver is or is not implemented now.

Commit:

```text
AXIOM-STOR-010: document virtio-blk investigation
```

## AXIOM-STOR-011 — Archive v1.4 evidence

Create:

```text
evidence/v1.4/REPORT.md
evidence/v1.4/storage_service_qemu_test.log
evidence/v1.4/verify_all.log
evidence/v1.4/tool_versions.txt
```

Tag:

```sh
git tag -a v1.4-storage-service -m "AxiomRT v1.4 storage service"
```

Commit:

```text
AXIOM-STOR-011: archive v1.4 storage service evidence
```

Gate:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
./scripts/verify_all.sh
```

---

# 8. Phase v1.5 — AXIOM-DRV: User-Space Driver Framework

Goal:

Create a user-space driver model. Do not put complex drivers in the kernel.

## Required components

```text
driver_manager
device capability
MMIO grant mechanism
interrupt event delivery
driver crash containment
minimal block driver skeleton
```

## AXIOM-DRV-001 — Driver architecture document

Create:

```text
docs/31_USER_SPACE_DRIVER_FRAMEWORK.md
```

Must document:

1. Why drivers are user-space.
2. Device object model.
3. MMIO capability.
4. IRQ event capability.
5. Driver lifecycle.
6. Driver restart policy.
7. Block driver path.
8. Security risks.
9. Kernel boundary.

## AXIOM-DRV-002 — Device object and capability model

Add minimal kernel mechanism if required:

* device object,
* device capability,
* MMIO grant,
* IRQ event endpoint.

No device policy in kernel.

## AXIOM-DRV-003 — driver_manager service

Create U-mode `driver_manager`.

Responsibilities:

* start drivers,
* grant driver capabilities,
* restart failed drivers,
* report state to shell.

## AXIOM-DRV-004 — Minimal block_driver_service skeleton

Not full production driver yet.

Must expose:

```text
driver status
block device present/absent
read-only capability path
```

## AXIOM-DRV-005 — Shell driver commands

Add:

```text
drivers
driver info <name>
driver restart <name>
```

## AXIOM-DRV-006 — Driver crash test

Add:

```text
tests/driver_framework_qemu_test.sh
```

Must assert:

* driver starts,
* shell sees driver,
* driver fault is contained,
* supervisor notified,
* driver_manager can restart it,
* kernel survives.

## AXIOM-DRV-007 — Evidence and tag

Tag:

```text
v1.5-user-space-driver-framework
```

Gate:

```text
14/14 QEMU tests
VERIFY ALL: PASS
zero warnings
clippy clean
```

---

# 9. Phase v1.6 — AXIOM-LOAD: Storage-Backed FS and Restricted App Loader

Goal:

Move from static embedded app table to a real restricted loader path.

Do not implement arbitrary unsafe ELF loading first.

Use restricted app image format before full ELF.

## Required documents

```text
docs/32_RESTRICTED_APP_IMAGE_FORMAT.md
docs/33_STORAGE_BACKED_FS.md
```

## App image format

Define:

```text
magic
version
entry offset
text size
rodata size
stack pages
required capabilities
checksum
```

Rules:

* no relocation at first,
* bounded image size,
* W^X enforced,
* no kernel parsing of app names,
* app_loader validates manifest,
* kernel only maps already-validated image.

## Shell commands

Add:

```text
apps reload
app load <name>
app unload <name>
run <name>
```

## Filesystem

Add storage-backed read-only files:

```text
/bin/hello.app
/bin/counter.app
/bin/fault_demo.app
/etc/version
```

## Tests

Create:

```text
tests/restricted_loader_qemu_test.sh
```

Must assert:

* app image listed,
* app loaded,
* hello runs,
* counter runs,
* invalid image rejected,
* unauthorized app capability request denied,
* shell alive.

Tag:

```text
v1.6-storage-backed-loader
```

Gate:

```text
15/15 QEMU tests
VERIFY ALL: PASS
```

---

# 10. Phase v1.7 — AXIOM-NET: Minimal User-Space Network Service

Goal:

Add minimal networking only after storage and driver framework are stable.

Network must be user-space.

No network stack in kernel.

## Required documents

```text
docs/34_NETWORK_SERVICE.md
docs/35_VIRTIO_NET_INVESTIGATION.md
```

## First scope

Minimal packet service:

```text
net status
net send-test
net rx-count
net tx-count
```

No TCP/IP stack required at first.

## Required services

```text
net_driver_service
net_service
```

## Tests

Create:

```text
tests/network_service_qemu_test.sh
```

Must assert:

* net service starts,
* shell `net status` works,
* malformed network command safe,
* network service crash contained,
* kernel survives.

Tag:

```text
v1.7-minimal-network-service
```

Gate:

```text
16/16 QEMU tests
VERIFY ALL: PASS
```

---

# 11. Phase v1.8 — AXIOM-FUZZ: Robustness and Fuzzing

Goal:

Attack the system.

Do not move to hardware before robustness is improved.

## Required documents

```text
docs/36_ROBUSTNESS_CAMPAIGN.md
docs/37_FUZZING_PLAN.md
```

## Fuzz targets

1. syscall decoder,
2. IPC message parser,
3. capability indexes,
4. shell parser,
5. app loader request parser,
6. fs request parser,
7. storage request parser,
8. driver manager request parser,
9. network request parser if network exists,
10. event parser,
11. Studio parser,
12. axiomctl parser.

## Required behavior

Fuzzing may crash user services only if containment works.

Fuzzing must never crash the kernel.

## Required tools

Use simple deterministic fuzz harnesses first. Avoid unnecessary dependencies.

## Tests

Create:

```text
tests/robustness_qemu_test.sh
tools/fuzz/
```

## Evidence

Archive:

```text
evidence/v1.8/fuzz_summary.md
evidence/v1.8/fuzz_logs/
```

Tag:

```text
v1.8-robustness-fuzzing
```

Gate:

```text
No user-controlled malformed input discovered by fuzzing can crash the kernel.
VERIFY ALL: PASS
```

---

# 12. Phase v1.9 — AXIOM-REFINE: Formal Refinement

Goal:

Connect Coq model-level proofs to implementation-observable behavior.

Current boundary:

Coq proofs are model-level. Refinement to Rust is not fully discharged.

## Required documents

```text
docs/38_FORMAL_REFINEMENT_STRATEGY.md
```

## Required refinement tracks

1. Memory isolation model ↔ Rust/Sv39 page table implementation.
2. Capability model ↔ Rust capability table.
3. Scheduler model ↔ Rust scheduler.
4. IPC model ↔ Rust IPC implementation.
5. Fault containment model ↔ Rust fault policy.
6. Service/application lifecycle model ↔ implementation.

## Acceptable outcomes

For each track, either:

* prove refinement lemma, or
* document exact trusted assumption, or
* document why proof is deferred.

No theorem may imply more than is proven.

## Required Coq files

Potential additions:

```text
proofs/coq/IPCModel.v
proofs/coq/FaultContainment.v
proofs/coq/RefinementAssumptions.v
```

## Gate

```text
coqc all proof files
VERIFY ALL: PASS
formal claims updated to match proofs
```

Tag:

```text
v1.9-formal-refinement
```

---

# 13. Phase v2.0 — AXIOM-HW: Real Hardware BSP

Goal:

Boot AxiomRT on a real RISC-V board with MMU support.

Do not fake this with QEMU.

If no board exists, create blocker document and continue only with non-hardware phases.

## Required blocker behavior

If hardware is not available, create:

```text
docs/blockers/BLOCKER_REAL_HARDWARE_<date>.md
```

Must state:

* exact missing board,
* required specs,
* candidate boards,
* price/availability,
* boot method,
* expected risks,
* next action.

## Required board properties

* RISC-V 64,
* MMU/Sv39 capable,
* UART,
* timer,
* interrupt controller,
* documented boot chain,
* enough RAM.

## Required documents

```text
docs/39_HARDWARE_BSP_PLAN.md
docs/40_BOARD_SELECTION.md
docs/41_HARDWARE_MEMORY_MAP.md
```

## Required BSP tasks

1. choose board,
2. boot chain,
3. UART output,
4. timer,
5. interrupt setup,
6. Sv39 mapping,
7. U-mode entry,
8. page fault containment,
9. shell prompt,
10. app run,
11. fs read,
12. controlled shutdown or reset path.

## Hardware gate

Real hardware must show:

```text
AxiomRT kernel booted
MMU status=enabled
TASK_STARTED task=init_service
SERVICE started=shell_service
axiom>
```

and:

```text
axiom> help
axiom> tasks
axiom> run hello
axiom> ls
```

Tag:

```text
v2.0-real-hardware-bsp
```

---

# 14. Phase v2.1 — AXIOM-REL: Real OS Beta Release

Goal:

Create a complete release package for external users.

## Required artifacts

```text
release/AxiomRT_Real_OS_Source.tar.gz
release/AxiomRT_QEMU_Image.tar.gz
release/AxiomRT_Evidence.zip
release/AxiomRT_User_Guide.pdf
release/AxiomRT_Developer_Guide.pdf
release/axiomctl
release/AxiomRT_Studio
release/install.sh
release/checksums.txt
release/release_notes.md
```

## Required documents

```text
docs/42_USER_GUIDE.md
docs/43_DEVELOPER_GUIDE.md
docs/44_RELEASE_PROCESS.md
docs/45_TROUBLESHOOTING.md
docs/46_LIMITATIONS_REAL_OS_BETA.md
```

## Required release command

```sh
./scripts/build_real_os_release.sh
```

## Gate

A new user can:

```sh
./install.sh
axiomctl doctor
axiomctl run os
axiomctl verify
```

and see:

```text
axiom>
```

Tag:

```text
v2.1-real-os-beta
```

---

# 15. Phase v2.2 — AXIOM-REVIEW: External Review Readiness

Goal:

Make the project reviewable by someone other than the author.

## Required documents

```text
docs/47_EXTERNAL_REVIEW_GUIDE.md
docs/48_REVIEW_CHECKLIST.md
docs/49_KNOWN_LIMITATIONS.md
```

## Required review checklist

External reviewer must be able to check:

1. clone repo,
2. install dependencies,
3. run QEMU,
4. run shell,
5. run apps,
6. use fs,
7. run storage,
8. run tests,
9. inspect evidence,
10. inspect limitations,
11. inspect proof boundaries.

## Required issue templates

Create:

```text
.github/ISSUE_TEMPLATE/bug_report.md
.github/ISSUE_TEMPLATE/safety_issue.md
.github/ISSUE_TEMPLATE/security_issue.md
.github/ISSUE_TEMPLATE/review_finding.md
```

## Gate

At least one external engineer can reproduce the system or the project has a documented blocker explaining why not.

Tag:

```text
v2.2-external-review-ready
```

---

# 16. Phase v3.0 — Real OS Complete Edition Gate

AxiomRT may be called:

```text
AxiomRT Real OS Complete Edition
```

only when all of the following are true:

## Kernel and isolation

* boots in QEMU,
* boots on real RISC-V hardware or hardware blocker is explicitly unresolved and project is not called hardware-complete,
* Sv39 MMU enabled,
* U-mode tasks isolated,
* page faults contained,
* illegal instructions contained,
* capability-less IPC denied,
* kernel survives faulty tasks.

## OS services

* init_service works,
* supervisor_service works,
* logger_service works,
* console_service works,
* shell_service works,
* app_loader_service works,
* fs_service works,
* storage_service works,
* driver_manager exists,
* at least one driver service exists,
* optional network service exists or is documented as not in scope.

## User experience

* `axiom>` shell works,
* `help` works,
* `tasks` works,
* `apps` works,
* `run hello` works,
* `run counter` works,
* `run fault_demo` works and is contained,
* `ls` works,
* `cat` works,
* `storage info` works,
* system remains alive after user errors.

## Developer experience

* `axiomctl doctor` works,
* `axiomctl run` works,
* `axiomctl verify` works,
* AxiomRT Studio works,
* installer works,
* release package builds,
* docs are sufficient.

## Verification

* all QEMU tests pass,
* host tests pass,
* clippy clean,
* zero warnings,
* Coq files compile,
* fuzzing campaign exists,
* evidence archived,
* limitations honest.

## Release

* release archive exists,
* checksums exist,
* user guide exists,
* developer guide exists,
* release notes exist,
* external review guide exists.

## Honesty

AxiomRT must not claim:

* certified,
* production-ready,
* vehicle-ready,
* aircraft-ready,
* defect-free,
* formally fully verified,

unless that evidence actually exists.

---

# 17. Next Immediate Action

Start now with:

```text
AXIOM-STOR-001
```

Do not start drivers before storage service.

Do not start hardware before storage + drivers + robustness are stable.

Do not start certification language at all.

Current next command sequence:

```sh
git status
./scripts/verify_all.sh
```

Then create:

```text
docs/29_STORAGE_SERVICE.md
```

Commit:

```text
AXIOM-STOR-001: document storage service architecture
```

Proceed one task at a time.

Do not stop until the final gate is reached or a real blocker document is created.
