//! Live IPC payload-ownership regression (AXIOM-FOUND-001).
//!
//! Requirement reference: docs/17_IPC_ONTARGET.md §2/§3/§5,
//! docs/08_IPC_MODEL.md §3, docs/36_ROBUSTNESS_AND_FUZZING.md §5.3.
//!
//! Drives the REAL dispatch path — every step is an `ecall` through
//! `trap_handler` into `dispatch::on_syscall`. The host `kernel::ipc`
//! model is a separate implementation and is not exercised here.
//!
//! Twelve tasks reproduce both payload-corruption sequences plus the
//! bounded length, short-receive and endpoint-reuse checks:
//!
//! * seq1 cross-endpoint: A parks 64 bytes on X, B parks 16 bytes on Y,
//!   receivers must get exactly their own sender's bytes;
//! * seq2 busy: C parks 128 bytes on Z, D's send on Z is rejected busy,
//!   the receiver must still get C's 128 bytes;
//! * seq3 zero length and oversized rejection;
//! * seq4 short receive then adequately sized retry on V;
//! * seq5 endpoint reuse after the parked sender is killed.
//!
//! Determinism comes from fixed priorities plus blocking: a send on an
//! Idle endpoint blocks, so `select_highest` must pick the next-lower
//! priority Ready task. No sleeps and no tick counting. Every task
//! exits after its final action, so no participant can starve a later
//! one, and every check reports pass/fail without looping, so a
//! baseline mismatch in one sequence never prevents the next.
//!
//! U-mode bodies are pure inline assembly and build every reported tag
//! on their own stack page with immediate stores. Nothing references
//! `.rodata`, so the LLVM lookup-table escape of docs/25 §2 cannot
//! occur here. Selected by the `demo_ipc_payload` cargo feature.

use crate::dispatch;
use crate::paging_hw;
use crate::uart;

const TASKS: usize = 12;

#[repr(C, align(4096))]
struct Stack([u8; 4096]);
static mut IPCOWN_STACKS: [Stack; TASKS] = [const { Stack([0; 4096]) }; TASKS];

#[repr(C, align(16))]
struct TrapStack([u8; 8 * 1024]);
static mut IPCOWN_TRAP_STACK: TrapStack = TrapStack([0; 8 * 1024]);

// Endpoint ids used by the scenarios.
const EP_X: u32 = 0;
const EP_Y: u32 = 1;
const EP_Z: u32 = 2;
const EP_W: u32 = 3;
const EP_V: u32 = 4;
const EP_U: u32 = 5;

const RIGHT_SEND: u16 = 1 << 3;
const RIGHT_RECV: u16 = 1 << 4;

// Every body writes its payload at 0x20_0040 and its 8-byte report tag
// at 0x20_0100, both inside its own mapped stack page.
//
// Tags are emitted as two little-endian words, so a report costs four
// instructions and needs no read-only data. Tag text, LE word pairs:
//   "IPC1RXP\n" 0x31435049 0x0A505852   "IPC1RXF\n" 0x31435049 0x0A465852
//   "IPC1RYP\n" 0x31435049 0x0A505952   "IPC1RYF\n" 0x31435049 0x0A465952
//   "IPC2BSY\n" 0x32435049 0x0A595342   "IPC2BSF\n" 0x32435049 0x0A465342
//   "IPC2RZP\n" 0x32435049 0x0A505A52   "IPC2RZF\n" 0x32435049 0x0A465A52
//   "IPC3ZLP\n" 0x33435049 0x0A504C5A   "IPC3ZLF\n" 0x33435049 0x0A464C5A
//   "IPC3OVP\n" 0x33435049 0x0A50564F   "IPC3OVF\n" 0x33435049 0x0A46564F
//   "IPC4SHP\n" 0x34435049 0x0A504853   "IPC4SHF\n" 0x34435049 0x0A464853
//   "IPC4RTP\n" 0x34435049 0x0A505452   "IPC4RTF\n" 0x34435049 0x0A465452
//   "IPC5RUP\n" 0x35435049 0x0A505552   "IPC5RUF\n" 0x35435049 0x0A465552
//   "IPCDONE\n" 0x44435049 0x0A454E4F

