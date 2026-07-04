//! Kernel panic handler (AXIOM-BOOT-001).
//!
//! A panic in kernel context is a KernelInvariantViolation
//! (docs/06_FAULT_MODEL.md): the only safe action is a controlled halt.
//! Phase 2 scope: halt only. Structured panic reporting over serial is
//! added with the trap/monitoring phases.

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    // Controlled halt: never continue on a broken kernel invariant,
    // never reboot silently (docs/06_FAULT_MODEL.md, KernelPanic).
    loop {
        core::hint::spin_loop();
    }
}
