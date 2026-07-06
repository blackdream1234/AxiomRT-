//! Watchdog / CPU-exhaustion containment demo (AXIOM-WDOG-008).
//!
//! Requirement reference: docs/16_WATCHDOG.md.
//!
//! `faulty_task` enters an infinite loop and never checks in;
//! `critical_task` yields periodically. Both have equal priority, so
//! preemption alone would not displace the hog — the watchdog is what
//! detects and contains it, after which the critical task runs and the
//! kernel stays alive. Selected by the `demo_watchdog` cargo feature.

use crate::dispatch;
use crate::paging_hw;
use crate::timer;
use crate::uart;

#[repr(C, align(4096))]
struct Stack([u8; 4096]);
static mut WD_STACKS: [Stack; 2] = [Stack([0; 4096]), Stack([0; 4096])];

#[repr(C, align(16))]
struct TrapStack([u8; 8 * 1024]);
static mut WD_TRAP_STACK: TrapStack = TrapStack([0; 8 * 1024]);

/// Faulty task: infinite compute loop, never checks in (never syscalls).
extern "C" fn faulty_body() -> ! {
    // SAFETY: intentional infinite loop; never returns (noreturn).
    unsafe {
        core::arch::asm!("1:", "j 1b", options(noreturn));
    }
}

/// Critical task: yield periodically (each yield is a check-in), then
/// keep yielding to stay alive after the faulty task is contained.
extern "C" fn critical_body() -> ! {
    // SAFETY: pure control flow over the syscall ABI; never returns.
    unsafe {
        core::arch::asm!(
            "1:",
            "li a7, 1", // sys_yield (also a watchdog check-in)
            "ecall",
            "j 1b",
            options(noreturn)
        )
    }
}

/// Set up and run the watchdog demo.
pub fn watchdog_demo() -> ! {
    // Equal priority (5): preemption will not displace the hog; the
    // watchdog must. faulty_task (slot 0) runs first.
    let bodies: [(&str, u8, u64); 2] = [
        ("faulty_task", 5, faulty_body as *const () as u64),
        ("critical_task", 5, critical_body as *const () as u64),
    ];

    for (i, &(name, prio, code_phys)) in bodies.iter().enumerate() {
        // SAFETY: addr_of! forms a raw pointer to a static-mut element;
        // boot-time, single hart.
        let stack_phys = unsafe { core::ptr::addr_of!(WD_STACKS[i]) as u64 };
        let uas = paging_hw::build_user_address_space(i, code_phys, stack_phys);
        // SAFETY: boot-time, single hart, distinct slot i.
        unsafe {
            dispatch::register_task(i, name, prio, uas.root, uas.entry_va, uas.stack_top_va);
        }
        uart::put_str("TASK_STARTED task=");
        uart::put_str(name);
        uart::put_str("\n");
    }

    timer::init();
    timer::arm_next();

    // SAFETY: both tasks registered with valid address spaces; the trap
    // stack is valid. Task 0 (faulty_task) runs first.
    unsafe {
        let trap_stack_top = core::ptr::addr_of!(WD_TRAP_STACK) as u64 + 8 * 1024;
        dispatch::start(trap_stack_top)
    }
}
