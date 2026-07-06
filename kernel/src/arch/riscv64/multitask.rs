//! Two-task cooperative dispatch demo (AXIOM-SCHEDRT-004/005/007).
//!
//! Requirement reference: docs/13_DISPATCH.md.
//!
//! Builds two U-mode tasks, each in its own Sv39 address space, sharing
//! one position-independent body that yields twice then exits. The
//! kernel dispatcher alternates between them and prints the structured
//! scheduling events. Selected by the `demo_multitask` cargo feature.

use crate::dispatch;
use crate::paging_hw;
use crate::uart;

/// Per-task user stack frame (mapped at the user stack VA of each AS).
#[repr(C, align(4096))]
struct Stack([u8; 4096]);
static mut TASK_STACKS: [Stack; 2] = [Stack([0; 4096]), Stack([0; 4096])];

/// Kernel trap stack for traps taken from user mode.
#[repr(C, align(16))]
struct TrapStack([u8; 8 * 1024]);
static mut MT_TRAP_STACK: TrapStack = TrapStack([0; 8 * 1024]);

/// Shared task body: yield twice, then exit. Position-independent
/// (only immediates + syscalls). `s0` is the yield counter; it survives
/// each yield because the dispatcher saves/restores the full frame.
extern "C" fn task_body() -> ! {
    // SAFETY: pure control flow over the syscall ABI (a7=number). The
    // function never returns (noreturn), so clobbering s0/a7/a0 is
    // sound; every ecall re-enters the kernel through the trap path.
    unsafe {
        core::arch::asm!(
            "li s0, 2",
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

const NAMES: [&str; 2] = ["task_a", "task_b"];

/// Set up and run the two-task cooperative demo.
pub fn multitask_demo() -> ! {
    let code_phys = task_body as *const () as u64;

    for i in 0..2usize {
        // SAFETY: addr_of! forms a raw pointer to a static-mut element
        // without a reference or read; boot-time, single hart.
        let stack_phys = unsafe { core::ptr::addr_of!(TASK_STACKS[i]) as u64 };
        let uas = paging_hw::build_user_address_space(i, code_phys, stack_phys);
        // SAFETY: boot-time, single hart, distinct slot i; the address
        // space maps the kernel (U=0) and the task's code/stack (U=1).
        // Equal priority (1): cooperative round-robin (docs/13 §5).
        unsafe {
            dispatch::register_task(i, NAMES[i], 1, uas.root, uas.entry_va, uas.stack_top_va);
        }
        uart::put_str("TASK_STARTED task=");
        uart::put_str(NAMES[i]);
        uart::put_str("\n");
    }

    // SAFETY: addr_of! forms a raw pointer to a static-mut without a
    // reference or read; both tasks are registered with valid address
    // spaces; trap_stack_top is a valid kernel trap stack top.
    unsafe {
        let trap_stack_top = core::ptr::addr_of!(MT_TRAP_STACK) as u64 + 8 * 1024;
        dispatch::start(trap_stack_top)
    }
}
