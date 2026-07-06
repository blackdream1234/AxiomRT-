//! Sv39 activation on target (AXIOM-MEMHW-004).
//!
//! Requirement reference: docs/12_MMU_SV39.md §4.
//!
//! Builds the kernel identity map from the linker section symbols
//! (correct per-region permissions), writes `satp`, and issues
//! `sfence.vma`. riscv64-only: the static table arena and CSR access
//! have no host meaning.

use kernel::memory::pagetable::Permissions;
use kernel::memory::paging::{Arena, Table, ARENA_TABLES};
use kernel::memory::sv39::satp_for;

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

/// Static table arena, 4 KiB aligned (Table is align(4096)). Lives in
/// .bss, therefore inside the R+W data span it maps for itself.
static mut KERNEL_TABLES: [Table; ARENA_TABLES] = [Table::zeroed(); ARENA_TABLES];

fn sym(addr: &'static u8) -> u64 {
    // addr_of! obtains the linker symbol address without forming a
    // reference to the (zero-sized) location.
    core::ptr::addr_of!(*addr) as u64
}

/// Build the kernel page table and enable Sv39. Returns after the MMU
/// is active; all subsequent kernel accesses are translated.
pub fn enable_kernel_paging() {
    let text_start = sym(unsafe { &__text_start });
    let text_end = sym(unsafe { &__text_end });
    let rodata_start = sym(unsafe { &__rodata_start });
    let rodata_end = sym(unsafe { &__rodata_end });
    let data_start = sym(unsafe { &__data_start });
    let data_end = sym(unsafe { &__data_end });

    // SAFETY (docs/07_CODEX_RULES.md §6): KERNEL_TABLES is a static
    // accessed once, before paging is enabled, from single-hart boot
    // context; no other reference exists. addr_of_mut! avoids forming a
    // reference to the large array.
    let base_pa = core::ptr::addr_of!(KERNEL_TABLES) as u64;
    let tables: &mut [Table] =
        unsafe { &mut *core::ptr::addr_of_mut!(KERNEL_TABLES) };
    let mut arena = Arena::new(tables, base_pa);

    let krx = Permissions { read: true, write: false, execute: true, user: false, device: false };
    let kr_only = Permissions { read: true, write: false, execute: false, user: false, device: false };
    let krw = Permissions::kernel_rw();
    let kdev = Permissions::kernel_device();

    // Kernel text: R + X.
    arena
        .identity_map_range(text_start, text_end, krx)
        .expect("map kernel text");
    // Kernel rodata: R only.
    if rodata_end > rodata_start {
        arena
            .identity_map_range(rodata_start, rodata_end, kr_only)
            .expect("map kernel rodata");
    }
    // Kernel data + bss + stack (one R + W span, includes the page
    // tables themselves): R + W.
    arena
        .identity_map_range(data_start, data_end, krw)
        .expect("map kernel data");
    // UART MMIO: R + W device, kernel-only, never executable.
    arena
        .identity_map_range(UART_PAGE, UART_PAGE + 0x1000, kdev)
        .expect("map uart");

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
