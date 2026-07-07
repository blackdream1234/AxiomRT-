# AxiomRT v1.5 Prompt — AXIOM-DRV User-Space Driver Framework

You are working on AxiomRT.

Repository:

```text
https://github.com/blackdream1234/AxiomRT-
```

Current verified local state:

AxiomRT has reached `v1.4-storage-service`.

The system currently has:

* QEMU RISC-V 64 / OpenSBI boot,
* Sv39 MMU enabled,
* isolated U-mode services,
* interactive `axiom>` shell,
* app loader service,
* static user applications,
* read-only filesystem service,
* user-space storage service,
* bounded IPC,
* capability-based access control,
* watchdog containment,
* supervisor/logger chain,
* `axiomctl`,
* AxiomRT Studio,
* install script,
* evidence archives,
* 13/13 QEMU tests,
* zero Rust warnings,
* clippy `-D warnings` clean,
* 3 Coq model files compiling.

v1.4 storage result:

* `storage_service` is U-mode, slot 11, endpoint 5.
* It serves an 8-block read-only image.
* Protocol: `INFO`, `READ block=<n>`, `READ_RANGE`.
* `cat /storage/version` travels shell → fs_service → storage_service → fs_service → shell.
* Kernel does not parse paths and does not read blocks.
* Kernel contribution was endpoint id + `storage_info` / `storage_read` rights.
* `CAPS_PER_TASK` is now 8.
* Shell must preserve all seven existing capabilities:

  * line,
  * console,
  * info,
  * control,
  * app,
  * fs,
  * storage.
* Apps must not receive storage capability.
* `fault_demo` must still have no capabilities.

Important current limitation:

`docs/30_VIRTIO_BLOCK_INVESTIGATION.md` states that a real virtio-blk driver requires three new kernel mechanisms:

1. MMIO grant,
2. DMA-visible buffer grant,
3. IRQ → endpoint delivery.

These mechanisms are the scope of v1.5.

Do not implement full production virtio-blk yet.

Do not implement writable filesystem yet.

Do not implement networking yet.

Do not implement real hardware BSP yet.

---

# 1. Phase Goal

Implement the first user-space driver framework.

The goal is not a full driver ecosystem. The goal is to introduce the minimal safe mechanisms needed to support isolated user-space drivers.

Final v1.5 target:

```text
AxiomRT boots to axiom>
driver_manager starts in U-mode
block_driver_service skeleton starts in U-mode
device capabilities are explicit
MMIO access is granted only through capabilities
DMA-visible buffer policy is documented and minimally modeled
IRQ events can be delivered to a driver endpoint
driver crash is contained
driver_manager can report/restart driver
kernel remains alive
verify_all.sh passes with 14/14 QEMU tests
```

---

# 2. Architecture Law

AxiomRT remains a microkernel.

The kernel may add only minimal mechanisms:

* Device object identity.
* Device capability.
* MMIO region grant.
* DMA buffer grant model.
* IRQ event endpoint delivery.
* Syscall validation for device operations.
* Fault containment for driver faults.

The kernel must not contain:

* driver policy,
* block protocol policy,
* filesystem policy,
* path parsing,
* storage cache policy,
* virtio queue high-level policy,
* network stack,
* shell logic,
* app logic,
* certification claims.

Driver policy lives in user space:

* `driver_manager`,
* `block_driver_service`,
* future device-specific services.

---

# 3. Mandatory First Step

Before editing code, run:

```bash
git status
./scripts/verify_all.sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

If any command fails, stop and fix the regression before AXIOM-DRV.

If the repo is not pushed, push before continuing:

```bash
git push origin main --tags
```

Then check GitHub Actions if available. If Actions are not running or not configured, document the CI limitation in the v1.5 evidence report.

---

# 4. Existing Behavior Must Not Regress

All existing shell commands must remain:

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
shutdown
```

All existing QEMU tests must still pass.

No existing capability may be silently dropped.

No existing service may lose authority required for its current behavior.

---

# 5. Task Sequence

