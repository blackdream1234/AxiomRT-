//! AxiomRT kernel binary entry (AXIOM-BOOT-001).
//!
//! Phase 2 scope only: bare-metal `no_std` skeleton. No scheduler, no
//! memory manager, no IPC, no capabilities, no heap
//! (docs/02_KERNEL_BLUEPRINT.md §13, Fulltask Phase 2).
//!
//! The kernel binary is riscv64-only. Host builds (which cargo performs
//! while compiling the host-run test suites, docs/14_TEST_STRATEGY.md)
//! compile an empty stub instead — the real entry, assembly, and drivers
//! are all gated on `target_arch = "riscv64"`.

#![cfg_attr(target_arch = "riscv64", no_std)]
#![cfg_attr(target_arch = "riscv64", no_main)]
#![forbid(unsafe_op_in_unsafe_fn)]

// The library provides the panic handler for the bare-metal build.
extern crate kernel;

// Assembly boot entry (AXIOM-BOOT-002): sets the stack, clears .bss,
// then calls kernel_main. See docs/09_BUILD_AND_BOOT.md.
#[cfg(target_arch = "riscv64")]
core::arch::global_asm!(include_str!("arch/riscv64/boot.S"));

// UART serial output for the QEMU boot banner (AXIOM-BOOT-003).
#[cfg(target_arch = "riscv64")]
#[path = "arch/riscv64/uart.rs"]
mod uart;

// Trap entry assembly and handler (AXIOM-TRAP-001).
#[cfg(target_arch = "riscv64")]
core::arch::global_asm!(include_str!("arch/riscv64/trap.S"));
#[cfg(target_arch = "riscv64")]
#[path = "arch/riscv64/trap.rs"]
mod trap;

// Sv39 kernel paging activation (v0.2, AXIOM-MEMHW-004).
#[cfg(target_arch = "riscv64")]
#[path = "arch/riscv64/paging_hw.rs"]
mod paging_hw;

// User task layer (Phase 7). The image model is target-independent and
// unit-tested on the host. Transitional allowance: the model is
// consumed on target by the user-mode transition (AXIOM-USER-002).
#[allow(dead_code, unused_imports)]
mod user;

/// Rust kernel entry, called from the assembly boot entry (`_start` in
/// arch/riscv64/boot.S). OpenSBI convention: a0 = hart id, a1 = device
/// tree blob address.
#[cfg(target_arch = "riscv64")]
#[no_mangle]
pub extern "C" fn kernel_main(_hartid: usize, _dtb: usize) -> ! {
    // Controlled trap entry paths must exist before anything else runs
    // (AXIOM-TRAP-001, docs/10_TRAP_MODEL.md).
    trap::init();

    // Boot banner (AXIOM-BOOT-003 expected output; checked by the boot
    // smoke test, docs/14_TEST_STRATEGY.md).
    uart::put_str("AxiomRT kernel booted\n");
    uart::put_str("arch=riscv64\n");
    uart::put_str("phase=boot\n");

    // v0.2 (AXIOM-MEMHW-004): enable Sv39 with the kernel identity map.
    // After this the MMU translates every kernel access; kernel
    // mappings carry no U bit (docs/12_MMU_SV39.md §4).
    paging_hw::enable_kernel_paging();

    // Phase 7 (AXIOM-USER-002): run the first user task outside kernel
    // privilege. Never returns: the user session ends in the registered
    // kernel continuation (docs/10_USER_MODE.md).
    user::first_user_task_demo()
}

/// Host stub: the kernel binary has no meaning off-target. It exists
/// only so `cargo test` can build the package for the host test suites.
#[cfg(not(target_arch = "riscv64"))]
fn main() {
    eprintln!("AxiomRT kernel binary is riscv64-only; build with the default target.");
}
