//! Serial event export (AXIOM-MON-002).
//!
//! Requirement reference: docs/11_RUNTIME_MONITORING.md §4.
//!
//! Renders one MonitorEvent per line in the stable `EVT key=value`
//! format and hands it to a caller-provided sink (on target: the QEMU
//! UART writer). `no_std`, allocation-free: a fixed stack buffer;
//! overflow is marked explicitly, never silent.

use core::fmt::Write as _;

use crate::fault::{RecoveryDecision, Severity};

use super::event::MonitorEvent;

/// Fixed render buffer size: generously above the longest possible
/// line (all fields present with maximal u64 values ≈ 130 bytes).
pub const LINE_CAPACITY: usize = 192;

/// One rendered event line.
pub struct EventLine {
    buf: [u8; LINE_CAPACITY],
    len: usize,
    truncated: bool,
}

impl EventLine {
    pub fn as_str(&self) -> &str {
        // Rendering only ever writes ASCII (key=value tokens and
        // decimal numbers), so the slice is always valid UTF-8.
        core::str::from_utf8(&self.buf[..self.len]).expect("ASCII by construction")
    }

    pub const fn truncated(&self) -> bool {
        self.truncated
    }
}

struct BufWriter<'a> {
    buf: &'a mut [u8],
    len: usize,
    overflow: bool,
}

impl core::fmt::Write for BufWriter<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let bytes = s.as_bytes();
        let space = self.buf.len() - self.len;
        if bytes.len() > space {
            self.buf[self.len..].copy_from_slice(&bytes[..space]);
            self.len = self.buf.len();
            self.overflow = true;
            return Err(core::fmt::Error);
        }
        self.buf[self.len..self.len + bytes.len()].copy_from_slice(bytes);
        self.len += bytes.len();
        Ok(())
    }
}

const fn severity_name(s: Severity) -> &'static str {
    match s {
        Severity::Info => "info",
        Severity::Error => "error",
        Severity::Critical => "critical",
        Severity::Fatal => "fatal",
    }
}

const fn policy_name(d: RecoveryDecision) -> &'static str {
    match d {
        RecoveryDecision::Kill => "kill",
        RecoveryDecision::Restart => "restart",
        RecoveryDecision::Suspend => "suspend",
        RecoveryDecision::Quarantine => "quarantine",
        RecoveryDecision::Escalate => "escalate",
    }
}

/// Render an event into the stable line format
/// (docs/11_RUNTIME_MONITORING.md §4). Never panics; on overflow the
/// line ends with the explicit `!truncated` marker.
pub fn render(event: &MonitorEvent) -> EventLine {
    let mut line = EventLine {
        buf: [0; LINE_CAPACITY],
        len: 0,
        truncated: false,
    };
    // Reserve tail space for the truncation marker.
    const MARKER: &str = " !truncated";
    let writable = LINE_CAPACITY - MARKER.len();
    let mut w = BufWriter {
        buf: &mut line.buf[..writable],
        len: 0,
        overflow: false,
    };

    let mut result = write!(
        w,
        "EVT type={} ts={} task={} sev={} phase={}",
        event.event_type.name(),
        event.timestamp,
        event.task.as_u32(),
        severity_name(event.severity),
        event.phase.name(),
    );
    if result.is_ok() {
        if let Some(policy) = event.policy_result {
            result = write!(w, " policy={}", policy_name(policy));
        }
    }
    if result.is_ok() {
        if let Some(cap) = event.related_cap {
            result = write!(w, " cap={cap}");
        }
    }
    if result.is_ok() {
        if let Some(sys) = event.related_syscall {
            result = write!(w, " syscall={sys}");
        }
    }

    line.len = w.len;
    if result.is_err() {
        line.truncated = true;
        line.buf[line.len..line.len + MARKER.len()].copy_from_slice(MARKER.as_bytes());
        line.len += MARKER.len();
    }
    line
}

/// Export one event through a sink (on target: `uart::put_str`; the
/// sink appends the newline handling of the serial driver).
pub fn export(event: &MonitorEvent, mut sink: impl FnMut(&str)) {
    let line = render(event);
    sink(line.as_str());
    sink("\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitor::event::{KernelPhase, MonitorEvent, MonitorEventType};
    use crate::thread::ThreadId;

    #[test]
    fn renders_mandatory_fields_in_stable_format() {
        let e = MonitorEvent::new(
            128,
            ThreadId(3),
            MonitorEventType::TaskFaulted,
            KernelPhase::Trap,
        );
        let line = render(&e);
        assert_eq!(
            line.as_str(),
            "EVT type=TASK_FAULTED ts=128 task=3 sev=error phase=trap"
        );
        assert!(!line.truncated());
    }

    #[test]
    fn optional_fields_appear_only_when_present() {
        let mut e = MonitorEvent::new(
            129,
            ThreadId(3),
            MonitorEventType::CapDenied,
            KernelPhase::Syscall,
        );
        e.related_cap = Some(0);
        e.related_syscall = Some(3);
        assert_eq!(
            render(&e).as_str(),
            "EVT type=CAP_DENIED ts=129 task=3 sev=error phase=syscall cap=0 syscall=3"
        );

        let mut r = MonitorEvent::new(
            130,
            ThreadId(3),
            MonitorEventType::RecoveryApplied,
            KernelPhase::Fault,
        );
        r.policy_result = Some(crate::fault::RecoveryDecision::Kill);
        assert_eq!(
            render(&r).as_str(),
            "EVT type=RECOVERY_APPLIED ts=130 task=3 sev=info phase=fault policy=kill"
        );
    }

    #[test]
    fn export_appends_newline_through_sink() {
        let e = MonitorEvent::new(
            1,
            ThreadId(0),
            MonitorEventType::TaskStarted,
            KernelPhase::Boot,
        );
        let mut out = String::new();
        export(&e, |s| out.push_str(s));
        assert_eq!(
            out,
            "EVT type=TASK_STARTED ts=1 task=0 sev=info phase=boot\n"
        );
    }

    #[test]
    fn worst_case_line_fits_without_truncation() {
        let mut e = MonitorEvent::new(
            u64::MAX,
            ThreadId(u32::MAX),
            MonitorEventType::WatchdogTimeout,
            KernelPhase::Syscall,
        );
        e.policy_result = Some(crate::fault::RecoveryDecision::Quarantine);
        e.related_cap = Some(u32::MAX);
        e.related_syscall = Some(u64::MAX);
        let line = render(&e);
        assert!(
            !line.truncated(),
            "capacity must cover the worst case: {}",
            line.as_str()
        );
        assert!(line.as_str().len() <= LINE_CAPACITY);
    }
}