/// T0 senderA (prio 12): park 64 bytes of 0xA1 on X. When the send
/// returns its message has been taken, so it exits.
#[no_mangle]
extern "C" fn ipcown_sender_a() -> ! {
    // SAFETY: writes only inside this task's mapped stack page, then
    // uses the syscall ABI. Never returns.
    unsafe {
        core::arch::asm!(
            // fill 64 bytes with 0xA1
            "li t0, 0x200040",
            "li t1, 64",
            "li t2, 0xA1",
            "1:",
            "sb t2, 0(t0)",
            "addi t0, t0, 1",
            "addi t1, t1, -1",
            "bnez t1, 1b",
            // sys_send(cap 1, buf, 64)
            "li a0, 1",
            "li a1, 0x200040",
            "li a2, 64",
            "li a7, 3",
            "ecall",
            // released: exit
            "li a7, 2",
            "ecall",
            "2:",
            "j 2b",
            options(noreturn)
        )
    }
}

/// T1 senderB (prio 11): park 16 bytes of 0xB2 on Y, then exit.
#[no_mangle]
extern "C" fn ipcown_sender_b() -> ! {
    // SAFETY: as ipcown_sender_a.
    unsafe {
        core::arch::asm!(
            "li t0, 0x200040",
            "li t1, 16",
            "li t2, 0xB2",
            "1:",
            "sb t2, 0(t0)",
            "addi t0, t0, 1",
            "addi t1, t1, -1",
            "bnez t1, 1b",
            "li a0, 1",
            "li a1, 0x200040",
            "li a2, 16",
            "li a7, 3",
            "ecall",
            "li a7, 2",
            "ecall",
            "2:",
            "j 2b",
            options(noreturn)
        )
    }
}

/// T2 recvX (prio 10): receive from X, require exactly 64 bytes of
/// 0xA1, report, then perform the zero-length receive on W and require
/// a returned length of 0. Exits either way.
#[no_mangle]
extern "C" fn ipcown_recv_x() -> ! {
    // SAFETY: as ipcown_sender_a.
    unsafe {
        core::arch::asm!(
            // sys_recv(cap 1 = X, buf, 128)
            "li a0, 1",
            "li a1, 0x200040",
            "li a2, 128",
            "li a7, 4",
            "ecall",
            "li t3, 64",
            "bne a0, t3, 8f", // wrong length -> FAIL
            "li t0, 0x200040",
            "li t1, 64",
            "li t2, 0xA1",
            "1:",
            "lbu t4, 0(t0)",
            "bne t4, t2, 8f", // wrong byte -> FAIL
            "addi t0, t0, 1",
            "addi t1, t1, -1",
            "bnez t1, 1b",
            // pass tag "IPC1RXP\n"
            "li t0, 0x200100",
            "li t1, 0x31435049",
            "sw t1, 0(t0)",
            "li t1, 0x0A505852",
            "sw t1, 4(t0)",
            "j 9f",
            "8:",
            // fail tag "IPC1RXF\n"
            "li t0, 0x200100",
            "li t1, 0x31435049",
            "sw t1, 0(t0)",
            "li t1, 0x0A465852",
            "sw t1, 4(t0)",
            "9:",
            "li a0, 0x200100",
            "li a1, 8",
            "li a7, 9",
            "ecall",
            // zero-length receive on W (cap 2)
            "li a0, 2",
            "li a1, 0x200040",
            "li a2, 128",
            "li a7, 4",
            "ecall",
            "beqz a0, 10f",
            "li t0, 0x200100",
            "li t1, 0x33435049",
            "sw t1, 0(t0)",
            "li t1, 0x0A464C5A", // "IPC3ZLF\n"
            "sw t1, 4(t0)",
            "j 11f",
            "10:",
            "li t0, 0x200100",
            "li t1, 0x33435049",
            "sw t1, 0(t0)",
            "li t1, 0x0A504C5A", // "IPC3ZLP\n"
            "sw t1, 4(t0)",
            "11:",
            "li a0, 0x200100",
            "li a1, 8",
            "li a7, 9",
            "ecall",
            "li a7, 2",
            "ecall",
            "12:",
            "j 12b",
            options(noreturn)
        )
    }
}

