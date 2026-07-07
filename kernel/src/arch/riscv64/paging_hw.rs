//! Sv39 activation on target (AXIOM-MEMHW-004; user address space added
//! by AXIOM-MEMHW-005..007).
//!
//! Requirement reference: docs/12_MMU_SV39.md §4, §5.
//!
//! Builds the kernel identity map from the linker section symbols
//! (correct per-region permissions), writes `satp`, and issues
//! `sfence.vma`. A per-task user page table contains the same kernel
//! mappings (U=0, so the S-mode trap handler runs after a U→S trap)
//! plus the task's own U=1 pages. riscv64-only.

use kernel::memory::pagetable::Permissions;
use kernel::memory::paging::{Arena, Table, ARENA_TABLES};
use kernel::memory::sv39::satp_for;
use kernel::memory::{PhysAddr, VirtAddr};

use crate::uart;

// Section boundaries from kernel/linker.ld (all 4 KiB aligned).
extern "C" {
    static __text_start: u8;
    static __text_end: u8;
    static __rodata_start: u8;
    static __rodata_end: u8;
    static __data_start: u8;
    static __data_end: u8;
}

/// UART MMIO page (QEMU virt NS16550A), kernel-only device mapping.
const UART_PAGE: u64 = 0x1000_0000;

/// Static kernel table arena, 4 KiB aligned (Table is align(4096)).
/// Lives in .bss, therefore inside the R+W data span it maps for itself.
static mut KERNEL_TABLES: [Table; ARENA_TABLES] = [Table::zeroed(); ARENA_TABLES];

/// Maximum concurrent user address spaces on target (v0.3).
pub const MAX_USER_AS: usize = 12;

/// Static user table arenas, one per user address space.
static mut USER_TABLES: [[Table; ARENA_TABLES]; MAX_USER_AS] =
    [[Table::zeroed(); ARENA_TABLES]; MAX_USER_AS];

fn sym(addr: &'static u8) -> u64 {
    // addr_of! obtains the linker symbol address without forming a
    // reference to the (zero-sized) location.
    core::ptr::addr_of!(*addr) as u64
}

/// Kernel region boundaries resolved from the linker symbols.
struct KernelRegions {
    text: (u64, u64),
    rodata: (u64, u64),
    data: (u64, u64),
}

fn kernel_regions() -> KernelRegions {
    KernelRegions {
        text: (sym(unsafe { &__text_start }), sym(unsafe { &__text_end })),
        rodata: (
            sym(unsafe { &__rodata_start }),
            sym(unsafe { &__rodata_end }),
        ),
        data: (sym(unsafe { &__data_start }), sym(unsafe { &__data_end })),
    }
}

/// Map the kernel regions into `arena` with correct per-region
/// permissions (all U=0). Shared by the kernel table and every user
/// table so the S-mode trap handler is reachable after a U→S trap
/// (docs/12_MMU_SV39.md §5).
fn map_kernel_regions(arena: &mut Arena<'_>, r: &KernelRegions) {
    let krx = Permissions {
        read: true,
        write: false,
        execute: true,
        user: false,
        device: false,
    };
    let kr_only = Permissions {
        read: true,
        write: false,
        execute: false,
        user: false,
        device: false,
    };
    let krw = Permissions::kernel_rw();
    let kdev = Permissions::kernel_device();

    arena
        .identity_map_range(r.text.0, r.text.1, krx)
        .expect("map kernel text");
    if r.rodata.1 > r.rodata.0 {
        arena
            .identity_map_range(r.rodata.0, r.rodata.1, kr_only)
            .expect("map kernel rodata");
    }
    arena
        .identity_map_range(r.data.0, r.data.1, krw)
        .expect("map kernel data");
    arena
        .identity_map_range(UART_PAGE, UART_PAGE + 0x1000, kdev)
        .expect("map uart");
}

