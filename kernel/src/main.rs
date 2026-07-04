//! AxiomRT kernel binary entry (AXIOM-BOOT-001).
//!
//! Phase 2 scope only: bare-metal `no_std` skeleton. No scheduler, no
//! memory manager, no IPC, no capabilities, no heap
//! (docs/02_KERNEL_BLUEPRINT.md §13, Fulltask Phase 2).

#![no_std]
#![no_main]
#![forbid(unsafe_op_in_unsafe_fn)]

// The library provides the panic handler for the bare-metal build.
extern crate kernel;

// Assembly boot entry (AXIOM-BOOT-002): sets the stack, clears .bss,
// then calls kernel_main. See docs/09_BUILD_AND_BOOT.md.
core::arch::global_asm!(include_str!("arch/riscv64/boot.S"));

// UART serial output for the QEMU boot banner (AXIOM-BOOT-003).
#[path = "arch/riscv64/uart.rs"]
mod uart;

/// Rust kernel entry, called from the assembly boot entry (`_start` in
/// arch/riscv64/boot.S). OpenSBI convention: a0 = hart id, a1 = device
/// tree blob address.
#[no_mangle]
pub extern "C" fn kernel_main(_hartid: usize, _dtb: usize) -> ! {
    // Boot banner (AXIOM-BOOT-003 expected output; checked by the boot
    // smoke test, docs/14_TEST_STRATEGY.md).
    uart::put_str("AxiomRT kernel booted\n");
    uart::put_str("arch=riscv64\n");
    uart::put_str("phase=boot\n");

    // Phase 2: no scheduler is started. Halt loop only
    // (AXIOM-BOOT-002 expected behavior).
    loop {
        core::hint::spin_loop();
    }
}
