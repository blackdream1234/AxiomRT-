//! Serial event log parser and JSON export.
//!
//! Requirement reference: docs/21_EVENT_FORMAT.md
//! (AXIOM-EVENT-002/003/004). Parses the kernel's structured serial
//! lines into categorized events, losslessly (`raw` always kept).
//! std only, zero external dependencies; JSON writing is hand-rolled.

/// Gate categories (docs/21 §2.1) plus the auxiliary ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Task,
    Scheduler,
    Syscall,
    Ipc,
    Capability,
    Fault,
    Watchdog,
    Recovery,
    Timer,
    Service,
    Boot,
}

impl Category {
    pub fn name(self) -> &'static str {
        match self {
            Category::Task => "task",
            Category::Scheduler => "scheduler",
            Category::Syscall => "syscall",
            Category::Ipc => "ipc",
            Category::Capability => "capability",
            Category::Fault => "fault",
            Category::Watchdog => "watchdog",
            Category::Recovery => "recovery",
            Category::Timer => "timer",
            Category::Service => "service",
            Category::Boot => "boot",
        }
    }
}

/// One parsed serial event (docs/21 §3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub seq: usize,
    pub category: Category,
    pub kind: String,
    pub flags: Vec<String>,
    pub fields: Vec<(String, String)>,
    pub raw: String,
}

/// Result of parsing a whole log.
#[derive(Debug, Default)]
pub struct ParsedLog {
    pub events: Vec<Event>,
    pub skipped: usize,
}

/// Category for an effective kind, given its fields (docs/21 §2.1).
/// `FAULT type=WatchdogTimeout` counts as watchdog, other faults as
/// fault. Unknown kinds return None (line is skipped, never guessed).
fn categorize(kind: &str, fields: &[(String, String)]) -> Option<Category> {
    let field = |k: &str| fields.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
    Some(match kind {
        "TASK_STARTED" | "TASK_EXITED" | "TASK_FAULTED" => Category::Task,
        "SCHED" => Category::Scheduler,
        "SYSCALL" => Category::Syscall,
        "IPC" | "IPC_DENIED" => Category::Ipc,
        "CAP_DENIED" => Category::Capability,
        "FAULT" => {
            if field("type") == Some("WatchdogTimeout") {
                Category::Watchdog
            } else {
                Category::Fault
            }
        }
        "PAGE_FAULT" | "DEADLINE_MISSED" | "CONTAIN" => Category::Fault,
        "WATCHDOG_TIMEOUT" => Category::Watchdog,
        "RECOVERY_APPLIED" => Category::Recovery,
        "TIMER" => Category::Timer,
        "SUPERVISOR" | "LOGGER" => Category::Service,
        "MMU" | "BOOT" | "BOOT_INFO" => Category::Boot,
        _ => return None,
    })
}

/// Parse one serial line into an event (docs/21 §2). Returns None for
/// lines outside the documented vocabulary (they are skipped and
/// counted by `parse_log`, never guessed at). `seq` is assigned by the
/// caller.
pub fn parse_line(line: &str) -> Option<Event> {
    let trimmed = line.trim_end();
    if trimmed.trim().is_empty() {
        return None;
    }

    // Boot banner: a fixed sentence, not KIND key=value.
    if trimmed == "AxiomRT kernel booted" {
        return Some(Event {
            seq: 0,
            category: Category::Boot,
            kind: "BOOT".to_string(),
            flags: Vec::new(),
            fields: Vec::new(),
            raw: trimmed.to_string(),
        });
    }

    let mut tokens = trimmed.split_whitespace();
    let first = tokens.next()?;

    // Bare `key=value` boot lines (`arch=riscv64`, `phase=boot`).
    let (kind, rest): (String, Vec<&str>) = if let Some((k, v)) = first.split_once('=') {
        if k.is_empty() || v.is_empty() {
            return None;
        }
        let mut rest: Vec<&str> = vec![first];
        rest.extend(tokens);
        ("BOOT_INFO".to_string(), rest)
    } else {
        (first.to_string(), tokens.collect())
    };

    let mut flags = Vec::new();
    let mut fields = Vec::new();
    for tok in rest {
        match tok.split_once('=') {
            Some((k, v)) if !k.is_empty() => {
                fields.push((k.to_string(), v.to_string()));
            }
            _ => flags.push(tok.to_string()),
        }
    }

    // Monitor lines: `EVT type=<KIND> ...` — the effective kind is the
    // type= value; the type field itself is not repeated (docs/11 §4).
    let effective_kind = if kind == "EVT" {
        let pos = fields.iter().position(|(k, _)| k == "type")?;
        fields.remove(pos).1
    } else {
        kind
    };

    let category = categorize(&effective_kind, &fields)?;
    Some(Event {
        seq: 0,
        category,
        kind: effective_kind,
        flags,
        fields,
        raw: trimmed.to_string(),
    })
}

