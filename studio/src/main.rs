//! AxiomRT Studio — local dashboard server.
//!
//! Requirement reference: docs/24_STUDIO.md (AXIOM-STUDIO-001..009).
//! std only; the single in-repo dependency is the axiomctl library
//! (shared docs/21 event parser). Binds 127.0.0.1 exclusively; file
//! endpoints accept single validated path components under fixed
//! roots (docs/24 §6).

use axiomctl::events::{self, json_escape, Category, Event};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

const HTML: &str = include_str!("page.html");
const DEMO_QEMU_SECONDS: u32 = 25;

#[derive(Clone, Copy, PartialEq, Eq)]
enum JobStatus {
    Idle,
    Running,
    Ok,
    Failed,
}

impl JobStatus {
    fn name(self) -> &'static str {
        match self {
            JobStatus::Idle => "idle",
            JobStatus::Running => "running",
            JobStatus::Ok => "ok",
            JobStatus::Failed => "failed",
        }
    }
}

#[derive(Default)]
struct Job {
    status: Option<JobStatus>,
    log: String,
}

struct State {
    busy: Option<&'static str>,
    demo: Job,
    verify: Job,
    kit: Job,
    doctor: Vec<(String, String)>,
}

type Shared = Arc<Mutex<State>>;

fn main() {
    let root = match axiomctl::repo_root() {
        Some(r) => r,
        None => {
            eprintln!("studio: not inside an AxiomRT repository");
            std::process::exit(1);
        }
    };
    let port: u16 = std::env::args()
        .nth(1)
        .and_then(|p| p.parse().ok())
        .unwrap_or(8787);

    let state: Shared = Arc::new(Mutex::new(State {
        busy: None,
        demo: Job::default(),
        verify: Job::default(),
        kit: Job::default(),
        doctor: doctor_info(),
    }));

    let listener = match TcpListener::bind(("127.0.0.1", port)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("studio: cannot bind 127.0.0.1:{port}: {e}");
            std::process::exit(1);
        }
    };
    println!("AxiomRT Studio: http://127.0.0.1:{port}/  (Ctrl-C to stop)");
    println!("local dashboard only — do not port-forward (docs/24 §6)");

    for conn in listener.incoming() {
        let Ok(stream) = conn else { continue };
        let state = Arc::clone(&state);
        let root = root.clone();
        thread::spawn(move || {
            let _ = handle(stream, &state, &root);
        });
    }
}

/// Tool versions for the status panel, gathered once at startup.
fn doctor_info() -> Vec<(String, String)> {
    let probe = |name: &str, prog: &str| {
        let line = Command::new(prog)
            .arg("--version")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string()
            })
            .unwrap_or_else(|| "missing".to_string());
        (name.to_string(), line)
    };
    vec![
        probe("rustc", "rustc"),
        probe("cargo", "cargo"),
        probe("qemu-system-riscv64", "qemu-system-riscv64"),
        probe("coqc", "coqc"),
    ]
}

// ---------------------------------------------------------------------
// HTTP plumbing
// ---------------------------------------------------------------------

fn handle(mut stream: TcpStream, state: &Shared, root: &Path) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let (method, target) = match parse_request_line(&request_line) {
        Some(mt) => mt,
        None => return respond(&mut stream, 400, "text/plain", "bad request"),
    };

    // Drain headers; discard any body (our POSTs are empty).
    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line)?;
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
            content_length = v.trim().parse().unwrap_or(0);
        }
    }
    if content_length > 0 {
        let mut sink = vec![0u8; content_length.min(64 * 1024)];
        let _ = reader.read_exact(&mut sink);
    }

    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p, q),
        None => (target.as_str(), ""),
    };

    match (method.as_str(), path) {
        ("GET", "/api/state") => api_state(&mut stream, state),
        ("POST", "/api/run_demo") => api_start(&mut stream, state, root, "demo"),
        ("POST", "/api/run_verify") => api_start(&mut stream, state, root, "verify"),
        ("POST", "/api/kit_build") => api_start(&mut stream, state, root, "kit"),
        ("GET", "/api/events") => api_events(&mut stream, state),
        ("GET", "/api/log") => {
            let log = tail(&state.lock().unwrap().demo.log, 200_000);
            respond(&mut stream, 200, "text/plain; charset=utf-8", &log)
        }
        ("GET", "/api/verify_log") => api_verify_log(&mut stream, state, root),
        ("GET", "/api/evidence") => api_evidence(&mut stream, root),
        ("GET", "/api/evidence/file") => api_evidence_file(&mut stream, root, query),
        ("GET", "/api/doc") => api_doc(&mut stream, root, query),
        ("GET", "/api/release_check") => api_release_check(&mut stream, root),
        ("GET", p) if page_route(p) => respond(&mut stream, 200, "text/html; charset=utf-8", HTML),
        _ => respond(&mut stream, 404, "text/plain", "not found"),
    }
}

