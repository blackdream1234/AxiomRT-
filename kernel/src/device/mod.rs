//! Device object and device capability model (AXIOM-DRV-002).
//!
//! Requirement reference: docs/31_USER_SPACE_DRIVER_FRAMEWORK.md §6/§10,
//! docs/06_CAPABILITY_MODEL.md.
//!
//! The host-testable model of the v1.5 user-space driver framework: a
//! static table of device objects (identity + MMIO region + IRQ line +
//! DMA region) reached exclusively through device capabilities. Rights
//! are deny-by-default; checks run in a fixed order (id → rights →
//! operation bounds) before any device state is touched; derivation can
//! only diminish rights. The on-target twin lives in the riscv64
//! dispatcher (same rights bit values, docs/31 §10).

/// Identity of one device object in the kernel device table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceId(pub u32);

/// Device classes known to the kernel. v1.5 supports only the block
/// device skeleton (docs/31 §5); the kind carries no protocol policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    BlockDeviceSkeleton,
}

/// One MMIO register window (docs/31 §7). `base` is a physical bus
/// address; user code never sees it — only offsets inside the region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MmioRegion {
    pub base: u64,
    pub size: u64,
}

/// One interrupt source, statically routed to an endpoint (docs/31 §9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IrqLine {
    pub endpoint: u32,
}

/// One DMA-visible buffer grant (docs/31 §8). v1.5: a kernel bounce
/// page; `base` is its physical (= kernel virtual) address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DmaRegion {
    pub base: u64,
    pub size: u64,
}

/// A device object: identity plus the three resources a driver may be
/// granted mediated access to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceObject {
    pub id: DeviceId,
    pub kind: DeviceKind,
    pub mmio: MmioRegion,
    pub irq: IrqLine,
    pub dma: DmaRegion,
}

/// Device access rights (docs/31 §10). Explicit bitmask, deny by
/// default, no implicit widening. Bit values match the on-target
/// dispatcher constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceRights(u16);

impl DeviceRights {
    pub const NONE: DeviceRights = DeviceRights(0);
    pub const DEVICE_INFO: DeviceRights = DeviceRights(1 << 0);
    pub const MMIO_READ: DeviceRights = DeviceRights(1 << 1);
    pub const MMIO_WRITE: DeviceRights = DeviceRights(1 << 2);
    pub const DMA_READ: DeviceRights = DeviceRights(1 << 3);
    pub const DMA_WRITE: DeviceRights = DeviceRights(1 << 4);
    pub const IRQ_RECEIVE: DeviceRights = DeviceRights(1 << 5);
    pub const DRIVER_CONTROL: DeviceRights = DeviceRights(1 << 6);

    pub const fn union(self, other: DeviceRights) -> DeviceRights {
        DeviceRights(self.0 | other.0)
    }

    /// Subset test: does this set include every right in `required`?
    pub const fn contains(self, required: DeviceRights) -> bool {
        self.0 & required.0 == required.0
    }

    /// Remove rights — the only direction rights can change.
    pub const fn diminish(self, removed: DeviceRights) -> DeviceRights {
        DeviceRights(self.0 & !removed.0)
    }

    pub const fn bits(self) -> u16 {
        self.0
    }
}

/// A device capability: (device id, rights). Kernel-held; user code
/// holds only a capability-table index (docs/06 §1 unforgeability).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceCapability {
    device: DeviceId,
    rights: DeviceRights,
}

impl DeviceCapability {
    /// Mint a device capability (kernel-internal, boot-time in v1.5).
    pub const fn new(device: DeviceId, rights: DeviceRights) -> Self {
        DeviceCapability { device, rights }
    }

    pub const fn device(&self) -> DeviceId {
        self.device
    }

    pub const fn rights(&self) -> DeviceRights {
        self.rights
    }

    /// Derive a weaker capability for the same device (no amplification
    /// operation exists, docs/03 §8).
    pub const fn derive_diminished(&self, removed: DeviceRights) -> DeviceCapability {
        DeviceCapability {
            device: self.device,
            rights: self.rights.diminish(removed),
        }
    }
}

/// Explicit failure behavior of device access checks (docs/31 §10),
/// mirroring the capability table's fixed-order errors (docs/06 §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceAccessError {
    /// No capability was presented at all (empty slot / no authority).
    NoCapability,
    /// The capability names a device id the table does not hold.
    UnknownDevice,
    /// The capability targets a different device than the operation.
    WrongDevice,
    /// Held rights do not include the required rights.
    InsufficientRights,
    /// Offset/width outside the granted MMIO or DMA region.
    OutOfBounds,
}