/// Parse a whole log; `seq` is 1-based over parsed events.
pub fn parse_log(text: &str) -> ParsedLog {
    let mut out = ParsedLog::default();
    for line in text.lines() {
        match parse_line(line) {
            Some(mut ev) => {
                ev.seq = out.events.len() + 1;
                out.events.push(ev);
            }
            None => {
                if !line.trim().is_empty() {
                    out.skipped += 1;
                }
            }
        }
    }
    out
}

/// JSON string escaping per RFC 8259 (quotes, backslashes, control
/// characters). Everything else passes through as UTF-8. Public so
/// Studio builds its API responses with the same escaping.
pub fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

/// One event as a single-line JSON object (docs/21 §3, NDJSON).
pub fn to_json(ev: &Event) -> String {
    let flags = ev
        .flags
        .iter()
        .map(|f| format!("\"{}\"", json_escape(f)))
        .collect::<Vec<_>>()
        .join(",");
    let fields = ev
        .fields
        .iter()
        .map(|(k, v)| format!("\"{}\":\"{}\"", json_escape(k), json_escape(v)))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"seq\":{},\"category\":\"{}\",\"kind\":\"{}\",\"flags\":[{}],\"fields\":{{{}}},\"raw\":\"{}\"}}",
        ev.seq,
        ev.category.name(),
        json_escape(&ev.kind),
        flags,
        fields,
        json_escape(&ev.raw)
    )
}

/// Human-readable per-category / per-kind counts (docs/21 §3).
pub fn summary(log: &ParsedLog) -> String {
    const ORDER: [Category; 11] = [
        Category::Task,
        Category::Scheduler,
        Category::Syscall,
        Category::Ipc,
        Category::Capability,
        Category::Fault,
        Category::Watchdog,
        Category::Recovery,
        Category::Timer,
        Category::Service,
        Category::Boot,
    ];

    let mut out = format!(
        "events: {} parsed, {} non-event lines skipped\n",
        log.events.len(),
        log.skipped
    );
    for cat in ORDER {
        let of_cat: Vec<&Event> = log.events.iter().filter(|e| e.category == cat).collect();
        if of_cat.is_empty() {
            continue;
        }
        // Stable per-kind counts, first-seen order.
        let mut kinds: Vec<(String, usize)> = Vec::new();
        for ev in &of_cat {
            match kinds.iter_mut().find(|(k, _)| *k == ev.kind) {
                Some((_, n)) => *n += 1,
                None => kinds.push((ev.kind.clone(), 1)),
            }
        }
        let detail = kinds
            .iter()
            .map(|(k, n)| format!("{k} x{n}"))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "  {:<10} {:>7}  ({detail})\n",
            cat.name(),
            of_cat.len()
        ));
    }
    out
}

