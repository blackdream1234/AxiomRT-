//! RISC-V 64 trap handling (AXIOM-TRAP-001, -002, -003).
//!
//! Requirement reference: docs/10_TRAP_MODEL.md, docs/06_FAULT_MODEL.md.
//!
//! Phase 3 scope: controlled entry paths for exceptions and the syscall
//! trap stub. No user mode, no scheduler, no IPC, no capabilities.
//! Unknown traps lead to a controlled panic; illegal instructions produce
//! a structured trap message and a safe halt.

use crate::uart;

// Syscall stub dispatch (AXIOM-TRAP-003). Declared here because the trap
// layer owns the only entry path to syscalls; path keeps the module at
// its documented location kernel/src/syscall/.
#[path = "../../syscall/mod.rs"]
pub mod syscall;

/// Saved register state pushed by `__trap_vector` (trap.S).
/// Layout contract: `regs[i]` holds general register `x(i+1)`;
/// `sepc` is the trapped program counter; `sstatus` carries the
/// pre-trap privilege in SPP. Must stay in sync with trap.S.
#[repr(C)]
pub struct TrapFrame {
    pub regs: [u64; 31],
    pub sepc: u64,
    pub sstatus: u64,
}

/// sstatus.SPP: privilege level the trap came from (0 = user).
const SSTATUS_SPP: u64 = 1 << 8;

impl TrapFrame {
    /// Syscall number register a7 (x17).
    pub fn a7(&self) -> u64 {
        self.regs[16]
    }
    /// Result register a0 (x10).
    pub fn set_a0(&mut self, value: i64) {
        self.regs[9] = value as u64;
    }
    /// True if the trap was taken from user mode (AXIOM-USER-002).
    pub fn from_user(&self) -> bool {
        self.sstatus & SSTATUS_SPP == 0
    }
}

/// Supervisor exception cause codes (scause, interrupt bit clear).
const CAUSE_ILLEGAL_INSTRUCTION: u64 = 2;
const CAUSE_ECALL_FROM_U: u64 = 8;
const CAUSE_ECALL_FROM_S: u64 = 9;
const CAUSE_INSTRUCTION_PAGE_FAULT: u64 = 12;
const CAUSE_LOAD_PAGE_FAULT: u64 = 13;
const CAUSE_STORE_PAGE_FAULT: u64 = 15;
const INTERRUPT_BIT: u64 = 1 << 63;

/// Kernel virtual range (identity-mapped), for classifying whether a
/// user page fault targeted kernel memory vs. an unmapped user address
/// (docs/12_MMU_SV39.md §6). Matches kernel/linker.ld KERNEL_BASE.
const KERNEL_BASE: u64 = 0x8020_0000;

// ---------------------------------------------------------------------
// User fault containment (AXIOM-USER-002, docs/10_USER_MODE.md §4).
//
// Before the kernel enters user mode it registers a continuation
// (kernel resume point + kernel stack). A faulting user task is then
// *contained*: the trap frame is redirected to the continuation in
// S-mode instead of resuming the user task, and the kernel survives.
// Zero means "no user session active" — user faults without a
// continuation are a kernel invariant violation (cannot happen: the
// only path to user mode registers the continuation first).

use core::sync::atomic::{AtomicU64, Ordering};

static USER_FAULT_CONT_PC: AtomicU64 = AtomicU64::new(0);
static USER_FAULT_CONT_SP: AtomicU64 = AtomicU64::new(0);

/// Register the kernel continuation used when a user task must be
/// terminated after a fault. Called by the user layer before sret to U.
pub fn set_user_fault_continuation(pc: u64, sp: u64) {
    USER_FAULT_CONT_PC.store(pc, Ordering::SeqCst);
    USER_FAULT_CONT_SP.store(sp, Ordering::SeqCst);
}

