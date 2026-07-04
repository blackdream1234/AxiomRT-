//! RISC-V 64 trap handling (AXIOM-TRAP-001, -002, -003).
//!
//! Requirement reference: docs/10_TRAP_MODEL.md, docs/06_FAULT_MODEL.md.
//!
//! Phase 3 scope: controlled entry paths for exceptions and the syscall
//! trap stub. No user mode, no scheduler, no IPC, no capabilities.
//! Unknown traps lead to a controlled panic; illegal instructions produce
//! a structured trap message and a safe halt.

use crate::uart;

/// Saved register state pushed by `__trap_vector` (trap.S).
/// Layout contract: `regs[i]` holds general register `x(i+1)`;
/// `sepc` is the trapped program counter. Must stay in sync with trap.S.
#[repr(C)]
pub struct TrapFrame {
    pub regs: [u64; 31],
    pub sepc: u64,
}

/// Supervisor exception cause codes (scause, interrupt bit clear).
const CAUSE_ILLEGAL_INSTRUCTION: u64 = 2;
const INTERRUPT_BIT: u64 = 1 << 63;

/// Install the trap vector (direct mode). Called once at boot.
pub fn init() {
    extern "C" {
        fn __trap_vector();
    }
    // SAFETY (docs/07_CODEX_RULES.md §6): writing stvec is a privileged
    // CSR operation that hardware requires for trap delivery
    // (docs/10_TRAP_MODEL.md). __trap_vector is 4-byte aligned (.align 2
    // in trap.S) and mode bits 00 select direct mode, so the written
    // value is valid for stvec.
    unsafe {
        core::arch::asm!("csrw stvec, {v}", v = in(reg) (__trap_vector as *const () as usize));
    }
}

fn read_scause() -> u64 {
    let v: u64;
    // SAFETY: reading scause is a side-effect-free privileged CSR read;
    // the kernel runs in S-mode where this access is architecturally legal.
    unsafe { core::arch::asm!("csrr {v}, scause", v = out(reg) v) };
    v
}

fn read_stval() -> u64 {
    let v: u64;
    // SAFETY: reading stval is a side-effect-free privileged CSR read in
    // S-mode.
    unsafe { core::arch::asm!("csrr {v}, stval", v = out(reg) v) };
    v
}

fn put_hex(value: u64) {
    uart::put_str("0x");
    for shift in (0..16).rev() {
        let digit = ((value >> (shift * 4)) & 0xf) as u8;
        uart::put_byte(if digit < 10 { b'0' + digit } else { b'a' + digit - 10 });
    }
}

/// Structured trap report over serial (docs/10_TRAP_MODEL.md §4).
fn report(kind: &str, cause: u64, frame: &TrapFrame) {
    uart::put_str("TRAP kind=");
    uart::put_str(kind);
    uart::put_str(" cause=");
    put_hex(cause);
    uart::put_str(" sepc=");
    put_hex(frame.sepc);
    uart::put_str(" stval=");
    put_hex(read_stval());
    uart::put_str("\n");
}

/// Safe halt: containment endpoint for Phase 3 fatal traps
/// (docs/06_FAULT_MODEL.md, KernelPanic — controlled halt, no silent
/// restart, no continuation).
fn halt() -> ! {
    loop {
        core::hint::spin_loop();
    }
}

/// Central trap handler, called from `__trap_vector` with the saved frame.
#[no_mangle]
pub extern "C" fn trap_handler(frame: &mut TrapFrame) {
    let scause = read_scause();

    if scause & INTERRUPT_BIT != 0 {
        // Phase 3: no interrupt sources are enabled; an interrupt here is
        // outside the specified state space -> controlled panic.
        report("unexpected-interrupt", scause, frame);
        uart::put_str("PANIC kernel=axiomrt reason=unexpected_interrupt phase=trap\n");
        halt();
    }

    match scause {
        // AXIOM-TRAP-002: illegal instruction is identified, reported in a
        // structured message, and the system halts safely for now (no
        // user tasks exist yet to contain it against; from Phase 5+ this
        // becomes an IllegalInstruction fault, docs/06_FAULT_MODEL.md).
        CAUSE_ILLEGAL_INSTRUCTION => {
            report("illegal-instruction", scause, frame);
            uart::put_str("HALT reason=illegal_instruction phase=trap\n");
            halt();
        }

        // Unknown trap: controlled panic (AXIOM-TRAP-001 requirement).
        // Dedicated handling for the syscall trap arrives with
        // AXIOM-TRAP-003.
        _ => {
            report("unknown", scause, frame);
            uart::put_str("PANIC kernel=axiomrt reason=unknown_trap phase=trap\n");
            halt();
        }
    }
}
