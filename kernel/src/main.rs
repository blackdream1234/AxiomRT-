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

// On-target cooperative task dispatcher (v0.3, AXIOM-SCHEDRT-001..006).
// Always compiled on target (the trap layer calls it); inert until
// tasks are registered by a demo.
#[cfg(target_arch = "riscv64")]
#[path = "arch/riscv64/dispatch.rs"]
mod dispatch;

// Supervisor timer and preemption tick (v0.4, AXIOM-TIMER-002..007).
// Always compiled on target (the trap layer routes timer interrupts
// here); dormant until enabled by the preemption demo.
#[cfg(target_arch = "riscv64")]
#[path = "arch/riscv64/timer.rs"]
mod timer;

// Two-task cooperative demo (v0.3, feature-gated so the default build
// keeps the v0.2 memory-isolation demo and its tests).
#[cfg(all(target_arch = "riscv64", feature = "demo_multitask"))]
#[path = "arch/riscv64/multitask.rs"]
mod multitask;

// Timer preemption demo (v0.4, feature-gated).
#[cfg(all(target_arch = "riscv64", feature = "demo_preempt"))]
#[path = "arch/riscv64/preempt.rs"]
mod preempt;

// Watchdog / CPU-exhaustion demo (v0.5, feature-gated).
#[cfg(all(target_arch = "riscv64", feature = "demo_watchdog"))]
#[path = "arch/riscv64/watchdog_demo.rs"]
mod watchdog_demo;

// On-target IPC rendezvous demo (v0.6, feature-gated).
#[cfg(all(target_arch = "riscv64", feature = "demo_ipc"))]
#[path = "arch/riscv64/ipc_demo.rs"]
mod ipc_demo;

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

    // Select the on-target demo (none returns): IPC (v0.6), watchdog
    // (v0.5), timer preemption (v0.4), two-task cooperative dispatch
    // (v0.3), or the v0.2 single-task memory-isolation demo by default.
    // At most one demo_* feature is expected to be set at a time.
    #[cfg(feature = "demo_ipc")]
    {
        ipc_demo::ipc_demo()
    }
    #[cfg(all(feature = "demo_watchdog", not(feature = "demo_ipc")))]
    {
        watchdog_demo::watchdog_demo()
    }
    #[cfg(all(
        feature = "demo_preempt",
        not(feature = "demo_watchdog"),
        not(feature = "demo_ipc")
    ))]
    {
        preempt::preempt_demo()
    }
    #[cfg(all(
        feature = "demo_multitask",
        not(feature = "demo_preempt"),
        not(feature = "demo_watchdog"),
        not(feature = "demo_ipc")
    ))]
    {
        multitask::multitask_demo()
    }
    #[cfg(not(any(
        feature = "demo_multitask",
        feature = "demo_preempt",
        feature = "demo_watchdog",
        feature = "demo_ipc"
    )))]
    {
        user::first_user_task_demo()
    }
}

/// Host stub: the kernel binary has no meaning off-target. It exists
/// only so `cargo test` can build the package for the host test suites.
#[cfg(not(target_arch = "riscv64"))]
fn main() {
    eprintln!("AxiomRT kernel binary is riscv64-only; build with the default target.");
}
