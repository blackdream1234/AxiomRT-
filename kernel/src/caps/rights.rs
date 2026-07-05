//! Capability rights (AXIOM-CAP-001).
//!
//! Requirement reference: docs/06_CAPABILITY_MODEL.md §2,
//! docs/03_KERNEL_OBJECTS.md §8, Project Description §11.
//!
//! A rights set is an explicit bitmask. No implicit widening exists:
//! checks are subset tests, and there is no operation that adds rights
//! to an existing capability (no amplification, docs/03 §8).

/// An explicit set of access rights.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rights(u16);

impl Rights {
    pub const NONE: Rights = Rights(0);
    pub const READ: Rights = Rights(1 << 0);
    pub const WRITE: Rights = Rights(1 << 1);
    pub const EXECUTE: Rights = Rights(1 << 2);
    pub const SEND: Rights = Rights(1 << 3);
    pub const RECEIVE: Rights = Rights(1 << 4);
    pub const GRANT: Rights = Rights(1 << 5);
    pub const MAP: Rights = Rights(1 << 6);
    pub const CONTROL: Rights = Rights(1 << 7);

    /// Combine rights (used only when the kernel mints capabilities).
    pub const fn union(self, other: Rights) -> Rights {
        Rights(self.0 | other.0)
    }

    /// Subset test: does this set include every right in `required`?
    pub const fn contains(self, required: Rights) -> bool {
        self.0 & required.0 == required.0
    }

    /// Remove rights — the only direction rights can change
    /// (diminish on derive; never amplify).
    pub const fn diminish(self, removed: Rights) -> Rights {
        Rights(self.0 & !removed.0)
    }

    pub const fn bits(self) -> u16 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_is_subset_test() {
        let rw = Rights::READ.union(Rights::WRITE);
        assert!(rw.contains(Rights::READ));
        assert!(rw.contains(Rights::WRITE));
        assert!(rw.contains(rw));
        assert!(!rw.contains(Rights::SEND));
        assert!(!Rights::NONE.contains(Rights::READ));
        assert!(rw.contains(Rights::NONE), "empty requirement always holds");
    }

    #[test]
    fn diminish_never_amplifies() {
        let s = Rights::SEND.union(Rights::RECEIVE);
        let d = s.diminish(Rights::RECEIVE);
        assert!(d.contains(Rights::SEND));
        assert!(!d.contains(Rights::RECEIVE));
        // Diminishing by an unheld right changes nothing.
        assert_eq!(s.diminish(Rights::CONTROL), s);
    }

    #[test]
    fn all_eight_rights_are_distinct() {
        let all = [
            Rights::READ,
            Rights::WRITE,
            Rights::EXECUTE,
            Rights::SEND,
            Rights::RECEIVE,
            Rights::GRANT,
            Rights::MAP,
            Rights::CONTROL,
        ];
        for (i, a) in all.iter().enumerate() {
            for (j, b) in all.iter().enumerate() {
                if i != j {
                    assert!(!a.contains(*b), "right {i} must not imply right {j}");
                }
            }
        }
    }
}
