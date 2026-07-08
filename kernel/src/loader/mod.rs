//! Restricted app image mapping admission (AXIOM-LOAD-009).
//!
//! Requirement reference: docs/32_RESTRICTED_APP_IMAGE_FORMAT.md §6/§7,
//! docs/12_MMU_SV39.md §5, docs/05_MEMORY_MODEL.md.
//!
//! v1.6 does NOT add a new kernel image-mapping syscall: restricted app
//! images are represented by the *existing* static service/app mapping
//! mechanism (`paging_hw::build_service_address_space`), which already
//! maps validated text as U+R+X, rodata as U+R, and stack as U+R+W, and
//! the page table's `validate_perms` already denies any W+X user mapping
//! (MEM-P5) and any user device mapping. The loader validates image
//! bounds, entry, and W^X-by-separation in user space (docs/32 §6).
//!
//! This module is the host-testable *model* of the admission rules such
//! a mapping must satisfy — the boundary a future map-validated-image
//! kernel mechanism would enforce. It parses no names and reads no image
//! bytes; it decides only whether a described layout may be mapped.

/// The kernel virtual base above which no user image may be mapped
/// (docs/12 §5: the user region lives well below the kernel at
/// 0x8020_0000). Any target at or above this is a kernel address and is
/// rejected.
pub const KERNEL_BASE: u64 = 0x8000_0000;

/// Maximum total image size a restricted image may map (docs/32 §6
/// rule 5): text + rodata, bounded.
pub const MAX_IMAGE_BYTES: u64 = 131_072;

/// A described restricted-image layout to be mapped at `base_va`.
/// Sizes are in bytes; `stack_pages` is the private stack request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageLayout {
    pub base_va: u64,
    pub entry_offset: u64,
    pub text_size: u64,
    pub rodata_size: u64,
    pub stack_pages: u64,
}

/// Why a layout may not be mapped (docs/32 §6). Distinct reasons so the
/// caller can report precisely and tests can pin each rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapReject {
    /// Empty or zero-length text: nothing executable to map.
    EmptyText,
    /// Entry point outside the text region (bad entry).
    EntryOutOfText,
    /// text + rodata exceeds the bounded image size (oversized).
    Oversized,
    /// The target VA (or the region it spans) reaches kernel space.
    KernelAddress,
    /// stack_pages outside policy (v1.6: exactly one).
    StackPolicy,
}

