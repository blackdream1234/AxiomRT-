//! Scheduling priorities (AXIOM-SCHED-001).
//!
//! Requirement reference: docs/09_SCHEDULER_MODEL.md §2.
//!
//! Fixed priority levels 0..=7; higher value = more urgent. Priorities
//! are assigned at task creation (static in v0.1) and validated at
//! construction — an out-of-range priority cannot exist.

/// Number of priority levels in v0.1.
pub const PRIORITY_LEVELS: usize = 8;

/// A validated fixed priority. Higher value = scheduled first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Priority(u8);

impl Priority {
    /// Lowest priority (background).
    pub const MIN: Priority = Priority(0);
    /// Highest priority (critical tasks).
    pub const MAX: Priority = Priority((PRIORITY_LEVELS - 1) as u8);

    /// Construct a priority; values outside 0..PRIORITY_LEVELS are
    /// rejected (docs/03_KERNEL_OBJECTS.md §9: priority values outside
    /// the defined range are invalid operations).
    pub const fn new(level: u8) -> Option<Priority> {
        if (level as usize) < PRIORITY_LEVELS {
            Some(Priority(level))
        } else {
            None
        }
    }

    pub const fn level(self) -> u8 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_is_enforced() {
        assert!(Priority::new(0).is_some());
        assert!(Priority::new(7).is_some());
        assert!(Priority::new(8).is_none());
        assert!(Priority::new(255).is_none());
    }

    #[test]
    fn ordering_follows_urgency() {
        assert!(Priority::MAX > Priority::MIN);
        assert!(Priority::new(5).unwrap() > Priority::new(3).unwrap());
    }
}
