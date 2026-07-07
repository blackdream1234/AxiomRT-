# AxiomRT v1.6 Prompt — AXIOM-LOAD Storage-Backed FS and Restricted App Loader

You are working on AxiomRT.

Repository:

```text
https://github.com/blackdream1234/AxiomRT-
```

Current local verified state:

AxiomRT has reached `v1.5-user-space-driver-framework`.

The current system has:

* QEMU RISC-V 64 / OpenSBI boot,
* Sv39 MMU enabled,
* isolated U-mode services,
* interactive `axiom>` shell,
* app_loader_service,
* static user applications,
* read-only fs_service,
* storage_service,
* driver_manager,
* block_driver_service skeleton,
* device object/capability model,
* capability-gated MMIO mechanism,
* modeled DMA bounce-page grant,
* synthetic IRQ delivery,
* watchdog containment,
* supervisor/logger recovery,
* `axiomctl`,
* AxiomRT Studio,
* installer,
* evidence archives,
* `./scripts/verify_all.sh -> VERIFY ALL: PASS`,
* 14/14 QEMU tests,
* zero warnings,
* clippy `-D warnings` clean in all relevant configurations,
* 3 Coq model files compile.

Known v1.5 limitations:

* v1.5 does not implement full virtio-blk.
* MMIO magic read works on real virtio-mmio window, but no full device is driven.
* DMA is modeled; no real device DMA occurs.
* IRQ delivery is synthetic; PLIC is not used.
* No writable filesystem.
* No certification claim.
* No production driver claim.
* Shell capability table is full at 8/8; the next endpoint consumer requires an explicit `CAPS_PER_TASK` review/bump.
* `/etc/version` and `/storage/version` may still report v1.4; v1.6 must correct user-facing version metadata without weakening storage semantics.

Before starting v1.6:

```bash
git status
git push origin main
git push origin --tags
```

If push fails because remote is ahead:

```bash
git pull --rebase origin main
./scripts/verify_all.sh
git push origin main
git push origin --tags
```

If GitHub Actions exist, wait for them or document their status in evidence.

---

# 1. Phase Goal

Implement `v1.6-storage-backed-fs-and-loader`.

The goal is to move from static embedded applications toward a real restricted application loading path, while staying safe.

This phase must introduce:

1. a restricted app image format,
2. app image metadata in the filesystem/storage model,
3. app_loader support for loading by manifest/image metadata,
4. shell commands for loading/listing storage-backed apps,
5. QEMU tests proving safe loading, running, rejection, and containment.

This phase must not implement arbitrary ELF loading yet.

This phase must not implement writable storage yet.

This phase must not implement production persistence.

This phase must not claim real hardware support.

---

# 2. Architecture Law

AxiomRT remains a microkernel.

The kernel may provide mechanisms only:

* validate syscall arguments,
* create address spaces,
* map already-validated code/rodata regions,
* assign capabilities from a validated manifest,
* schedule tasks,
* enforce IPC/capabilities,
* contain faults.

The kernel must not contain:

* app-name policy,
* filesystem path parsing,
* storage path parsing,
* app selection policy,
* shell logic,
* filesystem logic,
* storage cache policy,
* high-level executable format policy,
* certification claims.

Application policy lives in user space:

* `app_loader_service`,
* `fs_service`,
* `storage_service`,
* shell.

---

# 3. Existing Behavior Must Not Regress