fn parse_request_line(line: &str) -> Option<(String, String)> {
    let mut parts = line.split_whitespace();
    let method = parts.next()?;
    let target = parts.next()?;
    if !matches!(method, "GET" | "POST") || !target.starts_with('/') {
        return None;
    }
    Some((method.to_string(), target.to_string()))
}

/// The dashboard shell answers on every documented page path
/// (docs/24 §4); the client activates the matching panel.
fn page_route(path: &str) -> bool {
    matches!(
        path,
        "/" | "/run"
            | "/tasks"
            | "/scheduler"
            | "/faults"
            | "/ipc"
            | "/capabilities"
            | "/drivers"
            | "/tests"
            | "/proofs"
            | "/evidence"
            | "/limitations"
            | "/release"
    )
}

fn respond(stream: &mut TcpStream, code: u16, ctype: &str, body: &str) -> std::io::Result<()> {
    let reason = match code {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        409 => "Conflict",
        _ => "Error",
    };
    let head = format!(
        "HTTP/1.1 {code} {reason}\r\nContent-Type: {ctype}\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body.as_bytes())
}

fn query_param<'a>(query: &'a str, key: &str) -> Option<&'a str> {
    query
        .split('&')
        .filter_map(|kv| kv.split_once('='))
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v)
}

/// Single path component: alphanumerics plus `._-`, no leading dot.
/// Blocks `..`, separators, and hidden files by construction.
fn safe_name(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with('.')
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

fn tail(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let cut = s.len() - max;
    let start = (cut..s.len())
        .find(|&i| s.is_char_boundary(i))
        .unwrap_or(cut);
    format!(
        "[... truncated, showing last {max} bytes ...]\n{}",
        &s[start..]
    )
}

// ---------------------------------------------------------------------
// Background jobs (docs/24 §3): one at a time, output streamed into
// shared state so the page can poll it.
// ---------------------------------------------------------------------

fn api_start(
    stream: &mut TcpStream,
    state: &Shared,
    root: &Path,
    which: &'static str,
) -> std::io::Result<()> {
    {
        let mut st = state.lock().unwrap();
        if st.busy.is_some() {
            return respond(
                stream,
                409,
                "application/json",
                "{\"started\":false,\"reason\":\"busy\"}",
            );
        }
        st.busy = Some(which);
        let job = job_mut(&mut st, which);
        job.status = Some(JobStatus::Running);
        job.log.clear();
    }

    let shell = match which {
        // Build the demo kernel, boot QEMU under a bounded timeout
        // (the demo runs forever by design), restore default build.
        "demo" => format!(
            "cargo build --release --features demo_full -p kernel 2>&1 && \
             timeout {DEMO_QEMU_SECONDS} qemu-system-riscv64 -machine virt -smp 1 -m 128M \
             -nographic -bios default \
             -kernel target/riscv64gc-unknown-none-elf/release/kernel 2>&1; \
             cargo build --release >/dev/null 2>&1"
        ),
        "verify" => "./scripts/verify_all.sh 2>&1".to_string(),
        "kit" => "./scripts/build_eval_kit.sh 2>&1".to_string(),
        _ => unreachable!(),
    };

    let state = Arc::clone(state);
    let root = root.to_path_buf();
    thread::spawn(move || run_job(&state, &root, which, &shell));
    respond(stream, 200, "application/json", "{\"started\":true}")
}

fn job_mut<'a>(st: &'a mut State, which: &str) -> &'a mut Job {
    match which {
        "demo" => &mut st.demo,
        "verify" => &mut st.verify,
        _ => &mut st.kit,
    }
}

