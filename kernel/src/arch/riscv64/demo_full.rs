//! Full four-task fault-containment demo (AXIOM-DEMO-002).
//!
//! Requirement reference: docs/20_FULL_DEMO.md, docs/00_PROJECT_CHARTER.md §7.
//!
//! critical_task, supervisor_task, logger_task, faulty_task. The faulty
//! task attacks with an illegal (capability-less) IPC and then CPU
//! exhaustion; the kernel denies the IPC and the watchdog contains the
//! loop, the supervisor applies recovery, the logger records evidence,
//! and the critical task keeps running. Selected by the `demo_full`
//! cargo feature.

use crate::dispatch;
use crate::paging_hw;
use crate::timer;
use crate::uart;

#[repr(C, align(4096))]
struct Stack([u8; 4096]);
static mut FULL_STACKS: [Stack; 4] = [
    Stack([0; 4096]),
    Stack([0; 4096]),
    Stack([0; 4096]),
    Stack([0; 4096]),
];

#[repr(C, align(16))]
struct TrapStack([u8; 8 * 1024]);
static mut FULL_TRAP_STACK: TrapStack = TrapStack([0; 8 * 1024]);

const EP_FAULT: u32 = 2;
const EP_EVENT: u32 = 3;
const RIGHT_RECV: u16 = 1 << 4;
const RIGHT_CONTROL: u16 = 1 << 7;

/// Supervisor: recv fault event, acknowledge Kill, repeat.
extern "C" fn supervisor_body() -> ! {
    // SAFETY: syscall control flow only; never returns.
    unsafe {
        core::arch::asm!(
            "1:",
            "li a0, 0",
            "li a1, 0x200040",
            "li a2, 64",
            "li a7, 4",
            "ecall",
            "li a1, 2",
            "li a7, 7",
            "ecall", // fault_ack decision=Kill
            "j 1b",
            options(noreturn)
        )
    }
}

/// Logger: recv monitoring event, repeat.
extern "C" fn logger_body() -> ! {
    // SAFETY: syscall control flow only; never returns.
    unsafe {
        core::arch::asm!(
            "1:",
            "li a0, 0",
            "li a1, 0x200040",
            "li a2, 64",
            "li a7, 4",
            "ecall",
            "j 1b",
            options(noreturn)
        )
    }
}

/// Faulty: attempt an illegal (capability-less) IPC send — denied — then
/// enter an infinite loop (CPU exhaustion) that the watchdog contains.
extern "C" fn faulty_body() -> ! {
    // SAFETY: the send is denied (no capability), then an intentional
    // infinite loop; never returns.
    unsafe {
        core::arch::asm!(
            "li a0, 0",
            "li a1, 0x200040",
            "li a2, 4",
            "li a7, 3",
            "ecall", // illegal IPC
            "1:",
            "j 1b", // CPU exhaustion
            options(noreturn)
        )
    }
}

/// Critical: yield forever (each schedule proves it is alive).
extern "C" fn critical_body() -> ! {
    // SAFETY: syscall control flow only; never returns.
    unsafe {
        core::arch::asm!(
            "1:",
            "li a7, 1",
            "ecall",
            "j 1b", // sys_yield loop
            options(noreturn)
        )
    }
}

/// Set up and run the full four-task demo.
pub fn demo_full() -> ! {
    // Priorities: supervisor 7 > logger 6 > faulty 5 > critical 4. Entry
    // order lets supervisor and logger block on recv before the faulty
    // task hogs the CPU; after the faulty task is contained, only the
    // critical task remains Ready and runs on.
    let bodies: [(&str, u8, u64); 4] = [
        ("supervisor_task", 7, supervisor_body as *const () as u64),
        ("logger_task", 6, logger_body as *const () as u64),
        ("faulty_task", 5, faulty_body as *const () as u64),
        ("critical_task", 4, critical_body as *const () as u64),
    ];

    for (i, &(name, prio, code_phys)) in bodies.iter().enumerate() {
        // SAFETY: addr_of! forms a raw pointer to a static-mut element.
        let stack_phys = unsafe { core::ptr::addr_of!(FULL_STACKS[i]) as u64 };
        let uas = paging_hw::build_user_address_space(i, code_phys, stack_phys);
        // SAFETY: boot-time, single hart, distinct slot i.
        unsafe {
            dispatch::register_task(i, name, prio, uas.root, uas.entry_va, uas.stack_top_va);
        }
        uart::put_str("TASK_STARTED task=");
        uart::put_str(name);
        uart::put_str("\n");
    }

    // Capabilities: supervisor=Receive+Control on fault channel,
    // logger=Receive on event channel. faulty and critical get none.
    // SAFETY: boot-time capability minting, single hart, distinct slots.
    unsafe {
        dispatch::set_endpoint_cap(0, 0, EP_FAULT, RIGHT_RECV | RIGHT_CONTROL);
        dispatch::set_endpoint_cap(1, 0, EP_EVENT, RIGHT_RECV);
    }

    timer::init();
    timer::arm_next();

    // SAFETY: all tasks registered with valid address spaces; trap stack
    // valid. Task 0 (supervisor) runs first and blocks on recv.
    unsafe {
        let trap_stack_top = core::ptr::addr_of!(FULL_TRAP_STACK) as u64 + 8 * 1024;
        dispatch::start(trap_stack_top)
    }
}
