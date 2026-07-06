//! On-target capability enforcement demo (AXIOM-CAPRT-008).
//!
//! Requirement reference: docs/18_CAP_ONTARGET.md.
//!
//! Three U-mode tasks share one endpoint (id 1): `receiver` holds a
//! Receive capability, `good_sender` holds a Send capability, and
//! `faulty_task` holds no capability. The faulty task's send is denied
//! (endpoint untouched); the capable send then delivers. Selected by
//! the `demo_cap` cargo feature.

use crate::dispatch;
use crate::paging_hw;
use crate::uart;

#[repr(C, align(4096))]
struct Stack([u8; 4096]);
static mut CAP_STACKS: [Stack; 3] = [Stack([0; 4096]), Stack([0; 4096]), Stack([0; 4096])];

#[repr(C, align(16))]
struct TrapStack([u8; 8 * 1024]);
static mut CAP_TRAP_STACK: TrapStack = TrapStack([0; 8 * 1024]);

const ENDPOINT_ID: u32 = 1;
const RIGHT_SEND: u16 = 1 << 3;
const RIGHT_RECV: u16 = 1 << 4;

/// Receiver: sys_recv with the Receive capability (index 0), then exit.
extern "C" fn receiver_body() -> ! {
    // SAFETY: syscall control flow; never returns.
    unsafe {
        core::arch::asm!(
            "li a0, 0",
            "li a1, 0x200040",
            "li a2, 64",
            "li a7, 4",
            "ecall", // recv
            "li a7, 2",
            "ecall", // exit
            "1:",
            "j 1b",
            options(noreturn)
        )
    }
}

/// Faulty task: attempt sys_send with capability index 0, which is
/// empty in its table — the kernel must deny it. Then exit.
extern "C" fn faulty_body() -> ! {
    // SAFETY: the send is denied (no capability); never returns.
    unsafe {
        core::arch::asm!(
            "li a0, 0",
            "li a1, 0x200040",
            "li a2, 4",
            "li a7, 3",
            "ecall", // send (denied)
            "li a7, 2",
            "ecall", // exit
            "1:",
            "j 1b",
            options(noreturn)
        )
    }
}

/// Good sender: write "PING", sys_send with the Send capability, exit.
extern "C" fn good_sender_body() -> ! {
    // SAFETY: writes its own stack page, then the syscall ABI; noreturn.
    unsafe {
        core::arch::asm!(
            "li t0, 0x200040",
            "li t1, 0x50",
            "sb t1, 0(t0)",
            "li t1, 0x49",
            "sb t1, 1(t0)",
            "li t1, 0x4e",
            "sb t1, 2(t0)",
            "li t1, 0x47",
            "sb t1, 3(t0)",
            "li a0, 0",
            "li a1, 0x200040",
            "li a2, 4",
            "li a7, 3",
            "ecall", // send
            "li a7, 2",
            "ecall", // exit
            "1:",
            "j 1b",
            options(noreturn)
        )
    }
}

/// Set up and run the capability enforcement demo.
pub fn cap_demo() -> ! {
    // Slot 0 receiver (Receive cap), slot 1 faulty (no cap), slot 2
    // good_sender (Send cap). Receiver runs first and blocks; faulty is
    // denied; good_sender delivers.
    let bodies: [(&str, u64); 3] = [
        ("receiver", receiver_body as *const () as u64),
        ("faulty_task", faulty_body as *const () as u64),
        ("good_sender", good_sender_body as *const () as u64),
    ];

    for (i, &(name, code_phys)) in bodies.iter().enumerate() {
        // SAFETY: addr_of! forms a raw pointer to a static-mut element.
        let stack_phys = unsafe { core::ptr::addr_of!(CAP_STACKS[i]) as u64 };
        let uas = paging_hw::build_user_address_space(i, code_phys, stack_phys);
        // SAFETY: boot-time, single hart, distinct slot i.
        unsafe {
            dispatch::register_task(i, name, 3, uas.root, uas.entry_va, uas.stack_top_va);
        }
        uart::put_str("TASK_STARTED task=");
        uart::put_str(name);
        uart::put_str("\n");
    }

    // Mint capabilities: receiver=Receive, good_sender=Send; faulty
    // gets none (deny-by-default).
    // SAFETY: boot-time capability minting, single hart, distinct slots.
    unsafe {
        dispatch::set_endpoint_cap(0, 0, ENDPOINT_ID, RIGHT_RECV);
        dispatch::set_endpoint_cap(2, 0, ENDPOINT_ID, RIGHT_SEND);
    }

    // SAFETY: all tasks registered with valid address spaces; trap
    // stack valid. Task 0 (receiver) runs first.
    unsafe {
        let trap_stack_top = core::ptr::addr_of!(CAP_TRAP_STACK) as u64 + 8 * 1024;
        dispatch::start(trap_stack_top)
    }
}