/// T3 recvY (prio 9): receive from Y, require exactly 16 bytes of
/// 0xB2, report, exit.
#[no_mangle]
extern "C" fn ipcown_recv_y() -> ! {
    // SAFETY: as ipcown_sender_a.
    unsafe {
        core::arch::asm!(
            "li a0, 1",
            "li a1, 0x200040",
            "li a2, 128",
            "li a7, 4",
            "ecall",
            "li t3, 16",
            "bne a0, t3, 8f",
            "li t0, 0x200040",
            "li t1, 16",
            "li t2, 0xB2",
            "1:",
            "lbu t4, 0(t0)",
            "bne t4, t2, 8f",
            "addi t0, t0, 1",
            "addi t1, t1, -1",
            "bnez t1, 1b",
            "li t0, 0x200100",
            "li t1, 0x31435049",
            "sw t1, 0(t0)",
            "li t1, 0x0A505952", // "IPC1RYP\n"
            "sw t1, 4(t0)",
            "j 9f",
            "8:",
            "li t0, 0x200100",
            "li t1, 0x31435049",
            "sw t1, 0(t0)",
            "li t1, 0x0A465952", // "IPC1RYF\n"
            "sw t1, 4(t0)",
            "9:",
            "li a0, 0x200100",
            "li a1, 8",
            "li a7, 9",
            "ecall",
            "li a7, 2",
            "ecall",
            "10:",
            "j 10b",
            options(noreturn)
        )
    }
}

/// T4 senderC (prio 8): park the maximum 128 bytes of 0xC3 on Z, exit.
#[no_mangle]
extern "C" fn ipcown_sender_c() -> ! {
    // SAFETY: as ipcown_sender_a.
    unsafe {
        core::arch::asm!(
            "li t0, 0x200040",
            "li t1, 128",
            "li t2, 0xC3",
            "1:",
            "sb t2, 0(t0)",
            "addi t0, t0, 1",
            "addi t1, t1, -1",
            "bnez t1, 1b",
            "li a0, 1",
            "li a1, 0x200040",
            "li a2, 128",
            "li a7, 3",
            "ecall",
            "li a7, 2",
            "ecall",
            "2:",
            "j 2b",
            options(noreturn)
        )
    }
}

/// T5 senderD (prio 7): send 1 byte on the busy endpoint Z. The send
/// must be rejected with ERR_INVALID_ARG (-5) and must NOT block.
#[no_mangle]
extern "C" fn ipcown_sender_d() -> ! {
    // SAFETY: as ipcown_sender_a.
    unsafe {
        core::arch::asm!(
            "li t0, 0x200040",
            "li t2, 0xD4",
            "sb t2, 0(t0)",
            "li a0, 1",
            "li a1, 0x200040",
            "li a2, 1",
            "li a7, 3",
            "ecall",
            "li t3, -5", // ERR_INVALID_ARG (busy)
            "bne a0, t3, 8f",
            "li t0, 0x200100",
            "li t1, 0x32435049",
            "sw t1, 0(t0)",
            "li t1, 0x0A595342", // "IPC2BSY\n"
            "sw t1, 4(t0)",
            "j 9f",
            "8:",
            "li t0, 0x200100",
            "li t1, 0x32435049",
            "sw t1, 0(t0)",
            "li t1, 0x0A465342", // "IPC2BSF\n"
            "sw t1, 4(t0)",
            "9:",
            "li a0, 0x200100",
            "li a1, 8",
            "li a7, 9",
            "ecall",
            "li a7, 2",
            "ecall",
            "10:",
            "j 10b",
            options(noreturn)
        )
    }
}

/// T8 senderF (prio 6): park 64 bytes of 0xF6 on V, exit. Runs before
/// recvZ so V is already SenderWaiting when the short receive happens.
#[no_mangle]
extern "C" fn ipcown_sender_f() -> ! {
    // SAFETY: as ipcown_sender_a.
    unsafe {
        core::arch::asm!(
            "li t0, 0x200040",
            "li t1, 64",
            "li t2, 0xF6",
            "1:",
            "sb t2, 0(t0)",
            "addi t0, t0, 1",
            "addi t1, t1, -1",
            "bnez t1, 1b",
            "li a0, 1",
            "li a1, 0x200040",
            "li a2, 64",
            "li a7, 3",
            "ecall",
            "li a7, 2",
            "ecall",
            "2:",
            "j 2b",
            options(noreturn)
        )
    }
}