fn run_job(state: &Shared, root: &Path, which: &'static str, shell: &str) {
    let child = Command::new("sh")
        .args(["-c", shell])
        .current_dir(root)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();

    let ok = match child {
        Ok(mut child) => {
            if let Some(out) = child.stdout.take() {
                for line in BufReader::new(out).lines() {
                    let Ok(line) = line else { break };
                    let mut st = state.lock().unwrap();
                    let log = &mut job_mut(&mut st, which).log;
                    log.push_str(&line);
                    log.push('\n');
                }
            }
            let status_ok = child.wait().map(|s| s.success()).unwrap_or(false);
            if which == "demo" {
                // QEMU is stopped by `timeout` (exit 124) by design;
                // the demo succeeded if the kernel came up and the
                // recovery chain ran (docs/24 §8).
                let st = state.lock().unwrap();
                let log = &st.demo.log;
                log.contains("AxiomRT kernel booted") && log.contains("RECOVERY_APPLIED")
            } else {
                status_ok
            }
        }
        Err(_) => false,
    };

    let mut st = state.lock().unwrap();
    job_mut(&mut st, which).status = Some(if ok { JobStatus::Ok } else { JobStatus::Failed });
    st.busy = None;
}

// ---------------------------------------------------------------------
// JSON APIs
// ---------------------------------------------------------------------

fn api_state(stream: &mut TcpStream, state: &Shared) -> std::io::Result<()> {
    let st = state.lock().unwrap();
    let job = |j: &Job| {
        format!(
            "{{\"status\":\"{}\",\"log_bytes\":{}}}",
            j.status.unwrap_or(JobStatus::Idle).name(),
            j.log.len()
        )
    };
    let doctor = st
        .doctor
        .iter()
        .map(|(k, v)| format!("[\"{}\",\"{}\"]", json_escape(k), json_escape(v)))
        .collect::<Vec<_>>()
        .join(",");
    let body = format!(
        "{{\"busy\":{},\"demo\":{},\"verify\":{},\"kit\":{},\"doctor\":[{}]}}",
        st.busy
            .map(|b| format!("\"{b}\""))
            .unwrap_or_else(|| "null".to_string()),
        job(&st.demo),
        job(&st.verify),
        job(&st.kit),
        doctor
    );
    respond(stream, 200, "application/json", &body)
}

/// Task table derived from events (docs/24 §5.4): started tasks with
/// the last state the evidence supports.
fn derive_tasks(evs: &[Event]) -> Vec<(String, String)> {
    let mut tasks: Vec<(String, String)> = Vec::new();
    let field = |ev: &Event, k: &str| -> Option<String> {
        ev.fields
            .iter()
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.clone())
    };
    let mut last_faulted: Option<String> = None;
    for ev in evs {
        match ev.kind.as_str() {
            "TASK_STARTED" => {
                if let Some(t) = field(ev, "task") {
                    tasks.push((t, "running".to_string()));
                }
            }
            "TASK_EXITED" => {
                if let Some(t) = field(ev, "task") {
                    set_state(&mut tasks, &t, "exited");
                }
            }
            "FAULT" | "TASK_FAULTED" => {
                if let Some(t) = field(ev, "task") {
                    set_state(&mut tasks, &t, "faulted");
                    last_faulted = Some(t);
                }
            }
            "RECOVERY_APPLIED" => {
                if let (Some(policy), Some(t)) = (field(ev, "policy"), last_faulted.clone()) {
                    if policy == "Kill" {
                        set_state(&mut tasks, &t, "killed");
                    }
                }
            }
            _ => {}
        }
    }
    tasks
}

fn set_state(tasks: &mut [(String, String)], name: &str, state: &str) {
    if let Some((_, s)) = tasks.iter_mut().find(|(n, _)| n == name) {
        *s = state.to_string();
    }
}