Run these tasks in order.

One task = one commit, except when a disclosed grouping is necessary for a QEMU-verifiable state. If grouping is used, disclose it in the commit body.

---

## AXIOM-DRV-001 — Document User-Space Driver Framework

Create:

```text
docs/31_USER_SPACE_DRIVER_FRAMEWORK.md
```

Must document:

1. Why drivers are user-space.
2. What a driver is in AxiomRT.
3. Driver lifecycle:

   * Available,
   * Starting,
   * Running,
   * Blocked,
   * Faulted,
   * Killed,
   * Restarted.
4. `driver_manager` responsibilities.
5. `block_driver_service` skeleton responsibilities.
6. Device object model.
7. MMIO grant model.
8. DMA-visible buffer grant model.
9. IRQ event delivery model.
10. Capability model.
11. Kernel boundary.
12. Security risks.
13. Safety risks.
14. Limitations.
15. Future virtio-blk path.
16. Future real hardware path.

Required honesty:

State clearly:

* v1.5 is not full production driver support.
* v1.5 is not full virtio-blk implementation unless explicitly proven.
* v1.5 does not provide writable persistent storage.
* v1.5 does not claim crash consistency.
* v1.5 does not claim certification.

Commit:

```text
AXIOM-DRV-001: document user-space driver framework
```

---

## AXIOM-DRV-002 — Add Device Object and Capability Model

Goal:

Add a minimal kernel-side device object model.

Required concepts:

```text
DeviceId
DeviceKind
DeviceObject
MmioRegion
IrqLine
DmaRegion
DeviceCapability
```

Initial supported device kind:

```text
BlockDeviceSkeleton
```

Required rights:

```text
device_info
mmio_read
mmio_write
dma_read
dma_write
irq_receive
driver_control
```

Rules:

* Rights must be deny-by-default.
* No driver receives all rights by accident.
* Capability lookup must happen before any device operation.
* Invalid device id must fail safely.
* Insufficient rights must fail safely.
* Revoked/absent capability must fail safely.
* No existing cap behavior may regress.
* Shell/service capability capacity must be explicitly checked.

Add host tests for:

1. device capability creation,
2. insufficient rights rejection,
3. wrong device id rejection,
4. no authority means no access,
5. rights do not amplify by derivation.

Commit:

```text
AXIOM-DRV-002: add device object and capability model
```

---

## AXIOM-DRV-003 — Add MMIO Grant Mechanism

Goal:

Add minimal MMIO grant mechanics without implementing complex device policy.

Required behavior:

* Kernel knows a static MMIO region description.
* A driver can receive a capability to a region.
* Driver cannot access MMIO without capability.
* Driver cannot request arbitrary physical addresses.
* Region bounds are checked.
* Misaligned or out-of-range MMIO access is rejected.
* Kernel does not interpret device protocol.
* Kernel does not implement virtio queue policy.

Recommended syscall shape, adapt to existing syscall architecture:

```text
sys_device_info(device_id)
sys_mmio_read(device_cap, offset, width)
sys_mmio_write(device_cap, offset, width, value)
```

But only implement what is necessary for the v1.5 gate.

Add host tests.

Add QEMU-visible evidence line examples:

```text
DEVICE registered=block0 kind=block_skeleton
MMIO grant task=block_driver_service device=block0 region=virtio_mmio0
MMIO_DENIED task=faulty_task reason=no_valid_capability
```

Commit:

```text
AXIOM-DRV-003: add capability-gated MMIO grant mechanism
```

---

## AXIOM-DRV-004 — Add DMA-Visible Buffer Grant Model

Goal:

Add a minimal DMA buffer model needed for future virtio-blk.

Do not implement full DMA driver yet.

Required behavior:

* Define what a DMA-visible buffer is.
* Define who can allocate/grant it.
* Define access rights.
* Enforce bounds.
* Prevent user tasks from using arbitrary kernel memory as DMA.
* Prevent drivers from DMA-writing outside granted buffers.
* Document cache/coherency limitations honestly.

