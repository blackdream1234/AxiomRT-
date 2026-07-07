# 30 — Virtio-blk Investigation

Document ID: created by AXIOM-STOR-010 (Phase v1.4).
Requirement reference: `AxiomRTos.md` §7, docs/29 §10.

## 1. QEMU virtio-blk model

QEMU `virt` exposes virtio-mmio transports at 0x1000_1000–0x1000_8000
(one 0x200 window each, IRQs 1–8 on the PLIC) when started with
`-drive file=...,if=none,id=d0 -device virtio-blk-device,drive=d0`.
Device discovery = probing each window's magic ("virt"), version, and
device-id registers (2 = block). Legacy (version 1) and modern
(version 2) register layouts differ; QEMU 11 offers modern.

## 2. Discovery path

MMIO probing only (no PCI on virt's default virtio-mmio path). A
driver reads MagicValue/Version/DeviceID, negotiates features, sets up
one virtqueue (descriptor/avail/used rings in driver-owned memory),
and kicks via QueueNotify; completion arrives by PLIC interrupt or by
polling the used ring.

## 3. Required kernel mechanism (not yet present)

1. **MMIO grant**: map one device window (U=1, non-executable, device
   memory) into a driver's address space via a device capability —
   today only the kernel maps MMIO (UART, kernel-only).
2. **DMA-visible buffer grant**: virtqueue rings and data buffers need
   physical addresses the device can reach; a driver needs a
   capability to learn the physical address of its own pages
   (single-hart, no IOMMU on virt — a real board would add one more
   trust assumption to state).
3. **IRQ routing**: PLIC init in the kernel plus an interrupt→endpoint
   event delivery so a U-mode driver can block on its IRQ.

These are exactly the "device object / MMIO grant / IRQ event"
mechanisms the v1.5 driver-framework phase specifies (AXIOM-DRV-002).

## 4. What stays in user space

Everything else: probing policy, feature negotiation, virtqueue
management, request building, retries, the block cache, and the
storage protocol front-end (docs/29 §4 — unchanged for clients).

## 5. Minimal viable block driver design

`block_driver_service` (U-mode): one virtqueue, one in-flight request,
polling first (IRQ wiring second), 512-byte sectors repackaged into
the 48-byte block protocol (sector cache in its stack/data page),
read-only feature set. storage_service switches its backing from the
static image to IPC calls into the driver — clients see no change.

## 6. Risks

DMA into wrong physical memory (no IOMMU: driver pages must be
validated by the kernel grant), device-register parsing bugs (contained
user fault, but availability loss), interrupt storms (bounded by PLIC
masking policy in the kernel mechanism), and feature-negotiation
divergence between QEMU versions.

## 7. Why the full driver is not implemented now

The kernel currently has no MMIO-grant, DMA-address, or IRQ-delivery
mechanism, and the Architecture Law forbids parking a complex driver
in the kernel as a shortcut. Those mechanisms are the defined scope of
phase v1.5 (AXIOM-DRV); implementing them ad hoc inside the storage
phase would be phase-jumping (AxiomRTos.md §3.8). v1.4 therefore
delivers the storage *protocol boundary* with a static backing store,
so the driver phase can swap the backing without touching any client.