fn api_events(stream: &mut TcpStream, state: &Shared) -> std::io::Result<()> {
    const EVENT_CAP: usize = 1500;
    let log_text = state.lock().unwrap().demo.log.clone();
    let parsed = events::parse_log(&log_text);

    // Per-kind counts per category, and scheduler selections per task.
    let mut kinds: Vec<(Category, String, usize)> = Vec::new();
    let mut sched: Vec<(String, usize)> = Vec::new();
    for ev in &parsed.events {
        match kinds
            .iter_mut()
            .find(|(c, k, _)| *c == ev.category && *k == ev.kind)
        {
            Some((_, _, n)) => *n += 1,
            None => kinds.push((ev.category, ev.kind.clone(), 1)),
        }
        if ev.category == Category::Scheduler {
            if let Some((_, v)) = ev.fields.iter().find(|(k, _)| k == "selected") {
                match sched.iter_mut().find(|(t, _)| t == v) {
                    Some((_, n)) => *n += 1,
                    None => sched.push((v.clone(), 1)),
                }
            }
        }
    }

    let summary = kinds
        .iter()
        .map(|(c, k, n)| format!("[\"{}\",\"{}\",{n}]", c.name(), json_escape(k)))
        .collect::<Vec<_>>()
        .join(",");
    let sched_json = sched
        .iter()
        .map(|(t, n)| format!("[\"{}\",{n}]", json_escape(t)))
        .collect::<Vec<_>>()
        .join(",");
    let tasks_json = derive_tasks(&parsed.events)
        .iter()
        .map(|(t, s)| format!("[\"{}\",\"{}\"]", json_escape(t), json_escape(s)))
        .collect::<Vec<_>>()
        .join(",");
    let events_json = parsed
        .events
        .iter()
        .take(EVENT_CAP)
        .map(events::to_json)
        .collect::<Vec<_>>()
        .join(",");

    let body = format!(
        "{{\"total\":{},\"skipped\":{},\"shown\":{},\"summary\":[{}],\
         \"sched\":[{}],\"tasks\":[{}],\"events\":[{}]}}",
        parsed.events.len(),
        parsed.skipped,
        parsed.events.len().min(EVENT_CAP),
        summary,
        sched_json,
        tasks_json,
        events_json
    );
    respond(stream, 200, "application/json", &body)
}

fn api_verify_log(stream: &mut TcpStream, state: &Shared, root: &Path) -> std::io::Result<()> {
    let live = state.lock().unwrap().verify.log.clone();
    let text = if live.is_empty() {
        std::fs::read_to_string(root.join("evidence/v1.0/verify_all_clean.log"))
            .map(|t| format!("[archived evidence/v1.0/verify_all_clean.log]\n{t}"))
            .unwrap_or_else(|_| "no verify log yet — run the sweep".to_string())
    } else {
        live
    };
    respond(
        stream,
        200,
        "text/plain; charset=utf-8",
        &tail(&text, 200_000),
    )
}

fn api_evidence(stream: &mut TcpStream, root: &Path) -> std::io::Result<()> {
    let dir = root.join("evidence");
    let mut versions: Vec<(String, Vec<String>)> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.filter_map(|e| e.ok()) {
            if !e.path().is_dir() {
                continue;
            }
            let Ok(name) = e.file_name().into_string() else {
                continue;
            };
            let mut files: Vec<String> = std::fs::read_dir(e.path())
                .map(|d| {
                    d.filter_map(|f| f.ok())
                        .filter_map(|f| f.file_name().into_string().ok())
                        .collect()
                })
                .unwrap_or_default();
            files.sort();
            versions.push((name, files));
        }
    }
    versions.sort();
    let body = format!(
        "{{\"versions\":[{}]}}",
        versions
            .iter()
            .map(|(v, files)| {
                let fs = files
                    .iter()
                    .map(|f| format!("\"{}\"", json_escape(f)))
                    .collect::<Vec<_>>()
                    .join(",");
                format!("{{\"name\":\"{}\",\"files\":[{fs}]}}", json_escape(v))
            })
            .collect::<Vec<_>>()
            .join(",")
    );
    respond(stream, 200, "application/json", &body)
}

fn api_evidence_file(stream: &mut TcpStream, root: &Path, query: &str) -> std::io::Result<()> {
    let (Some(ver), Some(file)) = (query_param(query, "ver"), query_param(query, "file")) else {
        return respond(stream, 400, "text/plain", "ver and file required");
    };
    if !safe_name(ver) || !safe_name(file) {
        return respond(stream, 400, "text/plain", "invalid name");
    }
    serve_file(stream, &root.join("evidence").join(ver).join(file))
}

