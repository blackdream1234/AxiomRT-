//! On-target IPC rendezvous demo (AXIOM-IPCRT-010).
//!
//! Requirement reference: docs/17_IPC_ONTARGET.md.
//!
//! `receiver` calls `sys_recv` first (blocks), then `sender` writes a
//! 4-byte message ("PING") into its user stack buffer and calls
//! `sys_send`. The kernel copies the message across the two address
//! spaces via a bounded kernel buffer and completes delivery when the
//! receiver next runs. Selected by the `demo_ipc` cargo feature.

use crate::dispatch;
use crate::paging_hw;
use crate::uart;

#[repr(C, align(4096))]
struct Stack([u8; 4096]);
static mut IPC_STACKS: [Stack; 2] = [Stack([0; 4096]), Stack([0; 4096])];

#[repr(C, align(16))]
struct TrapStack([u8; 8 * 1024]);
static mut IPC_TRAP_STACK: TrapStack = TrapStack([0; 8 * 1024]);

/// Receiver: sys_recv into a user buffer at 0x20_0040 (its own stack
/// page), then exit. Blocks until the sender arrives.
extern "C" fn receiver_body() -> ! {
    // SAFETY: pure syscall control flow; never returns.
    unsafe {
        core::arch::asm!(
            "li a1, 0x200040", // buffer VA (in this task's stack page)
            "li a2, 64",       // capacity
            "li a7, 4",        // sys_recv
            "ecall",
            "li a7, 2",        // sys_exit
            "ecall",
            "1:",
            "j 1b",
            options(noreturn)
        )
    }
}

/// Sender: write "PING" into a user buffer at 0x20_0040 (its own stack
/// page), sys_send 4 bytes, then exit.
extern "C" fn sender_body() -> ! {
    // SAFETY: writes only to its own mapped user stack page, then uses
    // the syscall ABI; never returns.
    unsafe {
        core::arch::asm!(
            "li t0, 0x200040",
            "li t1, 0x50", "sb t1, 0(t0)", // 'P'
            "li t1, 0x49", "sb t1, 1(t0)", // 'I'
            "li t1, 0x4e", "sb t1, 2(t0)", // 'N'
            "li t1, 0x47", "sb t1, 3(t0)", // 'G'
            "li a1, 0x200040", // buffer VA
            "li a2, 4",        // length
            "li a7, 3",        // sys_send
            "ecall",
            "li a7, 2",        // sys_exit
            "ecall",
            "1:",
            "j 1b",
            options(noreturn)
        )
    }
}

/// Set up and run the IPC rendezvous demo (receiver first).
pub fn ipc_demo() -> ! {
    // Slot 0 = receiver (runs first, blocks), slot 1 = sender. Equal
    // priority so the sender runs after the receiver blocks.
    let bodies: [(&str, u64); 2] = [
        ("receiver", receiver_body as *const () as u64),
        ("sender", sender_body as *const () as u64),
    ];

    for (i, &(name, code_phys)) in bodies.iter().enumerate() {
        // SAFETY: addr_of! forms a raw pointer to a static-mut element.
        let stack_phys = unsafe { core::ptr::addr_of!(IPC_STACKS[i]) as u64 };
        let uas = paging_hw::build_user_address_space(i, code_phys, stack_phys);
        // SAFETY: boot-time, single hart, distinct slot i.
        unsafe {
            dispatch::register_task(i, name, 3, uas.root, uas.entry_va, uas.stack_top_va);
        }
        uart::put_str("TASK_STARTED task=");
        uart::put_str(name);
        uart::put_str("\n");
    }

    // SAFETY: both tasks registered with valid address spaces; the trap
    // stack is valid. Task 0 (receiver) runs first.
    unsafe {
        let trap_stack_top = core::ptr::addr_of!(IPC_TRAP_STACK) as u64 + 8 * 1024;
        dispatch::start(trap_stack_top)
    }
}
