# 31 — User-Space Driver Framework

Document ID: created by AXIOM-DRV-001 (Phase v1.5).
Requirement reference: `AxiomRT v1.5.md`, docs/30 §3, docs/06, docs/25.

## 1. Why drivers are user-space

A driver is the least-trusted code that touches the most dangerous
resources (device registers, DMA, interrupts). In a monolithic kernel a
driver bug is a kernel bug. In AxiomRT a driver is an ordinary U-mode
task: it runs in its own Sv39 address space, holds only the
capabilities its service-table entry mints, and a driver fault is a
contained user fault (docs/06_FAULT_MODEL.md) — the kernel, the shell,
and every other service keep running. The kernel provides *mechanism*
(device identity, capability-gated MMIO/DMA/IRQ access, containment);
all driver *policy* stays in user space.

## 2. What a driver is in AxiomRT

A driver is a U-mode service that:

* is started and tracked by `driver_manager` (not by init directly),
* holds a **device capability** naming exactly one device object,
* reaches device state only through capability-checked syscalls,
* serves a bounded IPC protocol to its clients,
* may fault and be restarted without kernel involvement beyond
  ordinary fault containment.

## 3. Driver lifecycle

States tracked by `driver_manager` (policy) over the kernel task
states (mechanism, docs/09 §4):

```text
Available  -> known in the service table, not yet started
Starting   -> start requested, device grants being minted
Running    -> serving its endpoint
Blocked    -> waiting in bounded IPC (normal operation)
Faulted    -> contained user fault (kernel alive, task stopped)
Killed     -> stopped by operator/supervisor decision
Restarted  -> re-armed with its initial frame and unchanged grants
```

Transitions are observable: the kernel emits `SERVICE started=`,
`FAULT`/`CONTAIN`, `TASK_KILLED`, `TASK_RESTARTED`; the manager emits
`DRIVER started=`, `DRIVER restarted=`, and its observation of faults.

## 4. driver_manager responsibilities

U-mode service (slot 12, table index 11, prio 2). It:

* starts driver services (it owns driver start order, not init),
* tracks each driver's lifecycle state,
* answers the shell's `drivers` / `driver info` / `driver restart` /
  `driver fault` lines over the driver-manager endpoint (EP_DRV = 6),
* observes driver failure (via the synthetic-IRQ liveness probe, §9,
  and refuses to IPC a dead driver),
* requests driver restart through its task-control capability,
* never parses device registers or block protocol — that is the
  driver's job.

## 5. block_driver_service skeleton responsibilities

U-mode service (slot 13, table index 12, prio 2). v1.5 scope is a
**skeleton**: it exercises every kernel mechanism once and serves a
synthetic status protocol; it does not drive a real block device.

* On start: reads its device object description (`sys_device_info`),
  performs one real MMIO read of the virtio-mmio magic register,
  demonstrates one denied MMIO write (right not granted), writes and
  reads back one byte of its granted DMA buffer, then blocks on its
  IRQ endpoint until the boot attention event arrives.
* Then serves `STATUS` / `FAULT` commands from `driver_manager` over
  EP_BLK (= 7), one bounded request, one bounded reply.
* `FAULT` makes it dereference an unmapped address: a deliberate,
  contained page fault for the containment test.
* It can be restarted; restart re-runs the start sequence.

It does **not** replace `storage_service`; the storage path and its
protocol are unchanged (docs/29).

## 6. Device object model

The kernel holds a static table of device objects. v1.5 registers one:

```text
DeviceId    0
name        block0
DeviceKind  BlockDeviceSkeleton
MmioRegion  virtio_mmio0: base=0x1000_1000 size=0x200
IrqLine     synthetic, routed to endpoint EP_IRQ (= 8)
DmaRegion   block0_dma: one kernel bounce page, 4096 bytes
```

The MMIO base is the first virtio-mmio transport window of the QEMU
`virt` machine (docs/30 §1) — a real bus window. The device *behind*
it is not probed or driven in v1.5; the QEMU test runs without a
`-device virtio-blk-device`, so the window's DeviceID register reads 0.
The host model (`kernel/src/device`) mirrors these concepts
(`DeviceId`, `DeviceKind`, `DeviceObject`, `MmioRegion`, `IrqLine`,
`DmaRegion`, `DeviceCapability`) for host-testable checking logic.

