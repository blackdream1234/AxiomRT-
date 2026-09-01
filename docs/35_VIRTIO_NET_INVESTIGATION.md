# 35 — Virtio-Net Investigation

Document ID: created by AXIOM-NET-002 (Phase v1.7).
Requirement reference: docs/30, docs/31, docs/34, Virtio 1.3.

## 1. Decision

v1.7 does **not** implement real virtio-net. It implements a synthetic,
bounded packet service in U-mode. The model registers a synthetic `net0`
device identity and reuses capability-gated task, IPC, device-information,
synthetic-IRQ, fault-containment, and restart mechanisms.

No QEMU network backend is enabled by the v1.7 test. There is no host TAP,
user-network, socket, or internet dependency. The reported mode is always
`synthetic`.

## 2. QEMU options investigated

QEMU's RISC-V `virt` machine provides eight virtio-mmio transports and a
generic PCIe host bridge. The two relevant frontend choices are therefore:

```text
-netdev user,id=n0 -device virtio-net-device,netdev=n0
-netdev user,id=n0 -device virtio-net-pci,netdev=n0
```

The first choice uses the platform virtio/MMIO transport; the second requires
PCI discovery and configuration. Other host backends such as TAP, socket,
passt, and vhost-user change host connectivity, not the guest virtio driver
requirements. They are out of scope for deterministic v1.7 tests.

Official references checked for this investigation:

* QEMU RISC-V `virt` board:
  <https://www.qemu.org/docs/master/system/riscv/virt.html>
* QEMU network emulation:
  <https://www.qemu.org/docs/master/system/devices/net.html>
* QEMU virtio transport naming:
  <https://www.qemu.org/docs/master/devel/virtio-backends.html>
* OASIS Virtio 1.3:
  <https://docs.oasis-open.org/virtio/virtio/v1.3/virtio-v1.3.html>

## 3. MMIO versus PCI

AxiomRT's v1.5 mechanism and the `block0` skeleton use the first
virtio-mmio window at `0x1000_1000`. A future minimal network driver should
prefer another DTB-described virtio-mmio window and
`virtio-net-device`. That path reuses the existing offset-bounded MMIO
syscalls and avoids introducing PCI enumeration, BAR sizing/mapping, MSI/MSI-X,
and PCI capability parsing in the same phase.

The PCI path remains possible because QEMU `virt` exposes a PCIe host bridge,
but AxiomRT has no PCI discovery/configuration service. Selecting
`virtio-net-pci` now would require a separate, documented bus-driver phase
and must not be hidden inside network policy.

## 4. Device discovery requirements

Virtio-mmio has no generic enumeration protocol. A real implementation must
obtain each transport's base, 0x200-byte size, and interrupt from the
QEMU-generated device tree, or use a board-specific static table with the
same information. It then probes aligned 32-bit registers:

```text
MagicValue  0x000  expected 0x74726976
Version     0x004  expected 2 for modern MMIO
DeviceID    0x008  expected 1 for virtio-net; 0 means unused
VendorID    0x00c
```

v1.7 does not parse the DTB and does not probe a network transport. The
synthetic `net0` identity is explicit evidence, not a claim that a real
virtio-net device was discovered.

## 5. MMIO initialization requirements

A modern virtio-mmio driver needs capability-gated, aligned 32-bit access to
at least the feature, queue, notification, interrupt, and status registers:

```text
DeviceFeatures / DeviceFeaturesSel
DriverFeatures / DriverFeaturesSel
QueueSel / QueueNumMax / QueueNum / QueueReady
QueueDescLow/High
QueueDriverLow/High
QueueDeviceLow/High
QueueNotify
InterruptStatus / InterruptACK
Status
ConfigGeneration
device-specific configuration at 0x100+
```

It must follow the virtio status handshake, negotiate only understood
features, reject a zero DeviceID, configure physical queue addresses, publish
memory in the specified order, and reset safely after errors. The current
network service does none of these operations.

## 6. Virtqueue and DMA requirements

The simplest virtio-net device needs two queues:

```text
queue 0  receiveq1: device writes into driver-supplied buffers
queue 1  transmitq1: device reads driver-supplied packets
```

Each split virtqueue requires a 16-byte-aligned descriptor table, a
2-byte-aligned available ring, and a 4-byte-aligned used ring, with physical
addresses visible to the device. The driver also needs bounded RX/TX buffers,
virtio-net headers, memory barriers, descriptor ownership tracking, used-ring
reclamation, queue-full handling, and a defined maximum frame size.

The existing modeled 4096-byte bounce page demonstrates a confined DMA grant
but is insufficient as a production network DMA design. A real path needs
multiple or carefully partitioned DMA buffers, physical-address exposure,
cache-coherency rules, reset/revocation behavior, and preferably IOMMU
confinement. v1.7 performs no device DMA.

## 7. IRQ requirements

A real driver must receive the transport IRQ through the PLIC, read
`InterruptStatus`, process used RX/TX entries, acknowledge handled bits via
`InterruptACK`, and bound work per activation. It also needs masking,
coalescing, spurious-interrupt, and interrupt-storm policy.

AxiomRT v1.5 supplies a bounded endpoint-delivery model and drop behavior, but
the event source is synthetic and the PLIC is untouched. v1.7 reuses that
synthetic event only as a start/liveness probe. It is not evidence of a real
virtio-net interrupt.

## 8. Reusable v1.5 mechanisms

The following mechanisms can support a future real driver without moving
network policy into the kernel:

* isolated U-mode driver address spaces and watchdog containment;
* static device identities and deny-by-default device capabilities;
* offset/width-checked MMIO syscalls;
* bounded DMA-region mediation model;
* IRQ-to-endpoint delivery semantics and explicit drop behavior;
* bounded copy-based IPC;
* task restart with unchanged, boot-minted capabilities;
* driver-manager lifecycle policy.

The v1.7 `net_driver_service` also establishes the client/driver IPC boundary
and deterministic restart behavior that a real implementation can preserve.

## 9. Missing work

Before real virtio-net is credible, AxiomRT still needs:

* DTB parsing or a verified board-specific second virtio-mmio assignment;
* safe MMIO-write grants for the selected transport;
* complete virtio feature/status negotiation;
* DMA allocation/address disclosure beyond the single modeled bounce page;
* split-virtqueue implementation with memory-ordering tests;
* bounded RX buffer replenishment and TX reclamation;
* PLIC initialization and real interrupt routing;
* device reset and queue cleanup on driver restart;
* packet-size validation and hostile-device testing;
* a user-space Ethernet/network stack above the driver;
* hardware-specific cache/IOMMU policy and hardware evidence.

## 10. Architecture-law result

The synthetic choice respects the architecture law because the kernel gains
no network-specific parser, protocol, socket, route, or policy. It retains
only generic device identity, capability, IPC, event, and containment
mechanisms. Network request interpretation, counters, test-packet semantics,
driver-down policy, and restart decisions run in U-mode.

This phase therefore demonstrates service isolation and bounded network-shaped
IPC only. It makes no TCP/IP, socket, internet, real virtio-net, production
networking, real-hardware, security, safety, or certification claim.

## 11. Future path

A later virtio-net phase can replace the synthetic transition inside
`net_driver_service` while keeping `net_service`, shell authority, and the
kernel boundary stable. The first safe hardware-backed milestone should use
one RX queue, one TX queue, one in-flight test frame, a fixed maximum frame
size, polling before PLIC IRQs, no offloads, and an isolated QEMU network
backend. TCP/IP remains a separate user-space phase.
