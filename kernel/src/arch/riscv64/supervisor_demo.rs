//! On-target supervisor + logger fault-recovery demo (AXIOM-SUPRT-008).
//!
//! Requirement reference: docs/19_SUPERVISOR_ONTARGET.md.
//!
//! `supervisor_task` (Receive+Control on the fault channel, id 2) and
//! `logger_task` (Receive on the event channel, id 3) block on their
//! channels; `faulty_task` runs an infinite loop. The watchdog contains
//! the loop and the kernel notifies the supervisor and logger; the
//! supervisor acknowledges with a Kill decision. Selected by the
//! `demo_supervisor` cargo feature.

use crate::dispatch;
use crate::paging_hw;
use crate::timer;
use crate::uart;

#[repr(C, align(4096))]
struct Stack([u8; 4096]);
static mut SUP_STACKS: [Stack; 3] = [Stack([0; 4096]), Stack([0; 4096]), Stack([0; 4096])];

#[repr(C, align(16))]
struct TrapStack([u8; 8 * 1024]);
static mut SUP_TRAP_STACK: TrapStack = TrapStack([0; 8 * 1024]);

const EP_FAULT: u32 = 2;
const EP_EVENT: u32 = 3;
const RIGHT_RECV: u16 = 1 << 4;
const RIGHT_CONTROL: u16 = 1 << 7;

/// Supervisor: recv a fault event on the fault channel (cap index 0),
/// then acknowledge with a Kill decision (sys_fault_ack, a1=2), loop to
/// wait for the next event.
extern "C" fn supervisor_body() -> ! {
    // SAFETY: syscall control flow only; never returns.
    unsafe {
        core::arch::asm!(
            "1:",
            "li a0, 0", "li a1, 0x200040", "li a2, 64", "li a7, 4", "ecall", // recv fault
            "li a1, 2", "li a7, 7", "ecall", // sys_fault_ack decision=Kill
            "j 1b",
            options(noreturn)
        )
    }
}

/// Logger: recv a monitoring event on the event channel (cap index 0),
/// loop to wait for the next event.
extern "C" fn logger_body() -> ! {
    // SAFETY: syscall control flow only; never returns.
    unsafe {
        core::arch::asm!(
            "1:",
            "li a0, 0", "li a1, 0x200040", "li a2, 64", "li a7, 4", "ecall", // recv event
            "j 1b",
            options(noreturn)
        )
    }
}

/// Faulty task: infinite compute loop, never checks in.
extern "C" fn faulty_body() -> ! {
    // SAFETY: intentional infinite loop; never returns.
    unsafe {
        core::arch::asm!("1:", "j 1b", options(noreturn));
    }
}

/// Set up and run the supervisor/logger demo.
pub fn supervisor_demo() -> ! {
    // Slot 0 supervisor (prio 6), slot 1 logger (prio 5), slot 2 faulty
    // (prio 4). Supervisor and logger run first and block on recv; the
    // faulty task then runs and is caught by the watchdog.
    let bodies: [(&str, u8, u64); 3] = [
        ("supervisor_task", 6, supervisor_body as *const () as u64),
        ("logger_task", 5, logger_body as *const () as u64),
        ("faulty_task", 4, faulty_body as *const () as u64),
    ];

    for (i, &(name, prio, code_phys)) in bodies.iter().enumerate() {
        // SAFETY: addr_of! forms a raw pointer to a static-mut element.
        let stack_phys = unsafe { core::ptr::addr_of!(SUP_STACKS[i]) as u64 };
        let uas = paging_hw::build_user_address_space(i, code_phys, stack_phys);
        // SAFETY: boot-time, single hart, distinct slot i.
        unsafe {
            dispatch::register_task(i, name, prio, uas.root, uas.entry_va, uas.stack_top_va);
        }
        uart::put_str("TASK_STARTED task=");
        uart::put_str(name);
        uart::put_str("\n");
    }

    // Mint channel capabilities: supervisor gets Receive+Control on the
    // fault channel; logger gets Receive on the event channel.
    // SAFETY: boot-time capability minting, single hart, distinct slots.
    unsafe {
        dispatch::set_endpoint_cap(0, 0, EP_FAULT, RIGHT_RECV | RIGHT_CONTROL);
        dispatch::set_endpoint_cap(1, 0, EP_EVENT, RIGHT_RECV);
    }

    // Enable the preemption/watchdog timer before entering user mode.
    timer::init();
    timer::arm_next();

    // SAFETY: all tasks registered with valid address spaces; trap stack
    // valid. Task 0 (supervisor) runs first and blocks on recv.
    unsafe {
        let trap_stack_top = core::ptr::addr_of!(SUP_TRAP_STACK) as u64 + 8 * 1024;
        dispatch::start(trap_stack_top)
    }
}
