//! Thread execution context (AXIOM-THREAD-002).
//!
//! Requirement reference: docs/03_KERNEL_OBJECTS.md §2.
//!
//! Arch-independent wrapper around the architecture context. The thread
//! layer never touches individual registers; it only owns, hands out,
//! and validates the context as a unit.

// The RISC-V context is plain data (repr(C) u64 fields), so it compiles
// and is unit-tested on the host as well.
#[path = "../arch/riscv64/context.rs"]
mod arch;

pub use arch::ArchContext;

/// Execution context of one thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ThreadContext {
    arch: ArchContext,
}

impl ThreadContext {
    /// New context with resume point and stack top set. Both must be
    /// nonzero: a thread that would resume at address zero or run on a
    /// null stack is a construction error, rejected before it can exist.
    pub fn new(resume_at: u64, stack_top: u64) -> Option<Self> {
        if resume_at == 0 || stack_top == 0 {
            return None;
        }
        let mut arch = ArchContext::zeroed();
        arch.ra = resume_at;
        arch.sp = stack_top;
        Some(ThreadContext { arch })
    }

    pub const fn arch(&self) -> &ArchContext {
        &self.arch
    }

    /// True if this context may be switched to (nonzero resume point
    /// and stack).
    pub fn is_runnable(&self) -> bool {
        self.arch.ra != 0 && self.arch.sp != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_resume_or_stack_rejected() {
        assert!(ThreadContext::new(0, 0x8030_0000).is_none());
        assert!(ThreadContext::new(0x8020_1000, 0).is_none());
    }

    #[test]
    fn valid_context_is_runnable() {
        let c = ThreadContext::new(0x8020_1000, 0x8030_0000).unwrap();
        assert!(c.is_runnable());
        assert_eq!(c.arch().ra, 0x8020_1000);
        assert_eq!(c.arch().sp, 0x8030_0000);
    }

    #[test]
    fn default_context_is_not_runnable() {
        assert!(!ThreadContext::default().is_runnable());
    }
}