/// Contain a faulting user task: stop it (it never resumes) and
/// redirect execution to the registered kernel continuation.
/// Returns false if no continuation is registered (kernel must halt).
fn contain_user_fault(frame: &mut TrapFrame, reason: &str) -> bool {
    let pc = USER_FAULT_CONT_PC.load(Ordering::SeqCst);
    let sp = USER_FAULT_CONT_SP.load(Ordering::SeqCst);
    if pc == 0 || sp == 0 {
        return false;
    }
    uart::put_str("CONTAIN scope=user reason=");
    uart::put_str(reason);
    uart::put_str(" action=terminate_task kernel=alive\n");
    // Redirect: resume in S-mode at the continuation, on a kernel stack.
    frame.sepc = pc;
    frame.regs[1] = sp; // x2 slot (destination stack)
    frame.sstatus |= SSTATUS_SPP;
    true
}

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
        // AXIOM-TRAP-002 / AXIOM-USER-002: illegal instruction is
        // identified and reported. From user mode it is *contained*
        // (IllegalInstruction fault, docs/06_FAULT_MODEL.md): the task
        // is terminated and the kernel continues. From kernel mode it
        // remains a safe halt (kernel invariant violation).
        CAUSE_ILLEGAL_INSTRUCTION => {
            report("illegal-instruction", scause, frame);
            if frame.from_user() && contain_user_fault(frame, "illegal_instruction") {
                return;
            }
            uart::put_str("HALT reason=illegal_instruction phase=trap\n");
            halt();
        }

        // AXIOM-TRAP-003: syscall trap stub. Recognizes the syscall trap,
        // dispatches on a7, returns a controlled result in a0. Syscalls
        // enter only from user mode: ecall from S-mode is an SBI call
        // handled by OpenSBI in M-mode and never delegated here
        // (docs/10_TRAP_MODEL.md §5).
        CAUSE_ECALL_FROM_U => {
            let result = syscall::dispatch(frame.a7(), frame);
            frame.set_a0(result);
            // Resume after the 4-byte ecall instruction, never re-execute it.
            frame.sepc = frame.sepc.wrapping_add(4);
        }

        // An S-mode ecall reaching the kernel means trap delegation is
        // misconfigured (OpenSBI owns this cause). Outside the specified
        // state space -> controlled panic, never silent continuation.
        CAUSE_ECALL_FROM_S => {
            report("smode-ecall-misdelegated", scause, frame);
            uart::put_str("PANIC kernel=axiomrt reason=smode_ecall_misdelegated phase=trap\n");
            halt();
        }

        // AXIOM-MEMHW-008: Sv39 page faults (instruction/load/store).
        // From user mode these are contained exactly like other user
        // faults (docs/12_MMU_SV39.md §6): structured page-fault report,
        // task terminated, kernel survives. The reason distinguishes a
        // user access to kernel memory from an unmapped user address.
        // A page fault taken in kernel mode is a KernelInvariantViolation.
        CAUSE_INSTRUCTION_PAGE_FAULT | CAUSE_LOAD_PAGE_FAULT | CAUSE_STORE_PAGE_FAULT => {
            report("page-fault", scause, frame);
            if frame.from_user() {
                let reason = page_fault_reason(scause, read_stval());
                if contain_user_fault(frame, reason) {
                    return;
                }
            }
            uart::put_str("PANIC kernel=axiomrt reason=kernel_page_fault phase=trap\n");
            halt();
        }

        // Any other exception from user mode is a user fault: contained,
        // task terminated, kernel survives (AXIOM-USER-002).
        // From kernel mode: controlled panic (AXIOM-TRAP-001).
        _ => {
            report("unknown", scause, frame);
            if frame.from_user() && contain_user_fault(frame, "exception") {
                return;
            }
            uart::put_str("PANIC kernel=axiomrt reason=unknown_trap phase=trap\n");
            halt();
        }
    }
}

/// Classify a user page fault for the containment report
/// (docs/12_MMU_SV39.md §6).
fn page_fault_reason(scause: u64, fault_addr: u64) -> &'static str {
    if fault_addr >= KERNEL_BASE {
        "user_access_kernel_memory"
    } else if scause == CAUSE_INSTRUCTION_PAGE_FAULT {
        "user_execute_nonexecutable"
    } else {
        "user_access_unmapped"
    }
}