/// T7 senderE (prio 5): send zero bytes on W. After release, attempt an
/// oversized 129-byte send and require ERR_MSG_TOO_LARGE (-6).
#[no_mangle]
extern "C" fn ipcown_sender_e() -> ! {
    // SAFETY: as ipcown_sender_a.
    unsafe {
        core::arch::asm!(
            // zero-length send on W (cap 1)
            "li a0, 1",
            "li a1, 0x200040",
            "li a2, 0",
            "li a7, 3",
            "ecall",
            // oversized send: 129 > IPC_MSG_MAX
            "li a0, 1",
            "li a1, 0x200040",
            "li a2, 129",
            "li a7, 3",
            "ecall",
            "li t3, -6", // ERR_MSG_TOO_LARGE
            "bne a0, t3, 8f",
            "li t0, 0x200100",
            "li t1, 0x33435049",
            "sw t1, 0(t0)",
            "li t1, 0x0A50564F", // "IPC3OVP\n"
            "sw t1, 4(t0)",
            "j 9f",
            "8:",
            "li t0, 0x200100",
            "li t1, 0x33435049",
            "sw t1, 0(t0)",
            "li t1, 0x0A46564F", // "IPC3OVF\n"
            "sw t1, 4(t0)",
            "9:",
            "li a0, 0x200100",
            "li a1, 8",
            "li a7, 9",
            "ecall",
            "li a7, 2",
            "ecall",
            "10:",
            "j 10b",
            options(noreturn)
        )
    }
}

/// T6 recvZ (prio 4): require C's exact 128 bytes from Z, then prove
/// the short-receive contract on V: a receive with capacity 16 against
/// a parked 64-byte message must return ERR_INVALID_ARG (-5) with the
/// sender still parked, and the adequately sized retry must return the
/// original length and bytes.
#[no_mangle]
extern "C" fn ipcown_recv_z() -> ! {
    // SAFETY: as ipcown_sender_a.
    unsafe {
        core::arch::asm!(
            // ---- seq2: exact 128 bytes of 0xC3 from Z (cap 1) ----
            "li a0, 1",
            "li a1, 0x200040",
            "li a2, 128",
            "li a7, 4",
            "ecall",
            "li t3, 128",
            "bne a0, t3, 8f",
            "li t0, 0x200040",
            "li t1, 128",
            "li t2, 0xC3",
            "1:",
            "lbu t4, 0(t0)",
            "bne t4, t2, 8f",
            "addi t0, t0, 1",
            "addi t1, t1, -1",
            "bnez t1, 1b",
            "li t0, 0x200100",
            "li t1, 0x32435049",
            "sw t1, 0(t0)",
            "li t1, 0x0A505A52", // "IPC2RZP\n"
            "sw t1, 4(t0)",
            "j 9f",
            "8:",
            "li t0, 0x200100",
            "li t1, 0x32435049",
            "sw t1, 0(t0)",
            "li t1, 0x0A465A52", // "IPC2RZF\n"
            "sw t1, 4(t0)",
            "9:",
            "li a0, 0x200100",
            "li a1, 8",
            "li a7, 9",
            "ecall",
            // ---- seq4a: short receive on V (cap 2), capacity 16 ----
            "li a0, 2",
            "li a1, 0x200040",
            "li a2, 16",
            "li a7, 4",
            "ecall",
            "li t3, -5", // ERR_INVALID_ARG, sender stays parked
            "bne a0, t3, 20f",
            "li t0, 0x200100",
            "li t1, 0x34435049",
            "sw t1, 0(t0)",
            "li t1, 0x0A504853", // "IPC4SHP\n"
            "sw t1, 4(t0)",
            "j 21f",
            "20:",
            "li t0, 0x200100",
            "li t1, 0x34435049",
            "sw t1, 0(t0)",
            "li t1, 0x0A464853", // "IPC4SHF\n"
            "sw t1, 4(t0)",
            "21:",
            "li a0, 0x200100",
            "li a1, 8",
            "li a7, 9",
            "ecall",
            // ---- seq4b: adequate retry must return the original ----
            "li a0, 2",
            "li a1, 0x200040",
            "li a2, 128",
            "li a7, 4",
            "ecall",
            "li t3, 64",
            "bne a0, t3, 30f",
            "li t0, 0x200040",
            "li t1, 64",
            "li t2, 0xF6",
            "22:",
            "lbu t4, 0(t0)",
            "bne t4, t2, 30f",
            "addi t0, t0, 1",
            "addi t1, t1, -1",
            "bnez t1, 22b",
            "li t0, 0x200100",
            "li t1, 0x34435049",
            "sw t1, 0(t0)",
            "li t1, 0x0A505452", // "IPC4RTP\n"
            "sw t1, 4(t0)",
            "j 31f",
            "30:",
            "li t0, 0x200100",
            "li t1, 0x34435049",
            "sw t1, 0(t0)",
            "li t1, 0x0A465452", // "IPC4RTF\n"
            "sw t1, 4(t0)",
            "31:",
            "li a0, 0x200100",
            "li a1, 8",
            "li a7, 9",
            "ecall",
            "li a7, 2",
            "ecall",
            "32:",
            "j 32b",
            options(noreturn)
        )
    }
}