## 7. MMIO grant model

MMIO access is **syscall-mediated**, never direct: no device page is
ever mapped U=1 into a driver. A driver calls

```text
sys_mmio_read(device_cap, offset, width)
sys_mmio_write(device_cap, offset, width, value)
```

and the kernel (1) resolves the device capability (deny-by-default,
fixed check order, docs/06 §4), (2) checks `offset`/`width` against
the device's `MmioRegion` (width 1/2/4, aligned, in bounds), then
(3) performs the one volatile access itself. A driver cannot name an
arbitrary physical address — only an offset inside the region its
capability grants. Denials emit `MMIO_DENIED` with a reason and touch
nothing. The kernel never interprets what a register means (no virtio
queue policy in the kernel).

The grant is logged when the driver's capabilities are minted:
`MMIO grant task=block_driver_service device=block0 region=virtio_mmio0`.

## 8. DMA-visible buffer grant model

v1.5 models DMA with a **kernel-owned static bounce page**
(`block0_dma`, 4096 bytes): physically contiguous by construction and
identity-mapped, so its kernel VA is its physical address — the
property a future virtio driver needs. Access is syscall-mediated and
capability-gated exactly like MMIO:

```text
sys_dma_read(device_cap, offset, width)
sys_dma_write(device_cap, offset, width, value)
```

Bounds are enforced against the granted region only; a driver cannot
use arbitrary kernel memory as DMA and cannot write outside its
buffer. Only the kernel grant defines the buffer; user tasks cannot
allocate DMA memory in v1.5. **No real device DMA occurs**: nothing
programs a device with this buffer's address yet. Cache/coherency: on
single-hart QEMU `virt` the model is trivially coherent; a real board
would add explicit cache maintenance and (ideally) an IOMMU — neither
is claimed here (docs/30 §3).

## 9. IRQ event delivery model

v1.5 IRQ delivery is **synthetic**: the PLIC is not programmed. The
mechanism is real; the interrupt source is not.

* The kernel statically routes the device's IRQ line to endpoint
  `EP_IRQ` and logs `IRQ registered source=block0 endpoint=driver_irq`
  at device registration.
* `sys_irq_raise(device_cap)` — requiring the `driver_control` right,
  held only by `driver_manager` — injects one synthetic device event.
  It stands in for a real PLIC interrupt until the hardware path
  exists.
* Delivery is a bounded one-byte event. If the authorized driver is
  blocked receiving on EP_IRQ, it is delivered immediately
  (`IRQ delivered to=block_driver_service source=block0`). If the
  driver is alive but not waiting, the event is held pending
  (coalesced: one bit, re-raising while pending is idempotent). If the
  driver is Faulted/Killed, the event is dropped and logged
  (`IRQ_DROPPED reason=driver_not_ready`) — documented drop policy,
  and the raise returns an error code, which is how `driver_manager`
  observes driver death (liveness probe).
* Delivery uses only the EP_IRQ waiter slot and the per-source pending
  bit; it cannot overwrite unrelated endpoint state. Receiving on
  EP_IRQ requires an ordinary endpoint Recv capability, held only by
  the driver — only the authorized driver can receive the event.

## 10. Capability model

Device capabilities are a new on-target object type (`device`,
OTYPE_DEVICE) carrying `(device_id, rights)`. Rights are
deny-by-default bits, meaningful only on device capabilities:

```text
device_info     1<<0   read the device object description
mmio_read       1<<1   sys_mmio_read on the granted region
mmio_write      1<<2   sys_mmio_write on the granted region
dma_read        1<<3   sys_dma_read on the granted buffer
dma_write       1<<4   sys_dma_write on the granted buffer
irq_receive     1<<5   be the registered IRQ event receiver
driver_control  1<<6   sys_irq_raise (synthetic injection / probe)
```

v1.5 grants (least privilege, docs/25 §5):