/// Build the kernel page table and enable Sv39. Returns after the MMU
/// is active; all subsequent kernel accesses are translated.
pub fn enable_kernel_paging() {
    let r = kernel_regions();
    // SAFETY (docs/07_CODEX_RULES.md §6): KERNEL_TABLES is a static
    // accessed once, before paging is enabled, from single-hart boot
    // context; no other reference exists. addr_of_mut! avoids forming a
    // reference to the large array.
    let base_pa = core::ptr::addr_of!(KERNEL_TABLES) as u64;
    let tables: &mut [Table] = unsafe { &mut *core::ptr::addr_of_mut!(KERNEL_TABLES) };
    let mut arena = Arena::new(tables, base_pa);

    map_kernel_regions(&mut arena, &r);

    let satp = satp_for(arena.root_pa());
    // SAFETY: writing satp with a valid Sv39 root (identity-mapped, so
    // PC/SP stay valid across the switch) enables translation; the
    // following sfence.vma flushes stale TLB entries
    // (docs/12_MMU_SV39.md §4).
    unsafe {
        core::arch::asm!(
            "csrw satp, {satp}",
            "sfence.vma",
            satp = in(reg) satp,
            options(nostack, preserves_flags)
        );
    }

    uart::put_str("MMU status=enabled mode=sv39 scope=kernel\n");
}

/// A built user address space: its root physical address plus the user
/// virtual addresses the caller uses to enter it.
pub struct UserAddressSpace {
    pub root: PhysAddr,
    pub entry_va: u64,
    pub stack_top_va: u64,
}

/// User virtual layout for the demo task (docs/12_MMU_SV39.md §5).
const USER_CODE_VA: u64 = 0x1_0000;
const USER_STACK_VA: u64 = 0x20_0000;
const USER_STACK_PAGES: u64 = 1;

/// Build a user address space (AXIOM-MEMHW-005/006; multi-AS in v0.3):
/// kernel mappings (U=0) for the trap handler, plus the task's U=1 code
/// and stack pages mapped at user virtual addresses. `as_index` selects
/// one of the MAX_USER_AS static table arenas.
///
/// `code_phys` is the physical address of the user entry function, and
/// `stack_phys` the physical base of its stack frame (both provided by
/// the caller from static/linker addresses).
pub fn build_user_address_space(
    as_index: usize,
    code_phys: u64,
    stack_phys: u64,
) -> UserAddressSpace {
    assert!(as_index < MAX_USER_AS, "user AS index out of range");
    let r = kernel_regions();
    // SAFETY: USER_TABLES is a static built once at boot on a single
    // hart before entering user mode; each as_index selects a disjoint
    // arena, so no aliasing reference exists.
    let all: &mut [[Table; ARENA_TABLES]; MAX_USER_AS] =
        unsafe { &mut *core::ptr::addr_of_mut!(USER_TABLES) };
    let base_pa = core::ptr::addr_of!(all[as_index]) as u64;
    let tables: &mut [Table] = &mut all[as_index];
    let mut arena = Arena::new(tables, base_pa);

    // Kernel mappings (U=0) so the trap handler runs post-trap.
    map_kernel_regions(&mut arena, &r);

    // User code: map two pages of the physical code frame at USER_CODE_VA
    // with U + R + X. Two pages cover a function that straddles a page.
    let urx = Permissions {
        read: true,
        write: false,
        execute: true,
        user: true,
        device: false,
    };
    let code_base = code_phys & !0xfff;
    for i in 0..2 {
        arena
            .map_page(
                VirtAddr::new(USER_CODE_VA + i * 0x1000),
                PhysAddr::new(code_base + i * 0x1000),
                urx,
            )
            .expect("map user code");
    }

    // User stack: U + R + W, non-executable (also the execute-fault
    // target for AXIOM-MEMHW-011).
    let urw = Permissions::user_rw();
    let stack_base = stack_phys & !0xfff;
    for i in 0..USER_STACK_PAGES {
        arena
            .map_page(
                VirtAddr::new(USER_STACK_VA + i * 0x1000),
                PhysAddr::new(stack_base + i * 0x1000),
                urw,
            )
            .expect("map user stack");
    }

    UserAddressSpace {
        root: arena.root_pa(),
        entry_va: USER_CODE_VA + (code_phys & 0xfff),
        stack_top_va: USER_STACK_VA + USER_STACK_PAGES * 0x1000,
    }
}

extern "C" {
    static __user_text_start: u8;
    static __user_text_end: u8;
    static __user_rodata_start: u8;
    static __user_rodata_end: u8;
}