Possible v1.5 implementation:

* model-level buffer object,
* static bounce buffer,
* capability-gated access,
* host tests,
* QEMU evidence lines,
* no real device DMA yet unless safe and necessary.

Required evidence lines:

```text
DMA grant task=block_driver_service buffer=block0_dma size=<n>
DMA_DENIED task=faulty_task reason=no_valid_capability
```

Commit:

```text
AXIOM-DRV-004: add DMA-visible buffer grant model
```

---

## AXIOM-DRV-005 — Add IRQ Event Delivery Mechanism

Goal:

Deliver device/IRQ events to a user-space driver endpoint.

Required behavior:

* IRQ source maps to endpoint.
* Only authorized driver receives IRQ event.
* Event is bounded.
* Event delivery cannot overwrite unrelated endpoint state.
* If driver is dead/faulted, event is logged/dropped safely according to documented policy.
* Kernel does not run driver policy.

For QEMU stage, it is acceptable to use a synthetic IRQ event if real virtio IRQ is not implemented yet.

Required evidence lines:

```text
IRQ registered source=block0 endpoint=driver_irq
IRQ delivered to=block_driver_service source=block0
IRQ_DROPPED reason=driver_not_ready
```

Add tests.

Commit:

```text
AXIOM-DRV-005: add IRQ event delivery mechanism
```

---

## AXIOM-DRV-006 — Add driver_manager Service

Goal:

Create U-mode `driver_manager`.

Responsibilities:

* start driver services,
* track driver state,
* expose driver status to shell,
* receive driver fault notifications,
* request driver restart when allowed,
* never parse device protocol.

Required shell-visible state:

```text
driver name=block_driver_service state=running kind=block_skeleton
```

Required commands later:

```text
drivers
driver info block
driver restart block
```

Commit:

```text
AXIOM-DRV-006: add driver manager service
```

---

## AXIOM-DRV-007 — Add block_driver_service Skeleton

Goal:

Create U-mode `block_driver_service`.

Scope:

This is a skeleton/prototype, not full production virtio-blk.

Required behavior:

* starts in its own address space,
* receives device capability,
* receives MMIO capability if implemented,
* receives DMA buffer capability if implemented,
* receives IRQ endpoint capability if implemented,
* responds to `driver status`,
* can serve a synthetic block device status,
* can fault safely in test mode,
* can be restarted by `driver_manager`.

Do not replace the existing `storage_service` yet.

Do not implement writable storage.

Commit:

```text
AXIOM-DRV-007: add block driver service skeleton
```

---

## AXIOM-DRV-008 — Add Shell Driver Commands

Add shell commands:

```text
drivers
driver info block
driver restart block
driver fault block
```

Expected behavior:

```text
drivers
-> block_driver_service running

driver info block
-> kind=block_skeleton state=running mmio=granted irq=registered

driver restart block
-> restart requested / restarted

driver fault block
-> deliberately faults driver for containment test
```

Preserve all existing commands.

Commit:

```text
AXIOM-DRV-008: add shell driver commands
```

---

## AXIOM-DRV-009 — Add Driver Crash Containment Test

Create:

```text
tests/driver_framework_qemu_test.sh
```

The test must assert:

1. boot reaches `axiom>`,
2. `drivers` lists `block_driver_service`,
3. `driver info block` works,
4. `driver fault block` triggers contained user fault,
5. supervisor receives fault,
6. driver_manager observes failure,
7. `driver restart block` restarts driver,
8. `drivers` shows driver running again,
9. shell remains alive,
10. existing `run hello` still works,
11. existing `cat /storage/version` still works,
12. `shutdown` exits QEMU 0.

Expected evidence lines:

```text
DRIVER started=block_driver_service
DEVICE registered=block0
MMIO grant task=block_driver_service
IRQ registered source=block0
FAULT type=<...> task=block_driver_service
CONTAIN scope=user reason=<...> kernel=alive
DRIVER restarted=block_driver_service
```

Commit:

```text
AXIOM-DRV-009: add driver framework QEMU test
```