/// The kernel device table (static; v1.5 holds one device). The single
/// check point for every device operation: resolve the capability
/// against the table in fixed order before touching anything.
pub struct DeviceTable<const N: usize> {
    devices: [DeviceObject; N],
}

impl<const N: usize> DeviceTable<N> {
    pub const fn new(devices: [DeviceObject; N]) -> Self {
        DeviceTable { devices }
    }

    pub fn get(&self, id: DeviceId) -> Option<&DeviceObject> {
        self.devices.iter().find(|d| d.id == id)
    }

    /// The enforcement point (docs/31 §10). Fixed check order:
    /// capability presence → device id known → id matches the target →
    /// rights subset. Only full success returns the device object.
    pub fn check(
        &self,
        cap: Option<&DeviceCapability>,
        target: DeviceId,
        required: DeviceRights,
    ) -> Result<&DeviceObject, DeviceAccessError> {
        let Some(cap) = cap else {
            return Err(DeviceAccessError::NoCapability);
        };
        let Some(dev) = self.get(cap.device()) else {
            return Err(DeviceAccessError::UnknownDevice);
        };
        if dev.id != target {
            return Err(DeviceAccessError::WrongDevice);
        }
        if !cap.rights().contains(required) {
            return Err(DeviceAccessError::InsufficientRights);
        }
        Ok(dev)
    }
}

