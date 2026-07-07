# AxiomRT v1.5 Evidence Report — User-Space Driver Framework

Phase: AXIOM-DRV (spec: `AxiomRT v1.5.md`; design: docs/31).
Tag: `v1.5-user-space-driver-framework`.
Archived: 2026-07-08. Tool versions: `tool_versions.txt`.

## 1. What v1.5 demonstrates

The first user-space driver framework on the AxiomRT microkernel,
QEMU-verified end to end (`driver_framework_qemu_test.log`, 30
assertions; `verify_all.log`, 14/14 QEMU tests + host suites + Coq):

* A kernel **device object model** (`block0`, kind `block_skeleton`)
  with a new device capability type and seven deny-by-default rights
  (`device_info`, `mmio_read`, `mmio_write`, `dma_read`, `dma_write`,
  `irq_receive`, `driver_control`), host-tested for creation,
  insufficient-rights rejection, wrong-id rejection,
  no-authority-no-access, and no amplification on derivation.
* **Capability-gated, syscall-mediated MMIO**: fixed check order
  (capability → bounds/alignment/width), the kernel performs the one
  volatile access itself, drivers never see physical addresses, every
  denial is logged (`MMIO_DENIED`).
* A **DMA-visible buffer grant model**: one kernel-owned bounce page
  per device, capability-gated read/write syscalls, bounds enforced —
  user tasks cannot nominate arbitrary memory as DMA.
* **IRQ event delivery to a driver endpoint**: static source→endpoint
  route, bounded one-byte events, deliver/pend/drop policy
  (`IRQ delivered` / pending / `IRQ_DROPPED reason=driver_not_ready`),
  drop wired into fault containment and kill.
* U-mode **`driver_manager`** (lifecycle policy, shell answers,
  liveness probe, restart requests) and U-mode
  **`block_driver_service` skeleton** (exercises every granted
  mechanism once, serves STATUS/FAULT).
* Shell commands `drivers`, `driver info block`, `driver restart
  block`, `driver fault block`.
* **Driver crash containment and recovery**: `driver fault block` is a
  contained user page fault (`CONTAIN ... kernel=alive`), the
  supervisor is notified, driver_manager observes the death via the
  probe, `driver restart block` re-arms the driver, which re-runs its
  start sequence; the shell, apps, filesystem, and storage service
  keep working throughout. The kernel stays alive.

No existing behavior regressed: all 13 pre-existing QEMU tests still
pass unchanged (the fs/storage image content still reports
`v1.4-storage-service` — deliberately untouched per the phase rule
"do not change storage semantics"); the shell kept all seven prior
capabilities and gained the driver-manager endpoint (8/8, asserted).

## 2. What remains prototype-only

* `block_driver_service` is a skeleton: it drives no device, serves no
  data, and implements no block protocol beyond its own status line.
* `driver_manager` manages exactly one statically-known driver; there
  is no driver discovery, no dynamic loading, no restart budget/policy
  beyond operator-triggered restart.
* The DMA model is a single static bounce page; no allocation, no
  scatter-gather, no device access to it.
* Driver restart does not reset any device hardware state (no device
  is driven, so none exists to reset).

## 3. Synthetic vs real

| Mechanism | Real | Synthetic / modeled |
|---|---|---|
| MMIO window | Real QEMU virt virtio-mmio transport at 0x1000_1000; the magic register genuinely reads 0x74726976 ("virt") through the mediated path | The device behind the window is neither probed nor driven; QEMU runs without a `-device virtio-blk-device`, so DeviceID reads 0 |
| MMIO access mechanism | Real: capability check + bounds + volatile access in the kernel | — |
| DMA | Buffer is a real, physically contiguous, identity-mapped kernel page (its VA is its PA) | No device ever masters it; "DMA-visible" is a property claim, not observed bus traffic; no cache maintenance, no IOMMU |
| IRQ | Delivery/pending/drop mechanism is real kernel code on a real endpoint | The interrupt source is synthetic: the PLIC is not programmed; `sys_irq_raise` (driver_control right, manager-only) injects events |

## 4. Is MMIO real or modeled?

The access mechanism and the bus window are **real** (see the
0x74726976 magic read in the test log). The *device semantics* are
**not exercised**: v1.5 makes no claim beyond "a U-mode driver can be
granted bounded, mediated access to a real MMIO window".

## 5. Is IRQ real or synthetic?

**Synthetic.** No PLIC initialization, no external interrupt handling.
The kernel-side delivery mechanism (route, bounded event, drop policy)
is real and is the piece a future PLIC integration plugs into.

## 6. Is DMA real or modeled?

**Modeled.** A real kernel page with the address property a virtio
driver needs, but no device DMA occurs. Coherency is trivially
satisfied on single-hart QEMU and is explicitly *not* solved for real
hardware (docs/31 §8).

## 7. Certification

**No certification claim.** Nothing in v1.5 is DO-178C/ISO 26262/IEC
61508 evidence; this is an emulator-only evaluation build.

## 8. Production drivers

**No production driver claim.** v1.5 is the minimal safe mechanism set
plus a skeleton; it is not a driver ecosystem and not virtio-blk.

## 9. Next phase

`v1.6-storage-backed-fs-and-loader`: connect storage and fs more
deeply and prepare restricted app image loading — still no arbitrary
ELF loading, no persistence claim, and no hardware claim. The
virtio-blk path itself (docs/30 §5, docs/31 §15) now has every kernel
mechanism it was blocked on.

## CI limitation

GitHub Actions workflows exist in-repo (`.github/workflows/`), but the
Actions gate on this push could not be confirmed from the local
environment at archive time; the authoritative evidence is the local
sweep in `verify_all.log` (VERIFY ALL: PASS, 14/14).
