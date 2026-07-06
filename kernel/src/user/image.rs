//! User task image model (AXIOM-USER-001).
//!
//! Requirement reference: docs/03_KERNEL_OBJECTS.md (Implementation
//! Notes), docs/05_MEMORY_MODEL.md §3.
//!
//! Describes *what* a user task is before it ever runs: entry point,
//! stack region, and owning address space. Validation happens at
//! construction — a descriptor whose entry or stack lies outside the
//! user virtual window cannot exist. No user-mode jump happens in this
//! task (Phase 7 boundary: AXIOM-USER-002 performs the transition).

use kernel::memory::address::{VirtAddr, PAGE_SIZE};
use kernel::memory::AddressSpaceId;

/// Explicit failure behavior for invalid image descriptors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageError {
    /// Entry point outside the user virtual window (docs/05 §3).
    EntryOutsideUserWindow,
    /// Stack region not fully inside the user virtual window.
    StackOutsideUserWindow,
    /// Stack must be at least one page and page-aligned.
    BadStackGeometry,
}

/// Descriptor of one user task image (v0.1: defined at boot, static).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserImage {
    entry: VirtAddr,
    stack_top: VirtAddr,
    stack_size: u64,
    address_space: AddressSpaceId,
}

impl UserImage {
    /// Validated construction. The stack grows downward from
    /// `stack_top`; the whole region `[stack_top - stack_size,
    /// stack_top)` must lie in the user window.
    pub fn new(
        entry: VirtAddr,
        stack_top: VirtAddr,
        stack_size: u64,
        address_space: AddressSpaceId,
    ) -> Result<Self, ImageError> {
        if !entry.is_user() {
            return Err(ImageError::EntryOutsideUserWindow);
        }
        if stack_size == 0 || !stack_size.is_multiple_of(PAGE_SIZE) || !stack_top.is_page_aligned()
        {
            return Err(ImageError::BadStackGeometry);
        }
        let top = stack_top.as_u64();
        let Some(base) = top.checked_sub(stack_size) else {
            return Err(ImageError::StackOutsideUserWindow);
        };
        // Top is exclusive: top-1 must be a user address, and so must
        // the base.
        if !VirtAddr::new(base).is_user() || !VirtAddr::new(top - 1).is_user() {
            return Err(ImageError::StackOutsideUserWindow);
        }
        Ok(UserImage {
            entry,
            stack_top,
            stack_size,
            address_space,
        })
    }

    pub const fn entry(&self) -> VirtAddr {
        self.entry
    }
    pub const fn stack_top(&self) -> VirtAddr {
        self.stack_top
    }
    pub const fn stack_size(&self) -> u64 {
        self.stack_size
    }
    pub const fn address_space(&self) -> AddressSpaceId {
        self.address_space
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ASID: AddressSpaceId = AddressSpaceId(1);

    #[test]
    fn valid_image_accepted() {
        let img = UserImage::new(
            VirtAddr::new(0x1_0000),
            VirtAddr::new(0x20_0000),
            4 * PAGE_SIZE,
            ASID,
        )
        .unwrap();
        assert_eq!(img.entry().as_u64(), 0x1_0000);
        assert_eq!(img.stack_size(), 4 * PAGE_SIZE);
    }

    #[test]
    fn kernel_entry_rejected() {
        assert_eq!(
            UserImage::new(
                VirtAddr::new(0x8020_0000),
                VirtAddr::new(0x20_0000),
                PAGE_SIZE,
                ASID
            ),
            Err(ImageError::EntryOutsideUserWindow)
        );
        assert_eq!(
            UserImage::new(VirtAddr::new(0), VirtAddr::new(0x20_0000), PAGE_SIZE, ASID),
            Err(ImageError::EntryOutsideUserWindow),
            "null entry can never exist"
        );
    }

    #[test]
    fn stack_must_stay_in_user_window() {
        // Stack that would underflow below the user window base.
        assert_eq!(
            UserImage::new(
                VirtAddr::new(0x1_0000),
                VirtAddr::new(0x2000),
                2 * PAGE_SIZE,
                ASID
            ),
            Err(ImageError::StackOutsideUserWindow)
        );
        // Stack top in kernel space.
        assert_eq!(
            UserImage::new(
                VirtAddr::new(0x1_0000),
                VirtAddr::new(0x8030_0000),
                PAGE_SIZE,
                ASID
            ),
            Err(ImageError::StackOutsideUserWindow)
        );
    }

    #[test]
    fn stack_geometry_validated() {
        assert_eq!(
            UserImage::new(VirtAddr::new(0x1_0000), VirtAddr::new(0x20_0000), 0, ASID),
            Err(ImageError::BadStackGeometry)
        );
        assert_eq!(
            UserImage::new(
                VirtAddr::new(0x1_0000),
                VirtAddr::new(0x20_0100),
                PAGE_SIZE,
                ASID
            ),
            Err(ImageError::BadStackGeometry),
            "unaligned stack top rejected"
        );
    }
}