/// T9 senderG (prio 3): park 32 bytes of 0x77 on U and never resume —
/// the controller kills it while it is parked.
#[no_mangle]
extern "C" fn ipcown_sender_g() -> ! {
    // SAFETY: as ipcown_sender_a.
    unsafe {
        core::arch::asm!(
            "li t0, 0x200040",
            "li t1, 32",
            "li t2, 0x77",
            "1:",
            "sb t2, 0(t0)",
            "addi t0, t0, 1",
            "addi t1, t1, -1",
            "bnez t1, 1b",
            "li a0, 1",
            "li a1, 0x200040",
            "li a2, 32",
            "li a7, 3",
            "ecall",
            "li a7, 2",
            "ecall",
            "2:",
            "j 2b",
            options(noreturn)
        )
    }
}

/// T10 controller (prio 2): kill the parked sender G (slot 9), which
/// releases endpoint U, then reuse U with a fresh 8-byte message.
/// Holds task-control authority in slot 0; the kill is independently
/// observable through the kernel's own TASK_KILLED line.
#[no_mangle]
extern "C" fn ipcown_controller() -> ! {
    // SAFETY: as ipcown_sender_a.
    unsafe {
        core::arch::asm!(
            "li a0, 9", // sys_task_kill(slot 9 = senderG)
            "li a7, 12",
            "ecall",
            "li t0, 0x200040",
            "li t1, 8",
            "li t2, 0x88",
            "1:",
            "sb t2, 0(t0)",
            "addi t0, t0, 1",
            "addi t1, t1, -1",
            "bnez t1, 1b",
            "li a0, 1", // reuse endpoint U
            "li a1, 0x200040",
            "li a2, 8",
            "li a7, 3",
            "ecall",
            "li a7, 2",
            "ecall",
            "2:",
            "j 2b",
            options(noreturn)
        )
    }
}

/// T11 recvU (prio 1): require exactly the 8 fresh bytes on the reused
/// endpoint — proving no residue of the killed sender's 32 bytes — and
/// emit the final completion tag.
#[no_mangle]
extern "C" fn ipcown_recv_u() -> ! {
    // SAFETY: as ipcown_sender_a.
    unsafe {
        core::arch::asm!(
            "li a0, 1",
            "li a1, 0x200040",
            "li a2, 128",
            "li a7, 4",
            "ecall",
            "li t3, 8",
            "bne a0, t3, 8f",
            "li t0, 0x200040",
            "li t1, 8",
            "li t2, 0x88",
            "1:",
            "lbu t4, 0(t0)",
            "bne t4, t2, 8f",
            "addi t0, t0, 1",
            "addi t1, t1, -1",
            "bnez t1, 1b",
            "li t0, 0x200100",
            "li t1, 0x35435049",
            "sw t1, 0(t0)",
            "li t1, 0x0A505552", // "IPC5RUP\n"
            "sw t1, 4(t0)",
            "j 9f",
            "8:",
            "li t0, 0x200100",
            "li t1, 0x35435049",
            "sw t1, 0(t0)",
            "li t1, 0x0A465552", // "IPC5RUF\n"
            "sw t1, 4(t0)",
            "9:",
            "li a0, 0x200100",
            "li a1, 8",
            "li a7, 9",
            "ecall",
            // final completion tag "IPCDONE\n"
            "li t0, 0x200100",
            "li t1, 0x44435049",
            "sw t1, 0(t0)",
            "li t1, 0x0A454E4F",
            "sw t1, 4(t0)",
            "li a0, 0x200100",
            "li a1, 8",
            "li a7, 9",
            "ecall",
            "li a7, 2",
            "ecall",
            "10:",
            "j 10b",
            options(noreturn)
        )
    }
}