All existing commands must continue to work:

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
apps
app info hello
run hello
run counter
run fault_demo
run demo
ls
ls /etc
ls /apps
cat /etc/version
cat /apps/hello.manifest
cat /storage/version
storage info
storage read 0
drivers
driver info block
driver fault block
driver restart block
shutdown
```

All existing tests must pass.

No existing service may lose required capability.

No old capability may silently disappear.

No existing QEMU test may be weakened.

---

# 4. v1.6 Safety Boundary

Allowed in v1.6:

* restricted app image format,
* static storage-backed image table,
* read-only app image loading,
* bounded manifest parsing,
* explicit capability grants,
* rejection of malformed images,
* rejection of unauthorized capability requests,
* shell commands for loading/running apps,
* QEMU tests,
* documentation,
* evidence.

Forbidden in v1.6:

* full ELF loader,
* dynamic relocation,
* writable filesystem,
* arbitrary app upload,
* package manager,
* network download,
* production persistence claim,
* certification claim,
* kernel filesystem logic,
* kernel app-name parsing,
* unsafe broad device grants,
* giving apps driver/MMIO/DMA/IRQ capabilities unless a specific test proves denial or tightly bounded grant.

---

# 5. Task Sequence

Run tasks in order.

One task = one commit, unless a disclosed grouping is necessary to reach a QEMU-verifiable state.

---

## AXIOM-LOAD-001 — Document Restricted App Image Format

Create:

```text
docs/32_RESTRICTED_APP_IMAGE_FORMAT.md
```

Must document:

1. Why full ELF is out of scope for v1.6.
2. Restricted app image goals.
3. Image fields.
4. Manifest fields.
5. Capability request model.
6. Loader validation rules.
7. Kernel boundary.
8. Filesystem/storage relationship.
9. Security limitations.
10. Future ELF path.

Required image fields:

```text
magic
version
name
entry_offset
text_size
rodata_size
stack_pages
required_capabilities
image_size
checksum
```

Rules:

* bounded image size,
* no relocation at this phase,
* W^X enforced,
* code and rodata separated,
* capabilities must be explicit,
* invalid checksum rejected,
* unknown capability rejected,
* excessive capability request rejected,
* malformed image rejected safely.

Commit:

```text
AXIOM-LOAD-001: document restricted app image format
```

---

## AXIOM-LOAD-002 — Document Storage-Backed FS Transition

Create:

```text
docs/33_STORAGE_BACKED_FS.md
```

Must document:

1. Current v1.3 embedded read-only fs.
2. Current v1.4 storage service.
3. v1.6 storage-backed app image path.
4. Why filesystem remains user-space.
5. Why storage remains user-space.
6. How fs_service asks storage_service for image data.
7. How app_loader asks fs_service for app metadata.
8. Limitations.
9. Future writable filesystem.
10. Future block-backed persistence.

Commit:

```text
AXIOM-LOAD-002: document storage-backed filesystem transition
```

---

## AXIOM-LOAD-003 — Update Version Metadata Safely

Goal:

Fix stale user-facing version metadata without weakening storage semantics.

Update:

```text
/etc/version
/storage/version
/docs/about
README current milestone
evidence plan references
```

Expected content should identify:

```text
AxiomRT v1.6-storage-backed-loader
```

But do not claim v1.6 complete until the final evidence task. During implementation, use:

```text
AxiomRT v1.6-dev
```

Final evidence task may update to:

```text
AxiomRT v1.6-storage-backed-loader
```

Required tests:

* `cat /etc/version`
* `cat /storage/version`

Commit:

```text
AXIOM-LOAD-003: update version metadata for loader phase
```

---

## AXIOM-LOAD-004 — Define App Image Metadata in Read-Only FS

Add read-only app image entries.

Initial storage-backed app paths:

```text
/bin/hello.app
/bin/counter.app
/bin/fault_demo.app
/bin/invalid_bad_magic.app
/bin/invalid_bad_cap.app
/bin/invalid_bad_checksum.app
```

Manifest paths:

```text
/apps/hello.manifest
/apps/counter.manifest
/apps/fault_demo.manifest
```

`ls /bin` must list:

```text
hello.app counter.app fault_demo.app invalid_bad_magic.app invalid_bad_cap.app invalid_bad_checksum.app
```

Rules:

* These may be static images for v1.6.
* The loader must treat them as storage/fs-provided image records.
* Do not load arbitrary external files yet.
* Invalid images must exist for tests.

Commit:

```text
AXIOM-LOAD-004: add restricted app image records to filesystem
```

---

## AXIOM-LOAD-005 — Add Loader Request Protocol

Define bounded IPC protocol between shell, app_loader, fs_service, and storage_service.

Shell to app_loader examples:

```text
APP_LIST
APP_INFO hello
APP_LOAD hello
APP_RUN hello
APP_UNLOAD hello
```

app_loader to fs_service examples:

```text
CAT /bin/hello.app
CAT /apps/hello.manifest
```

Expected responses:

```text
OK loaded hello
OK running hello
OK unloaded hello
ERR not_found
ERR bad_image
ERR bad_checksum
ERR denied_capability
ERR already_loaded
ERR not_loaded
ERR malformed
```

Rules:

* bounded messages only,
* no dynamic allocation unless already used safely,
* malformed requests fail safely,
* app_loader owns app policy,
* kernel never parses app name.

Commit:

```text
AXIOM-LOAD-005: define loader IPC protocol
```

---

## AXIOM-LOAD-006 — Implement App Image Validator

Add validator in app_loader/user-space policy.

Validator must check:

1. magic,
2. version,
3. image size bounds,
4. text/rodata bounds,
5. entry offset in text,
6. W^X layout,
7. stack pages within policy,
8. capability requests,
9. checksum,
10. known app name.

Invalid image tests:

* bad magic -> `ERR bad_image`
* bad checksum -> `ERR bad_checksum`
* bad capability -> `ERR denied_capability`

Commit:

```text
AXIOM-LOAD-006: add restricted app image validator
```

---

## AXIOM-LOAD-007 — Add Loaded-App State Machine

Add user-space app_loader state machine:

```text
Available
Loaded
Running
Exited
Faulted
Killed
Unloaded
```

Rules:

* `APP_LOAD` moves Available -> Loaded.
* `APP_RUN` moves Loaded -> Running.
* Exited app returns to Loaded or Exited according to documented policy.
* Faulted app is visible.
* `APP_UNLOAD` clears loaded state.
* repeated load/run must be deterministic.

Commit:

```text
AXIOM-LOAD-007: add loaded app state machine
```

---

## AXIOM-LOAD-008 — Add Shell Loader Commands

Add shell commands:

```text
bin
app load <name>
app unload <name>
app state <name>
run loaded <name>
```

Preserve existing:

```text
apps
app info <name>
run <name>
```

Expected behavior:

```text
bin
-> hello.app counter.app fault_demo.app ...