fn api_doc(stream: &mut TcpStream, root: &Path, query: &str) -> std::io::Result<()> {
    let path: PathBuf = match query_param(query, "name") {
        Some("limitations") => root.join("kit/LIMITATIONS.md"),
        Some("assumptions") => root.join("kit/ASSUMPTIONS_OF_USE.md"),
        Some("final") => root.join("kit/FINAL_REPORT.md"),
        _ => return respond(stream, 400, "text/plain", "unknown doc"),
    };
    serve_file(stream, &path)
}

fn serve_file(stream: &mut TcpStream, path: &Path) -> std::io::Result<()> {
    match std::fs::read_to_string(path) {
        Ok(text) => respond(
            stream,
            200,
            "text/plain; charset=utf-8",
            &tail(&text, 400_000),
        ),
        Err(_) => respond(stream, 404, "text/plain", "no such file"),
    }
}

/// Release checklist via the axiomctl binary — same checks as the CLI
/// (docs/24 §4). Built on demand through the cargo alias.
fn api_release_check(stream: &mut TcpStream, root: &Path) -> std::io::Result<()> {
    let out = Command::new("sh")
        .args(["-c", "cargo axiomctl release check 2>&1"])
        .current_dir(root)
        .output();
    let (ok, text) = match out {
        Ok(o) => (
            o.status.success(),
            String::from_utf8_lossy(&o.stdout).to_string(),
        ),
        Err(e) => (false, format!("failed to run release check: {e}")),
    };
    let body = format!("{{\"ok\":{},\"text\":\"{}\"}}", ok, json_escape(&text));
    respond(stream, 200, "application/json", &body)
}

// ---------------------------------------------------------------------
// Tests (AXIOM-STUDIO gate support; run on host)
// ---------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_line_parsing() {
        assert_eq!(
            parse_request_line("GET /api/state HTTP/1.1\r\n"),
            Some(("GET".into(), "/api/state".into()))
        );
        assert_eq!(
            parse_request_line("POST /api/run_demo HTTP/1.1\r\n"),
            Some(("POST".into(), "/api/run_demo".into()))
        );
        assert!(parse_request_line("DELETE / HTTP/1.1\r\n").is_none());
        assert!(parse_request_line("GET nopath HTTP/1.1\r\n").is_none());
    }

    #[test]
    fn all_documented_pages_route() {
        for p in [
            "/",
            "/run",
            "/tasks",
            "/scheduler",
            "/faults",
            "/ipc",
            "/capabilities",
            "/tests",
            "/proofs",
            "/evidence",
            "/limitations",
            "/release",
        ] {
            assert!(page_route(p), "page {p} must serve the shell");
        }
        assert!(!page_route("/etc/passwd"));
    }

    #[test]
    fn evidence_names_are_validated() {
        assert!(safe_name("v0.9"));
        assert!(safe_name("verify_all_clean.log"));
        assert!(!safe_name("../secrets"));
        assert!(!safe_name("a/b"));
        assert!(!safe_name(".hidden"));
        assert!(!safe_name(""));
    }

    #[test]
    fn task_states_derive_from_demo_events() {
        let log = events::parse_log(
            "TASK_STARTED task=supervisor_task\n\
             TASK_STARTED task=faulty_task\n\
             TASK_STARTED task=critical_task\n\
             FAULT type=WatchdogTimeout task=faulty_task\n\
             RECOVERY_APPLIED policy=Kill\n\
             SCHED selected=critical_task\n",
        );
        let tasks = derive_tasks(&log.events);
        assert_eq!(
            tasks,
            vec![
                ("supervisor_task".to_string(), "running".to_string()),
                ("faulty_task".to_string(), "killed".to_string()),
                ("critical_task".to_string(), "running".to_string()),
            ]
        );
    }

    #[test]
    fn query_params_and_tail() {
        assert_eq!(
            query_param("ver=v0.9&file=log.txt", "file"),
            Some("log.txt")
        );
        assert_eq!(query_param("", "x"), None);
        assert_eq!(tail("abc", 10), "abc");
        assert!(tail(&"x".repeat(100), 10).contains("truncated"));
    }
}
