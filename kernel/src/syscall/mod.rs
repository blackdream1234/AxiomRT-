//! AxiomRT syscall dispatch stub (AXIOM-TRAP-003).
//!
//! Requirement reference: docs/04_SYSCALL_MODEL.md,
//! docs/10_TRAP_MODEL.md §5.
//!
//! Phase 3 scope: stub dispatch only. The syscall trap is recognized, the
//! number is decoded, and a controlled result is returned. No IPC, no
//! capabilities, no thread logic (Phase 3 boundary). Real implementations
//! arrive with their phases (yield/exit: Phase 6/7; send/recv/reply:
//! Phase 8/9; cap_query: Phase 9; fault_ack: Phase 10).

// Phase 3 stub status: the result-code set below is the complete ABI
// surface fixed by docs/04_SYSCALL_MODEL.md; codes not yet returned by the
// stub are consumed by later phases. Remove this allowance when the real
// syscall implementations land.
#![allow(dead_code)]

use crate::uart;

use super::TrapFrame;

/// Syscall numbers (docs/04_SYSCALL_MODEL.md ABI; a7).
pub const SYS_YIELD: u64 = 1;
pub const SYS_EXIT: u64 = 2;
pub const SYS_SEND: u64 = 3;
pub const SYS_RECV: u64 = 4;
pub const SYS_REPLY: u64 = 5;
pub const SYS_CAP_QUERY: u64 = 6;
pub const SYS_FAULT_ACK: u64 = 7;

/// Result codes (docs/04_SYSCALL_MODEL.md; returned in a0 as i64).
pub const OK: i64 = 0;
pub const ERR_INVALID_SYSCALL: i64 = -1;
pub const ERR_INVALID_CAP: i64 = -2;
pub const ERR_INSUFFICIENT_RIGHTS: i64 = -3;
pub const ERR_WRONG_OBJECT_TYPE: i64 = -4;
pub const ERR_INVALID_ARG: i64 = -5;
pub const ERR_MSG_TOO_LARGE: i64 = -6;
pub const ERR_PEER_KILLED: i64 = -7;
pub const ERR_NO_PENDING_FAULT: i64 = -8;
/// Phase 3 stub code: the syscall exists in the model but its
/// implementing phase has not landed yet.
pub const ERR_NOT_IMPLEMENTED: i64 = -9;

fn stub(name: &str) -> i64 {
    uart::put_str("SYSCALL name=");
    uart::put_str(name);
    uart::put_str(" status=stub result=ERR_NOT_IMPLEMENTED\n");
    ERR_NOT_IMPLEMENTED
}

/// Dispatch a recognized syscall trap. Phase 3: every known syscall is
/// acknowledged with a structured line and `ERR_NOT_IMPLEMENTED`; an
/// unknown number logs a controlled error and returns
/// `ERR_INVALID_SYSCALL` (docs/04_SYSCALL_MODEL.md, forbidden/unknown
/// syscall rule). The kernel never panics on a bad syscall number.
pub fn dispatch(number: u64, _frame: &mut TrapFrame) -> i64 {
    match number {
        SYS_YIELD => stub("sys_yield"),
        SYS_EXIT => stub("sys_exit"),
        SYS_SEND => stub("sys_send"),
        SYS_RECV => stub("sys_recv"),
        SYS_REPLY => stub("sys_reply"),
        SYS_CAP_QUERY => stub("sys_cap_query"),
        SYS_FAULT_ACK => stub("sys_fault_ack"),
        _ => {
            uart::put_str("SYSCALL name=unknown status=rejected result=ERR_INVALID_SYSCALL\n");
            ERR_INVALID_SYSCALL
        }
    }
}