---

## AXIOM-DRV-010 — Integrate Driver Test Into verify_all

Update:

```text
scripts/verify_all.sh
```

After this phase, it must report:

```text
14/14 QEMU tests
VERIFY ALL: PASS
```

Must also preserve:

* host tests,
* axiomctl tests,
* studio tests,
* Coq model compilation,
* zero warnings,
* clippy clean.

Commit:

```text
AXIOM-DRV-010: integrate driver test into verification sweep
```

---

## AXIOM-DRV-011 — Update axiomctl and Studio

Update host tools minimally.

`axiomctl` should expose or summarize driver evidence if applicable:

```text
axiomctl demo drivers
axiomctl events summary <driver log>
```

AxiomRT Studio should show:

* driver events,
* device events,
* MMIO grants,
* IRQ events,
* driver faults/restarts.

Do not add dependencies unless justified.

Commit:

```text
AXIOM-DRV-011: update host tools for driver events
```

---

## AXIOM-DRV-012 — Archive v1.5 Evidence

Create:

```text
evidence/v1.5/REPORT.md
evidence/v1.5/driver_framework_qemu_test.log
evidence/v1.5/verify_all.log
evidence/v1.5/tool_versions.txt
```

Report must state:

1. What v1.5 demonstrates.
2. What remains prototype-only.
3. What is synthetic vs real hardware/device behavior.
4. Whether MMIO is real or modeled.
5. Whether IRQ is real or synthetic.
6. Whether DMA is real or modeled.
7. No certification claim.
8. No production driver claim.
9. Next phase.

Update README current milestone.

Tag:

```bash
git tag -a v1.5-user-space-driver-framework -m "AxiomRT v1.5 user-space driver framework"
```

Commit:

```text
AXIOM-DRV-012: archive v1.5 driver framework evidence
```

---

# 6. Required Final Verification

Before tagging v1.5, run:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
./scripts/verify_all.sh
```

Then run the driver test alone:

```bash
./tests/driver_framework_qemu_test.sh
```

Expected final state:

```text
VERIFY ALL: PASS
14/14 QEMU tests
zero warnings
clippy clean
```

---

# 7. Forbidden Shortcuts

Do not do any of this:

* Do not implement full virtio-blk if the kernel mechanisms are not ready.
* Do not put virtio queue policy in the kernel.
* Do not allow arbitrary MMIO addresses from user input.
* Do not let shell directly access MMIO.
* Do not grant apps device capabilities.
* Do not give `fault_demo` any device rights.
* Do not silently increase capability slots without tests.
* Do not remove existing services.
* Do not remove existing shell commands.
* Do not change storage semantics.
* Do not claim real hardware support.
* Do not claim production driver support.
* Do not claim certification.
* Do not hide synthetic IRQ/DMA limitations.

---

# 8. Definition of Done

v1.5 is complete only when:

* `driver_manager` exists in U-mode,
* `block_driver_service` exists in U-mode,
* device capability model exists,
* MMIO grant model exists or is honestly bounded if modeled,
* DMA grant model exists or is honestly bounded if modeled,
* IRQ event delivery exists or is honestly synthetic,
* driver crash is contained,
* driver restart works,
* shell remains alive,
* existing apps still work,
* filesystem still works,
* storage service still works,
* `tests/driver_framework_qemu_test.sh` passes,
* `./scripts/verify_all.sh` ends with `VERIFY ALL: PASS`,
* QEMU test count is 14/14,
* zero warnings,
* clippy clean,
* evidence is archived,
* README is updated,
* tag `v1.5-user-space-driver-framework` exists.

---

# 9. Next Phase After v1.5

After v1.5, do not jump directly to hardware unless the driver framework is stable.

Next phase should be:

```text
v1.6-storage-backed-fs-and-loader
```

Goal:

* connect storage and fs more deeply,
* prepare restricted app image loading,
* still no arbitrary unsafe ELF loading,
* still no production persistence claim.

But do not start v1.6 until v1.5 gate passes.