/// Admit or reject a restricted-image mapping (AXIOM-LOAD-009). W^X is
/// structural — text is mapped R+X, rodata R, stack R+W, never W+X — so
/// there is no "W^X" input to reject here; the rejectable conditions are
/// bad entry, oversized image, kernel-address target, and stack policy.
/// The region spans `[base_va, base_va + text_size + rodata_size)` plus
/// `stack_pages` pages; all of it must stay in user space.
pub fn admit_image_mapping(layout: &ImageLayout) -> Result<(), MapReject> {
    if layout.text_size == 0 {
        return Err(MapReject::EmptyText);
    }
    if layout.entry_offset >= layout.text_size {
        return Err(MapReject::EntryOutOfText);
    }
    // Oversized: text + rodata must be bounded (checked before span
    // arithmetic so an overflowing pair cannot slip through).
    let Some(image_bytes) = layout.text_size.checked_add(layout.rodata_size) else {
        return Err(MapReject::Oversized);
    };
    if image_bytes > MAX_IMAGE_BYTES {
        return Err(MapReject::Oversized);
    }
    if layout.stack_pages != 1 {
        return Err(MapReject::StackPolicy);
    }
    // Kernel-address rejection: the base, the image span, and the stack
    // pages that follow it must all stay strictly below KERNEL_BASE.
    // Any overflow in the span arithmetic is treated as reaching kernel
    // space (fails safe).
    let stack_bytes = layout.stack_pages.saturating_mul(4096);
    let end = image_bytes
        .checked_add(stack_bytes)
        .and_then(|span| layout.base_va.checked_add(span));
    match end {
        Some(e) if layout.base_va < KERNEL_BASE && e <= KERNEL_BASE => Ok(()),
        _ => Err(MapReject::KernelAddress),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A valid restricted-image layout (hello-sized, docs/32 §3).
    const OK: ImageLayout = ImageLayout {
        base_va: 0x1_0000,
        entry_offset: 0,
        text_size: 4096,
        rodata_size: 4096,
        stack_pages: 1,
    };

    #[test]
    fn a_valid_layout_is_admitted() {
        assert_eq!(admit_image_mapping(&OK), Ok(()));
    }

    /// Required test: W^X rejection — exercised through the real kernel
    /// map path, since a restricted image is mapped by exactly that
    /// mechanism. The three legitimate image permissions (text R+X,
    /// rodata R, stack R+W) all map; a W+X user page is rejected.
    #[test]
    fn wx_user_mapping_is_rejected_by_the_page_table() {
        use crate::memory::frame::{FrameOwner, PhysicalFrame};
        use crate::memory::pagetable::{MapError, PageTable, Permissions};
        use crate::memory::{AddressSpaceId, PhysAddr, VirtAddr};

        let asid = AddressSpaceId(7);
        let new_frame = || {
            let mut f = PhysicalFrame::new_free(PhysAddr::new(0x1000_0000));
            f.allocate(FrameOwner::AddressSpace(asid)).unwrap();
            f
        };
        // Text (R+X), rodata (R), stack (R+W) all map.
        for perms in [
            Permissions::user_rx(),
            Permissions::user_r(),
            Permissions::user_rw(),
        ] {
            let mut pt = PageTable::new(asid);
            let mut f = new_frame();
            assert!(pt.map(VirtAddr::new(0x2000), &mut f, perms).is_ok());
        }
        // A W+X user page — what a restricted image must never produce —
        // is rejected at the mechanism.
        let wx = Permissions {
            read: true,
            write: true,
            execute: true,
            user: true,
            device: false,
        };
        let mut pt = PageTable::new(asid);
        let mut f = new_frame();
        assert_eq!(
            pt.map(VirtAddr::new(0x2000), &mut f, wx),
            Err(MapError::WxViolation)
        );
    }

    /// Required test: kernel address rejection.
    #[test]
    fn kernel_address_target_is_rejected() {
        let at_kernel = ImageLayout {
            base_va: KERNEL_BASE,
            ..OK
        };
        assert_eq!(
            admit_image_mapping(&at_kernel),
            Err(MapReject::KernelAddress)
        );
        // A base low enough to start in user space but whose span
        // crosses into the kernel is also rejected.
        let spans_into_kernel = ImageLayout {
            base_va: KERNEL_BASE - 4096,
            ..OK
        };
        assert_eq!(
            admit_image_mapping(&spans_into_kernel),
            Err(MapReject::KernelAddress)
        );
        // Overflowing base fails safe.
        let overflow = ImageLayout {
            base_va: u64::MAX - 16,
            ..OK
        };
        assert_eq!(
            admit_image_mapping(&overflow),
            Err(MapReject::KernelAddress)
        );
    }

    /// Required test: oversized image rejection.
    #[test]
    fn oversized_image_is_rejected() {
        let big = ImageLayout {
            text_size: 65536,
            rodata_size: 65537, // 1 over the 131072 bound
            ..OK
        };
        assert_eq!(admit_image_mapping(&big), Err(MapReject::Oversized));
        // Overflowing size pair fails safe as oversized.
        let overflow = ImageLayout {
            text_size: u64::MAX,
            rodata_size: 4096,
            ..OK
        };
        assert_eq!(admit_image_mapping(&overflow), Err(MapReject::Oversized));
    }

    /// Required test: bad entry rejection.
    #[test]
    fn bad_entry_offset_is_rejected() {
        let at_text_end = ImageLayout {
            entry_offset: 4096, // == text_size, just past the last byte
            ..OK
        };
        assert_eq!(
            admit_image_mapping(&at_text_end),
            Err(MapReject::EntryOutOfText)
        );
        let empty_text = ImageLayout {
            text_size: 0,
            entry_offset: 0,
            ..OK
        };
        assert_eq!(admit_image_mapping(&empty_text), Err(MapReject::EmptyText));
    }

    #[test]
    fn stack_policy_is_one_page() {
        assert_eq!(
            admit_image_mapping(&ImageLayout {
                stack_pages: 0,
                ..OK
            }),
            Err(MapReject::StackPolicy)
        );
        assert_eq!(
            admit_image_mapping(&ImageLayout {
                stack_pages: 2,
                ..OK
            }),
            Err(MapReject::StackPolicy)
        );
    }
}