app load hello
-> OK loaded hello

app state hello
-> state=loaded

run loaded hello
-> hello runs and exits cleanly

app unload hello
-> OK unloaded hello
```

Commit:

```text
AXIOM-LOAD-008: add shell commands for storage-backed apps
```

---

## AXIOM-LOAD-009 — Kernel Mapping Mechanism Review

Goal:

Review whether current kernel static service/app mapping is sufficient for restricted images.

If the existing mechanism can represent restricted images safely, document it.

If a small kernel mechanism is needed, it must be limited to:

* mapping validated text as U+R+X,
* mapping validated rodata as U+R,
* mapping stack as U+R+W,
* denying W+X,
* denying kernel-address mappings,
* denying oversized images.

Kernel must not parse names or manifests.

Add host tests for:

* W^X rejection,
* kernel address rejection,
* oversized image rejection,
* bad entry rejection.

Commit:

```text
AXIOM-LOAD-009: review app image mapping mechanism
```

---

## AXIOM-LOAD-010 — Implement Load/Run for hello

Goal:

`hello` can be loaded from the restricted image path and run.

Required shell sequence:

```text
bin
app load hello
app state hello
run loaded hello
```

Expected output:

```text
OK loaded hello
state=loaded
hello from app: hello
TASK_EXITED task=hello
```

Commit:

```text
AXIOM-LOAD-010: load and run hello from restricted image
```

---

## AXIOM-LOAD-011 — Implement Load/Run for counter

Required shell sequence:

```text
app load counter
run loaded counter
```

Expected output:

```text
counter progress=1
counter progress=2
counter progress=3
TASK_EXITED task=counter
```

Commit:

```text
AXIOM-LOAD-011: load and run counter from restricted image
```

---

## AXIOM-LOAD-012 — Implement Fault Containment for Loaded App

Required shell sequence:

```text
app load fault_demo
run loaded fault_demo
uptime
```

Expected:

```text
CAP_DENIED task=fault_demo
FAULT type=WatchdogTimeout task=fault_demo
CONTAIN scope=user ...
RECOVERY_APPLIED policy=Kill
uptime ticks=...
```

The shell must remain alive.

Commit:

```text
AXIOM-LOAD-012: contain faulting restricted app
```

---

## AXIOM-LOAD-013 — Add Invalid Image Tests

Required shell sequences:

```text
app load invalid_bad_magic
app load invalid_bad_checksum
app load invalid_bad_cap
```

Expected:

```text
ERR bad_image
ERR bad_checksum
ERR denied_capability
```

No kernel fault.

No shell crash.

Commit:

```text
AXIOM-LOAD-013: reject invalid restricted app images
```

---

## AXIOM-LOAD-014 — Add QEMU Test

Create:

```text
tests/restricted_loader_qemu_test.sh
```

Must assert:

1. boot reaches `axiom>`,
2. `bin` lists app images,
3. `app load hello` works,
4. `run loaded hello` works,
5. `app load counter` works,
6. `run loaded counter` works,
7. `app load fault_demo` works,
8. `run loaded fault_demo` is contained,
9. shell alive after fault,
10. invalid bad magic rejected,
11. invalid bad checksum rejected,
12. invalid bad capability rejected,
13. existing `run hello` still works,
14. existing `ls` still works,
15. existing `storage info` still works,
16. existing `drivers` still works,
17. shutdown exits QEMU 0.

Commit:

```text
AXIOM-LOAD-014: add restricted loader QEMU test
```

---

## AXIOM-LOAD-015 — Integrate Into verify_all

Update:

```text
scripts/verify_all.sh
```

Expected:

```text
15/15 QEMU tests
VERIFY ALL: PASS
```

Commit:

```text
AXIOM-LOAD-015: integrate restricted loader test into verification sweep
```

---

## AXIOM-LOAD-016 — Update axiomctl and Studio

Update host tools minimally.

axiomctl should be able to summarize loader-related events.

Studio should display:

* app image list,
* load events,
* invalid image rejection events,
* loaded app state,
* faulting loaded app containment.

No new dependency unless justified.

Commit:

```text
AXIOM-LOAD-016: update host tools for restricted loader events
```

---

## AXIOM-LOAD-017 — Archive v1.6 Evidence

Create:

```text
evidence/v1.6/REPORT.md
evidence/v1.6/restricted_loader_qemu_test.log
evidence/v1.6/verify_all.log
evidence/v1.6/tool_versions.txt
```

Report must state:

1. what v1.6 demonstrates,
2. what remains static,
3. what is not full ELF,
4. what is not writable storage,
5. what is not production persistence,
6. no certification claim,
7. next phase.

Update README current milestone.

Final tag:

```bash
git tag -a v1.6-storage-backed-loader -m "AxiomRT v1.6 storage-backed restricted loader"
```

Commit:

```text
AXIOM-LOAD-017: archive v1.6 restricted loader evidence
```

---

# Required Final Commands

Before final tag:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
./scripts/verify_all.sh
./tests/restricted_loader_qemu_test.sh
```