/// Validate one mediated access of `width` bytes at `offset` inside a
/// region of `size` bytes: width ∈ {1,2,4}, aligned, in bounds
/// (docs/31 §7/§8). Pure bounds logic shared by the MMIO and DMA
/// checks; the on-target syscalls apply the same rule.
pub fn access_in_bounds(size: u64, offset: u64, width: u64) -> bool {
    (width == 1 || width == 2 || width == 4)
        && offset.is_multiple_of(width)
        && offset.checked_add(width).is_some_and(|end| end <= size)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLOCK0: DeviceObject = DeviceObject {
        id: DeviceId(0),
        kind: DeviceKind::BlockDeviceSkeleton,
        mmio: MmioRegion {
            base: 0x1000_1000,
            size: 0x200,
        },
        irq: IrqLine { endpoint: 8 },
        dma: DmaRegion {
            base: 0x8060_0000,
            size: 4096,
        },
    };

    fn table() -> DeviceTable<1> {
        DeviceTable::new([BLOCK0])
    }

    /// Required test 1: device capability creation.
    #[test]
    fn device_capability_creation_carries_id_and_rights() {
        let cap = DeviceCapability::new(
            DeviceId(0),
            DeviceRights::DEVICE_INFO.union(DeviceRights::MMIO_READ),
        );
        assert_eq!(cap.device(), DeviceId(0));
        assert!(cap.rights().contains(DeviceRights::DEVICE_INFO));
        assert!(cap.rights().contains(DeviceRights::MMIO_READ));
        assert!(!cap.rights().contains(DeviceRights::MMIO_WRITE));
        let t = table();
        let dev = t
            .check(Some(&cap), DeviceId(0), DeviceRights::MMIO_READ)
            .expect("valid capability must resolve");
        assert_eq!(dev.kind, DeviceKind::BlockDeviceSkeleton);
    }

    /// Required test 2: insufficient rights are rejected.
    #[test]
    fn insufficient_rights_are_rejected() {
        let cap = DeviceCapability::new(DeviceId(0), DeviceRights::MMIO_READ);
        assert_eq!(
            table().check(Some(&cap), DeviceId(0), DeviceRights::MMIO_WRITE),
            Err(DeviceAccessError::InsufficientRights)
        );
        // A read+write requirement is not met by read alone.
        assert_eq!(
            table().check(
                Some(&cap),
                DeviceId(0),
                DeviceRights::MMIO_READ.union(DeviceRights::MMIO_WRITE),
            ),
            Err(DeviceAccessError::InsufficientRights)
        );
    }

    /// Required test 3: a wrong device id is rejected.
    #[test]
    fn wrong_device_id_is_rejected() {
        // Capability naming a device the table does not hold.
        let stale = DeviceCapability::new(DeviceId(9), DeviceRights::MMIO_READ);
        assert_eq!(
            table().check(Some(&stale), DeviceId(9), DeviceRights::MMIO_READ),
            Err(DeviceAccessError::UnknownDevice)
        );
        // Valid capability used against a different target device.
        let cap = DeviceCapability::new(DeviceId(0), DeviceRights::MMIO_READ);
        assert_eq!(
            table().check(Some(&cap), DeviceId(1), DeviceRights::MMIO_READ),
            Err(DeviceAccessError::WrongDevice)
        );
    }

    /// Required test 4: no authority means no access.
    #[test]
    fn no_capability_means_no_access() {
        assert_eq!(
            table().check(None, DeviceId(0), DeviceRights::NONE),
            Err(DeviceAccessError::NoCapability)
        );
        // Even asking for nothing requires presenting a capability.
        let none = DeviceCapability::new(DeviceId(0), DeviceRights::NONE);
        assert_eq!(
            table().check(Some(&none), DeviceId(0), DeviceRights::DEVICE_INFO),
            Err(DeviceAccessError::InsufficientRights)
        );
    }

    /// Required test 5: rights do not amplify by derivation.
    #[test]
    fn derivation_never_amplifies_rights() {
        let parent = DeviceCapability::new(
            DeviceId(0),
            DeviceRights::DEVICE_INFO
                .union(DeviceRights::MMIO_READ)
                .union(DeviceRights::DMA_READ),
        );
        let child = parent.derive_diminished(DeviceRights::DMA_READ);
        assert!(child.rights().contains(DeviceRights::MMIO_READ));
        assert!(!child.rights().contains(DeviceRights::DMA_READ));
        // Diminishing by an unheld right adds nothing.
        let same = child.derive_diminished(DeviceRights::DRIVER_CONTROL);
        assert_eq!(same.rights(), child.rights());
        // No operation can produce a right the parent never held.
        assert!(!parent.rights().contains(DeviceRights::MMIO_WRITE));
        assert!(!child.rights().contains(DeviceRights::MMIO_WRITE));
        assert_eq!(child.device(), parent.device(), "same object, less power");
    }

    #[test]
    fn all_seven_device_rights_are_distinct() {
        let all = [
            DeviceRights::DEVICE_INFO,
            DeviceRights::MMIO_READ,
            DeviceRights::MMIO_WRITE,
            DeviceRights::DMA_READ,
            DeviceRights::DMA_WRITE,
            DeviceRights::IRQ_RECEIVE,
            DeviceRights::DRIVER_CONTROL,
        ];
        for (i, a) in all.iter().enumerate() {
            for (j, b) in all.iter().enumerate() {
                if i != j {
                    assert!(!a.contains(*b), "right {i} must not imply right {j}");
                }
            }
        }
    }

    /// AXIOM-DRV-003: an MMIO operation is legal only when the fixed
    /// order holds end to end — capability check first, then bounds
    /// against the granted region (never against anything wider).
    #[test]
    fn mmio_access_requires_capability_then_bounds() {
        let t = table();
        let cap = DeviceCapability::new(DeviceId(0), DeviceRights::MMIO_READ);
        let dev = t
            .check(Some(&cap), DeviceId(0), DeviceRights::MMIO_READ)
            .expect("read capability resolves");
        // In-region access is accepted; the same offset is rejected the
        // moment it leaves the granted window.
        assert!(access_in_bounds(dev.mmio.size, 0x1fc, 4));
        assert!(!access_in_bounds(dev.mmio.size, 0x200, 4));
        // A write with a read-only grant never reaches the bounds check.
        assert_eq!(
            t.check(Some(&cap), DeviceId(0), DeviceRights::MMIO_WRITE),
            Err(DeviceAccessError::InsufficientRights)
        );
    }

    #[test]
    fn access_bounds_reject_misaligned_and_out_of_range() {
        // In bounds, aligned.
        assert!(access_in_bounds(0x200, 0, 4));
        assert!(access_in_bounds(0x200, 0x1fc, 4));
        assert!(access_in_bounds(0x200, 0x1ff, 1));
        // Bad width.
        assert!(!access_in_bounds(0x200, 0, 8));
        assert!(!access_in_bounds(0x200, 0, 0));
        // Misaligned.
        assert!(!access_in_bounds(0x200, 2, 4));
        assert!(!access_in_bounds(0x200, 1, 2));
        // Out of range, including overflow.
        assert!(!access_in_bounds(0x200, 0x200, 1));
        assert!(!access_in_bounds(0x200, 0x1fd, 4));
        assert!(!access_in_bounds(0x200, u64::MAX - 3, 4));
    }
}
