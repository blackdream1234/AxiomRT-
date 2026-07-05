//! Kernel monitoring event model (AXIOM-MON-001).
//!
//! Requirement reference: docs/11_RUNTIME_MONITORING.md, Project
//! Description §18.
//!
//! Structured evidence records: every security- or safety-relevant
//! kernel action produces one MonitorEvent. The model defines *what* an
//! event is; no storage backend exists (serial export only in v0.1,
//! AXIOM-MON-002).

use crate::fault::{RecoveryDecision, Severity};
use crate::thread::ThreadId;

/// The nine v0.1 monitoring event types (docs/11_RUNTIME_MONITORING.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorEventType {
    TaskStarted,
    TaskExited,
    TaskFaulted,
    CapDenied,
    IpcDenied,
    PageFault,
    DeadlineMissed,
    WatchdogTimeout,
    RecoveryApplied,
}

impl MonitorEventType {
    /// Canonical wire name (stable, uppercase — the evidence format).
    pub const fn name(self) -> &'static str {
        match self {
            MonitorEventType::TaskStarted => "TASK_STARTED",
            MonitorEventType::TaskExited => "TASK_EXITED",
            MonitorEventType::TaskFaulted => "TASK_FAULTED",
            MonitorEventType::CapDenied => "CAP_DENIED",
            MonitorEventType::IpcDenied => "IPC_DENIED",
            MonitorEventType::PageFault => "PAGE_FAULT",
            MonitorEventType::DeadlineMissed => "DEADLINE_MISSED",
            MonitorEventType::WatchdogTimeout => "WATCHDOG_TIMEOUT",
            MonitorEventType::RecoveryApplied => "RECOVERY_APPLIED",
        }
    }

    /// Baseline severity of the event class (docs/11).
    pub const fn severity(self) -> Severity {
        match self {
            MonitorEventType::TaskStarted | MonitorEventType::TaskExited => Severity::Info,
            MonitorEventType::RecoveryApplied => Severity::Info,
            MonitorEventType::TaskFaulted
            | MonitorEventType::CapDenied
            | MonitorEventType::IpcDenied
            | MonitorEventType::PageFault
            | MonitorEventType::DeadlineMissed => Severity::Error,
            MonitorEventType::WatchdogTimeout => Severity::Critical,
        }
    }
}

/// Kernel phase in which the event was produced (docs/11 field list).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KernelPhase {
    Boot,
    Trap,
    Syscall,
    Sched,
    Ipc,
    Fault,
    User,
}

impl KernelPhase {
    pub const fn name(self) -> &'static str {
        match self {
            KernelPhase::Boot => "boot",
            KernelPhase::Trap => "trap",
            KernelPhase::Syscall => "syscall",
            KernelPhase::Sched => "sched",
            KernelPhase::Ipc => "ipc",
            KernelPhase::Fault => "fault",
            KernelPhase::User => "user",
        }
    }
}

/// One structured monitoring event (docs/11 field list). Optional
/// fields are `Option` — absent fields are omitted from the export,
/// never faked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonitorEvent {
    /// Timestamp (v0.1: caller-provided monotonic value; a hardware
    /// time source is wired when the timer phase lands).
    pub timestamp: u64,
    pub task: ThreadId,
    pub event_type: MonitorEventType,
    pub severity: Severity,
    pub phase: KernelPhase,
    /// Recovery decision, for RECOVERY_APPLIED.
    pub policy_result: Option<RecoveryDecision>,
    /// Related capability index, if relevant (CAP_DENIED, IPC_DENIED).
    pub related_cap: Option<u32>,
    /// Related syscall number, if relevant.
    pub related_syscall: Option<u64>,
}

impl MonitorEvent {
    /// New event with derived severity and no optional fields.
    pub const fn new(
        timestamp: u64,
        task: ThreadId,
        event_type: MonitorEventType,
        phase: KernelPhase,
    ) -> Self {
        MonitorEvent {
            timestamp,
            task,
            event_type,
            severity: event_type.severity(),
            phase,
            policy_result: None,
            related_cap: None,
            related_syscall: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_the_documented_nine() {
        let all = [
            (MonitorEventType::TaskStarted, "TASK_STARTED"),
            (MonitorEventType::TaskExited, "TASK_EXITED"),
            (MonitorEventType::TaskFaulted, "TASK_FAULTED"),
            (MonitorEventType::CapDenied, "CAP_DENIED"),
            (MonitorEventType::IpcDenied, "IPC_DENIED"),
            (MonitorEventType::PageFault, "PAGE_FAULT"),
            (MonitorEventType::DeadlineMissed, "DEADLINE_MISSED"),
            (MonitorEventType::WatchdogTimeout, "WATCHDOG_TIMEOUT"),
            (MonitorEventType::RecoveryApplied, "RECOVERY_APPLIED"),
        ];
        for (t, n) in all {
            assert_eq!(t.name(), n);
        }
    }

    #[test]
    fn severity_is_derived() {
        assert_eq!(MonitorEventType::TaskStarted.severity(), Severity::Info);
        assert_eq!(MonitorEventType::CapDenied.severity(), Severity::Error);
        assert_eq!(MonitorEventType::WatchdogTimeout.severity(), Severity::Critical);
        let e = MonitorEvent::new(1, ThreadId(2), MonitorEventType::PageFault, KernelPhase::Trap);
        assert_eq!(e.severity, Severity::Error);
    }

    #[test]
    fn optional_fields_default_absent() {
        let e = MonitorEvent::new(1, ThreadId(2), MonitorEventType::TaskExited, KernelPhase::Syscall);
        assert!(e.policy_result.is_none());
        assert!(e.related_cap.is_none());
        assert!(e.related_syscall.is_none());
    }
}
