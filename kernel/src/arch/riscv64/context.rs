//! RISC-V 64 saved register context (AXIOM-THREAD-002).
//!
//! Requirement reference: docs/03_KERNEL_OBJECTS.md §2 and
//! Implementation Notes; RISC-V ELF psABI calling convention.
//!
//! Assumptions (documented per task requirement):
//!   1. Kernel-side context switches happen at a controlled call
//!      boundary (the future `switch_to` in Phase 6/7), so only
//!      callee-saved registers plus ra/sp need to be preserved here;
//!      caller-saved registers are dead at a call boundary by the ABI.
//!   2. Full register state of an *interrupted* thread (all x1..x31)
//!      lives in the trap frame (arch/riscv64/trap.S), not here. The
//!      two structures have distinct jobs and must not be merged.
//!   3. Floating point state is not saved: the kernel does not use FP,
//!      and user tasks get FP state handling only if a later phase
//!      enables it explicitly (mstatus.FS is Off by default).
//!   4. Address-space switching (satp) is added when the MMU is
//!      activated; it is deliberately absent from this structure now.
//!
//! No context switch assembly exists in this task (Phase 5 boundary).

/// Callee-saved execution context of a suspended kernel-visible thread.
///
/// Layout is `#[repr(C)]` and fixed: the future context-switch assembly
/// (Phase 6/7) will address these fields by offset. Field order must
/// not change without updating that assembly and this comment.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ArchContext {
    /// Return address (x1): where the thread resumes.
    pub ra: u64,
    /// Stack pointer (x2): the thread's own kernel stack.
    pub sp: u64,
    /// Callee-saved registers s0..s11 (x8, x9, x18..x27).
    pub s: [u64; 12],
}

impl ArchContext {
    /// A zeroed context: `resume` target and stack must be set before
    /// first use; a zero ra/sp context must never be switched to.
    pub const fn zeroed() -> Self {
        ArchContext {
            ra: 0,
            sp: 0,
            s: [0; 12],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_is_fixed_and_complete() {
        // 2 + 12 registers, 8 bytes each: the offset contract for the
        // future switch assembly (ra=0, sp=8, s0=16 .. s11=104).
        assert_eq!(core::mem::size_of::<ArchContext>(), 14 * 8);
        assert_eq!(core::mem::align_of::<ArchContext>(), 8);
    }
}
