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

/// Rust kernel entry, called from the assembly boot entry
/// (added by AXIOM-BOOT-002). OpenSBI convention: a0 = hart id,
/// a1 = device tree blob address.
#[no_mangle]
pub extern "C" fn kernel_main(_hartid: usize, _dtb: usize) -> ! {
    // Phase 2: no scheduler. Halt loop only.
    loop {
        core::hint::spin_loop();
    }
}