* `block_driver_service`: `device_info | mmio_read | dma_read |
  dma_write | irq_receive` — **not** `mmio_write` (the skeleton never
  writes a register; its one write attempt is denied on purpose) and
  **not** `driver_control`.
* `driver_manager`: `driver_control` only — it can probe/inject but
  cannot touch MMIO or DMA at all.
* Shell, apps, `fault_demo`, and every other task: **no device
  capability**. The shell reaches drivers only by forwarding text
  lines to `driver_manager` over EP_DRV; it can never reach MMIO.

Capability lookup happens before any device operation; invalid ids,
empty slots, wrong object types, and missing rights fail safely with
distinct errors and `MMIO_DENIED`/`DMA_DENIED`/`IRQ_DENIED` evidence.
Rights never amplify on derivation (host-tested).

Capability capacity: `CAPS_PER_TASK` stays 8. The shell's table
becomes full (8/8: line, console, info, control, app, fs, storage,
driver-manager endpoint) — checked explicitly by the QEMU test; no
existing capability is dropped.

## 11. Kernel boundary

Kernel (mechanism only): device object identity, device capability
type and checks, syscall-mediated MMIO/DMA access with bounds
checking, synthetic IRQ routing/delivery/drop, fault containment,
task start/restart. The kernel does **not** contain: driver policy,
block or virtio protocol, request queues, filesystem or path logic,
storage cache policy, shell logic, restart policy (the manager decides
when), or certification claims.

## 12. Security risks

* MMIO writes can reconfigure a device; v1.5 mitigates by granting
  `mmio_write` to no task and mediating every access — but the
  mechanism exists and future grants widen the attack surface.
* DMA is the classic isolation escape: a device writing physical
  memory bypasses the MMU. v1.5 has no device DMA; the model confines
  future DMA to kernel-granted buffers, but without an IOMMU a
  misprogrammed device could still write anywhere — a stated trust
  assumption for the future hardware phase (docs/30 §6).
* `sys_irq_raise` is an injection primitive; it is gated on
  `driver_control` (manager only) and delivers one inert byte.
* A malicious driver can lie to its clients about device state; v1.5
  clients (none yet) must treat driver replies as untrusted input.

## 13. Safety risks

* A faulted driver means loss of its device's availability until
  restart — containment preserves the system, not the service.
* Restart re-arms the initial frame but does not reset device
  hardware state (no device is driven yet; a real driver phase needs
  a documented reset-on-restart story).
* IRQ drop policy favors safety over completeness: events raised
  while the driver is dead are lost, not queued.
* The synthetic IRQ path exercises delivery but not interrupt-storm
  behavior; PLIC masking policy is future work (docs/30 §6).

## 14. Limitations

* v1.5 is **not** full production driver support.
* v1.5 is **not** a virtio-blk implementation: no feature negotiation,
  no virtqueue, no data transfer from any device.
* MMIO: the *window* and the access mechanism are real; the device
  behind it is neither probed nor driven (DeviceID is not required to
  be nonzero).
* DMA: modeled with a kernel bounce page; no device ever masters it.
* IRQ: synthetic injection only; the PLIC is untouched.
* No writable persistent storage, no crash-consistency claim, no
  networking, no real-hardware BSP, and **no certification claim**.

## 15. Future virtio-blk path

With these mechanisms, the docs/30 §5 design becomes implementable
without new kernel policy: the driver probes the window
(`sys_mmio_read` of magic/version/device-id), negotiates features via
mediated register writes (grant `mmio_write` then), builds one
virtqueue inside `block0_dma` (its physical address is knowable by
construction), kicks via QueueNotify, and completes by polling first,
then by a real PLIC interrupt routed to EP_IRQ replacing
`sys_irq_raise`. `storage_service` then swaps its static image for
IPC calls into the driver; storage clients see no change (docs/29 §7).

## 16. Future real hardware path

A real board adds: a BSP with the board's MMIO map (device tree
parsing stays in user space; the kernel table stays static per
build), real PLIC init and per-source masking, cache maintenance for
DMA buffers, an IOMMU if present (or a documented trust assumption if
not), and a device-reset protocol for driver restart. None of this is
started in v1.5 (AXIOM-HW is blocked on physical hardware).
