//! Timer preemption demo (AXIOM-TIMER-008).
//!
//! Requirement reference: docs/15_TIMER_PREEMPTION.md.
//!
//! Two U-mode tasks: `low_loop` (priority 0, an infinite loop that never
//! yields) and `critical_task` (priority 7). The timer preempts the
//! loop and runs the critical task, proving a low-priority infinite loop
//! cannot freeze the kernel or starve a high-priority task. Selected by
//! the `demo_preempt` cargo feature.

use crate::dispatch;
use crate::paging_hw;
use crate::timer;
use crate::uart;

#[repr(C, align(4096))]
struct Stack([u8; 4096]);
static mut PRE_STACKS: [Stack; 2] = [Stack([0; 4096]), Stack([0; 4096])];

#[repr(C, align(16))]
struct TrapStack([u8; 8 * 1024]);
static mut PRE_TRAP_STACK: TrapStack = TrapStack([0; 8 * 1024]);

/// Low-priority task: spin forever, never syscall. Only the timer can
/// take the CPU back from it.
extern "C" fn low_loop_body() -> ! {
    // SAFETY: an intentional infinite loop; never returns (noreturn).
    unsafe {
        core::arch::asm!("1:", "j 1b", options(noreturn));
    }
}

/// High-priority task: yield a few times (each proves it is alive and
/// scheduled), then exit.
extern "C" fn critical_body() -> ! {
    // SAFETY: pure control flow over the syscall ABI; never returns.
    unsafe {
        core::arch::asm!(
            "li s0, 3",
            "2:",
            "beqz s0, 3f",
            "li a7, 1", // sys_yield
            "ecall",
            "addi s0, s0, -1",
            "j 2b",
            "3:",
            "li a7, 2", // sys_exit
            "ecall",
            "4:",
            "j 4b",
            options(noreturn)
        )
    }
}

/// Set up and run the preemption demo.
pub fn preempt_demo() -> ! {
    // Task 0 = low_loop (priority 0), task 1 = critical_task (priority 7).
    let bodies: [(&str, u8, u64); 2] = [
        ("low_loop", 0, low_loop_body as *const () as u64),
        ("critical_task", 7, critical_body as *const () as u64),
    ];

    for (i, &(name, prio, code_phys)) in bodies.iter().enumerate() {
        // SAFETY: addr_of! forms a raw pointer to a static-mut element;
        // boot-time, single hart.
        let stack_phys = unsafe { core::ptr::addr_of!(PRE_STACKS[i]) as u64 };
        let uas = paging_hw::build_user_address_space(i, code_phys, stack_phys);
        // SAFETY: boot-time, single hart, distinct slot i.
        unsafe {
            dispatch::register_task(i, name, prio, uas.root, uas.entry_va, uas.stack_top_va);
        }
        uart::put_str("TASK_STARTED task=");
        uart::put_str(name);
        uart::put_str("\n");
    }

    // Enable and arm the preemption timer before entering user mode.
    timer::init();
    timer::arm_next();

    // SAFETY: both tasks registered with valid address spaces; the trap
    // stack is a valid kernel stack. Task 0 (low_loop) runs first.
    unsafe {
        let trap_stack_top = core::ptr::addr_of!(PRE_TRAP_STACK) as u64 + 8 * 1024;
        dispatch::start(trap_stack_top)
    }
}