Expected:

```text
VERIFY ALL: PASS
15/15 QEMU tests
zero warnings
clippy clean
```

---

# Definition of Done

v1.6 is complete only when:

* restricted app image format is documented,
* storage-backed FS transition is documented,
* stale version metadata is corrected,
* `/bin` exists in read-only filesystem view,
* app images exist as restricted records,
* app_loader validates image metadata,
* bad magic is rejected,
* bad checksum is rejected,
* unauthorized capability request is rejected,
* `app load hello` works,
* `run loaded hello` works,
* `app load counter` works,
* `run loaded counter` works,
* `app load fault_demo` works,
* `run loaded fault_demo` is contained,
* shell remains alive,
* existing static `run hello` still works,
* existing fs/storage/driver commands still work,
* kernel does not parse app names,
* kernel does not parse paths,
* kernel does not implement filesystem policy,
* kernel does not implement arbitrary ELF loading,
* tests pass,
* evidence archived,
* README updated,
* tag exists.

---

# Forbidden Shortcuts

Do not:

* implement arbitrary ELF loading,
* add writable filesystem,
* add network,
* start real hardware,
* give apps MMIO/DMA/IRQ rights,
* allow unrestricted capability requests,
* parse paths in kernel,
* parse app names in kernel,
* hide stale version metadata,
* remove existing commands,
* weaken existing tests,
* claim production readiness,
* claim certification.

---

# Next Phase After v1.6

After v1.6, next likely phase:

```text
v1.7-minimal-network-service
```

But do not start it until v1.6 gate passes.