/// Register the twelve participants and run the demo.
///
/// Priorities force the order without any timing dependence: each
/// sender blocks, handing the CPU to the next-lower-priority Ready
/// task. senderF (T8) sits at priority 6 so its 64-byte message is
/// already parked on V before recvZ (priority 4) performs the short
/// receive and the retry.
pub fn ipc_payload_demo() -> ! {
    // (name, entry, priority, slot)
    let bodies: [(&str, u64, u8); TASKS] = [
        ("ipcown_sender_a", ipcown_sender_a as *const () as u64, 12),
        ("ipcown_sender_b", ipcown_sender_b as *const () as u64, 11),
        ("ipcown_recv_x", ipcown_recv_x as *const () as u64, 10),
        ("ipcown_recv_y", ipcown_recv_y as *const () as u64, 9),
        ("ipcown_sender_c", ipcown_sender_c as *const () as u64, 8),
        ("ipcown_sender_d", ipcown_sender_d as *const () as u64, 7),
        ("ipcown_recv_z", ipcown_recv_z as *const () as u64, 4),
        ("ipcown_sender_e", ipcown_sender_e as *const () as u64, 5),
        ("ipcown_sender_f", ipcown_sender_f as *const () as u64, 6),
        ("ipcown_sender_g", ipcown_sender_g as *const () as u64, 3),
        (
            "ipcown_controller",
            ipcown_controller as *const () as u64,
            2,
        ),
        ("ipcown_recv_u", ipcown_recv_u as *const () as u64, 1),
    ];

    for (i, &(name, code_phys, prio)) in bodies.iter().enumerate() {
        // SAFETY: addr_of! forms a raw pointer to a static-mut element;
        // boot-time, single hart, distinct slot i.
        let stack_phys = unsafe { core::ptr::addr_of!(IPCOWN_STACKS[i]) as u64 };
        let uas = paging_hw::build_user_address_space(i, code_phys, stack_phys);
        // SAFETY: boot-time, single hart, distinct slot i.
        unsafe {
            dispatch::register_task(i, name, prio, uas.root, uas.entry_va, uas.stack_top_va);
        }
        uart::put_str("TASK_STARTED task=");
        uart::put_str(name);
        uart::put_str("\n");
    }

    // Capabilities. Slot 0 is the console grant (task-control for the
    // controller); slot 1, and slot 2 where a task uses two endpoints,
    // carry the endpoint grants.
    // SAFETY: boot-time capability minting, single hart, distinct slots.
    unsafe {
        for i in 0..TASKS {
            if i != 10 {
                dispatch::set_boot_cap(i, dispatch::cap_console(dispatch::CAP_RIGHT_SEND));
            }
        }
        dispatch::set_boot_cap(10, dispatch::cap_control());

        dispatch::set_endpoint_cap(0, 1, EP_X, RIGHT_SEND); // senderA -> X
        dispatch::set_endpoint_cap(1, 1, EP_Y, RIGHT_SEND); // senderB -> Y
        dispatch::set_endpoint_cap(2, 1, EP_X, RIGHT_RECV); // recvX  <- X
        dispatch::set_endpoint_cap(2, 2, EP_W, RIGHT_RECV); // recvX  <- W (zero length)
        dispatch::set_endpoint_cap(3, 1, EP_Y, RIGHT_RECV); // recvY  <- Y
        dispatch::set_endpoint_cap(4, 1, EP_Z, RIGHT_SEND); // senderC -> Z
        dispatch::set_endpoint_cap(5, 1, EP_Z, RIGHT_SEND); // senderD -> Z (busy)
        dispatch::set_endpoint_cap(6, 1, EP_Z, RIGHT_RECV); // recvZ  <- Z
        dispatch::set_endpoint_cap(6, 2, EP_V, RIGHT_RECV); // recvZ  <- V (short + retry)
        dispatch::set_endpoint_cap(7, 1, EP_W, RIGHT_SEND); // senderE -> W
        dispatch::set_endpoint_cap(8, 1, EP_V, RIGHT_SEND); // senderF -> V
        dispatch::set_endpoint_cap(9, 1, EP_U, RIGHT_SEND); // senderG -> U
        dispatch::set_endpoint_cap(10, 1, EP_U, RIGHT_SEND); // controller -> U
        dispatch::set_endpoint_cap(11, 1, EP_U, RIGHT_RECV); // recvU  <- U
    }

    // SAFETY: all tasks registered with valid address spaces; the trap
    // stack is valid. Task 0 (senderA) runs first.
    unsafe {
        let trap_stack_top = core::ptr::addr_of!(IPCOWN_TRAP_STACK) as u64 + 8 * 1024;
        dispatch::start(trap_stack_top)
    }
}