// ---------------------------------------------------------------------
// AXIOM-EVENT-004: tests against verbatim lines from a real demo_full
// QEMU run (docs/21 §4).
// ---------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    fn field<'a>(ev: &'a Event, k: &str) -> Option<&'a str> {
        ev.fields
            .iter()
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.as_str())
    }

    #[test]
    fn task_started_line() {
        let ev = parse_line("TASK_STARTED task=critical_task").unwrap();
        assert_eq!(ev.category, Category::Task);
        assert_eq!(ev.kind, "TASK_STARTED");
        assert_eq!(field(&ev, "task"), Some("critical_task"));
    }

    #[test]
    fn scheduler_line() {
        let ev = parse_line("SCHED selected=critical_task").unwrap();
        assert_eq!(ev.category, Category::Scheduler);
        assert_eq!(field(&ev, "selected"), Some("critical_task"));
    }

    #[test]
    fn ipc_flags_and_fields() {
        let ev =
            parse_line("IPC delivered fault_event to=supervisor_task from=faulty_task").unwrap();
        assert_eq!(ev.category, Category::Ipc);
        assert_eq!(ev.flags, vec!["delivered", "fault_event"]);
        assert_eq!(field(&ev, "to"), Some("supervisor_task"));
        assert_eq!(field(&ev, "from"), Some("faulty_task"));
    }

    #[test]
    fn capability_denial() {
        let ev = parse_line("CAP_DENIED task=faulty_task reason=no_valid_capability").unwrap();
        assert_eq!(ev.category, Category::Capability);
        assert_eq!(field(&ev, "reason"), Some("no_valid_capability"));
    }

    #[test]
    fn watchdog_fault_is_watchdog_category() {
        let ev = parse_line("FAULT type=WatchdogTimeout task=faulty_task").unwrap();
        assert_eq!(ev.category, Category::Watchdog);
        assert_eq!(ev.kind, "FAULT");
    }

    #[test]
    fn other_fault_stays_fault_category() {
        let ev =
            parse_line("CONTAIN scope=user reason=watchdog_timeout action=faulted kernel=alive")
                .unwrap();
        assert_eq!(ev.category, Category::Fault);
        assert_eq!(field(&ev, "kernel"), Some("alive"));
    }

    #[test]
    fn recovery_line() {
        let ev = parse_line("RECOVERY_APPLIED policy=Kill").unwrap();
        assert_eq!(ev.category, Category::Recovery);
        assert_eq!(field(&ev, "policy"), Some("Kill"));
    }

    #[test]
    fn evt_monitor_line_uses_type_as_kind() {
        let ev =
            parse_line("EVT type=CAP_DENIED ts=129 task=3 sev=error phase=syscall cap=0").unwrap();
        assert_eq!(ev.kind, "CAP_DENIED");
        assert_eq!(ev.category, Category::Capability);
        assert_eq!(field(&ev, "ts"), Some("129"));
        assert_eq!(field(&ev, "type"), None, "type consumed as kind");
    }

    #[test]
    fn boot_banner_and_bare_fields() {
        assert_eq!(
            parse_line("AxiomRT kernel booted").unwrap().category,
            Category::Boot
        );
        let ev = parse_line("arch=riscv64").unwrap();
        assert_eq!(ev.kind, "BOOT_INFO");
        assert_eq!(field(&ev, "arch"), Some("riscv64"));
    }

    #[test]
    fn foreign_lines_are_skipped_not_guessed() {
        assert!(parse_line("Platform Name               : riscv-virtio,qemu").is_none());
        assert!(parse_line("OpenSBI v1.8").is_none());
        assert!(parse_line("").is_none());
        assert!(parse_line("   Compiling kernel v0.1.0").is_none());
    }

    #[test]
    fn parse_log_sequences_and_counts_skips() {
        let log = parse_log("OpenSBI v1.8\nTASK_STARTED task=a\n\nSCHED selected=a\nnoise here\n");
        assert_eq!(log.events.len(), 2);
        assert_eq!(log.events[0].seq, 1);
        assert_eq!(log.events[1].seq, 2);
        assert_eq!(log.skipped, 2, "OpenSBI banner + noise; blank not counted");
    }

    #[test]
    fn json_is_lossless_and_escaped() {
        let mut ev = parse_line("CAP_DENIED task=faulty_task reason=no_valid_capability").unwrap();
        ev.seq = 17;
        let json = to_json(&ev);
        assert_eq!(
            json,
            "{\"seq\":17,\"category\":\"capability\",\"kind\":\"CAP_DENIED\",\
             \"flags\":[],\"fields\":{\"task\":\"faulty_task\",\
             \"reason\":\"no_valid_capability\"},\
             \"raw\":\"CAP_DENIED task=faulty_task reason=no_valid_capability\"}"
        );
        // Escaping: quotes and backslashes cannot corrupt the JSON.
        assert_eq!(json_escape("a\"b\\c\td"), "a\\\"b\\\\c\\td");
    }

    #[test]
    fn summary_covers_gate_categories() {
        let text = "TASK_STARTED task=a\nSCHED selected=a\nSYSCALL name=sys_yield task=a\n\
                    IPC recv task=a\nCAP_DENIED task=b reason=r\n\
                    FAULT type=WatchdogTimeout task=b\nCONTAIN scope=user kernel=alive\n\
                    RECOVERY_APPLIED policy=Kill\n";
        let log = parse_log(text);
        let s = summary(&log);
        for cat in [
            "task",
            "scheduler",
            "syscall",
            "ipc",
            "capability",
            "fault",
            "watchdog",
            "recovery",
        ] {
            assert!(s.contains(cat), "summary missing category {cat}:\n{s}");
        }
    }
}