/// Virtual span `[USER_CODE_VA, end)` the mapped user region occupies in
/// every service address space (docs/25_OS_BOOT_FLOW.md §2). Used by the
/// syscall layer to validate read-only user buffers that point at
/// sectioned rodata.
pub fn user_region_va_span() -> (u64, u64) {
    // Linker-provided section boundary symbols; address-of only.
    let (start, end) = (
        core::ptr::addr_of!(__user_text_start) as u64,
        core::ptr::addr_of!(__user_rodata_end) as u64,
    );
    (USER_CODE_VA, USER_CODE_VA + (end - start))
}

/// Build a service address space (AXIOM-INIT-002, docs/25 §2): the whole
/// linker-gathered user region mapped contiguously at USER_CODE_VA
/// (text pages U+R+X, rodata pages U+R — W^X preserved) plus a private
/// U+R+W stack page. RISC-V medany code is pc-relative, so intra-region
/// calls and static references work at the mapped address; anything that
/// escapes the region page-faults and is contained.
///
/// `entry_kernel_va` is the link-time address of the sectioned entry
/// function; `stack_phys` the physical base of the service's stack page.
pub fn build_service_address_space(
    as_index: usize,
    entry_kernel_va: u64,
    stack_phys: u64,
) -> UserAddressSpace {
    assert!(as_index < MAX_USER_AS, "user AS index out of range");
    let r = kernel_regions();
    // SAFETY: as in build_user_address_space — disjoint per-index arena,
    // single hart.
    let all: &mut [[Table; ARENA_TABLES]; MAX_USER_AS] =
        unsafe { &mut *core::ptr::addr_of_mut!(USER_TABLES) };
    let base_pa = core::ptr::addr_of!(all[as_index]) as u64;
    let tables: &mut [Table] = &mut all[as_index];
    let mut arena = Arena::new(tables, base_pa);

    map_kernel_regions(&mut arena, &r);

    // Linker section symbols; address-of only.
    let (text_start, text_end, ro_start, ro_end) = (
        core::ptr::addr_of!(__user_text_start) as u64,
        core::ptr::addr_of!(__user_text_end) as u64,
        core::ptr::addr_of!(__user_rodata_start) as u64,
        core::ptr::addr_of!(__user_rodata_end) as u64,
    );

    let urx = Permissions {
        read: true,
        write: false,
        execute: true,
        user: true,
        device: false,
    };
    let uro = Permissions {
        read: true,
        write: false,
        execute: false,
        user: true,
        device: false,
    };
    let mut pa = text_start;
    while pa < text_end {
        arena
            .map_page(
                VirtAddr::new(USER_CODE_VA + (pa - text_start)),
                PhysAddr::new(pa),
                urx,
            )
            .expect("map user region text");
        pa += 0x1000;
    }
    let mut pa = ro_start;
    while pa < ro_end {
        arena
            .map_page(
                VirtAddr::new(USER_CODE_VA + (pa - text_start)),
                PhysAddr::new(pa),
                uro,
            )
            .expect("map user region rodata");
        pa += 0x1000;
    }

    let urw = Permissions::user_rw();
    arena
        .map_page(
            VirtAddr::new(USER_STACK_VA),
            PhysAddr::new(stack_phys & !0xfff),
            urw,
        )
        .expect("map service stack");

    UserAddressSpace {
        root: arena.root_pa(),
        entry_va: USER_CODE_VA + (entry_kernel_va - text_start),
        stack_top_va: USER_STACK_VA + 0x1000,
    }
}

/// Switch satp to a user address space root and flush the TLB
/// (AXIOM-MEMHW-007). Must be called immediately before __enter_user.
///
/// # Safety
/// `root` must be a fully built Sv39 root table that also maps the
/// kernel regions (U=0) so the trap handler remains reachable, and the
/// currently executing code + stack must be identity-mapped in it.
pub unsafe fn switch_to_user_space(root: PhysAddr) {
    let satp = satp_for(root);
    unsafe {
        core::arch::asm!(
            "csrw satp, {satp}",
            "sfence.vma",
            satp = in(reg) satp,
            options(nostack, preserves_flags)
        );
    }
}
