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

// ---------------------------------------------------------------------
// Live QEMU session (AXIOM-STUDIO-001): one interactive os_boot boot
// driven only through the fixed scenario command table below. The
// serial log is the single source of truth — Studio derives state from
// parsed events and never invents system state.
// ---------------------------------------------------------------------

/// Serial log retention bound (docs/24 §6 / docs/36 §5.12: Studio must
/// not introduce unbounded history growth).
const SESSION_LOG_MAX: usize = 1024 * 1024;
/// Issued-command history bound.
const SESSION_CMDS_MAX: usize = 200;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SessionStatus {
    Idle,
    Building,
    Running,
    Stopped,
    Failed,
}

impl SessionStatus {
    fn name(self) -> &'static str {
        match self {
            SessionStatus::Idle => "idle",
            SessionStatus::Building => "building",
            SessionStatus::Running => "running",
            SessionStatus::Stopped => "stopped",
            SessionStatus::Failed => "failed",
        }
    }
}

struct Session {
    status: SessionStatus,
    log: String,
    stdin: Option<std::process::ChildStdin>,
    child: Option<std::process::Child>,
    commands: Vec<String>,
}

impl Default for Session {
    fn default() -> Self {
        Session {
            status: SessionStatus::Idle,
            log: String::new(),
            stdin: None,
            child: None,
            commands: Vec::new(),
        }
    }
}

/// How a scenario is presented and guarded in the UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScenarioKind {
    /// Read-only observation command.
    Observe,
    /// Normal state-changing action (run/load/restart/send).
    Action,
    /// Deliberate test fault — destructive by intent, marked in the UI.
    Fault,
}

impl ScenarioKind {
    fn name(self) -> &'static str {
        match self {
            ScenarioKind::Observe => "observe",
            ScenarioKind::Action => "action",
            ScenarioKind::Fault => "fault",
        }
    }
}

/// The complete scenario command whitelist. Every command string is a
/// documented shell command (docs/26, docs/27, docs/28, docs/29,
/// docs/31, docs/34) exercised verbatim by the QEMU test scripts. The
/// browser can only name a scenario key — it can never inject a free
/// command line into the target.
const SCENARIOS: &[(&str, &str, &str, ScenarioKind)] = &[
    ("help", "help", "List shell commands", ScenarioKind::Observe),
    (
        "version",
        "version",
        "Kernel version string",
        ScenarioKind::Observe,
    ),
    (
        "tasks",
        "tasks",
        "Task table (sys_info)",
        ScenarioKind::Observe,
    ),
    (
        "caps",
        "caps",
        "Per-task capability kinds",
        ScenarioKind::Observe,
    ),
    ("ipc", "ipc", "Endpoint states", ScenarioKind::Observe),
    (
        "memory",
        "memory",
        "Memory layout facts",
        ScenarioKind::Observe,
    ),
    ("uptime", "uptime", "Timer ticks", ScenarioKind::Observe),
    (
        "events",
        "events",
        "Kernel event ring",
        ScenarioKind::Observe,
    ),
    (
        "faults",
        "faults",
        "Fault/denial ring entries",
        ScenarioKind::Observe,
    ),
    ("ls", "ls", "List filesystem root", ScenarioKind::Observe),
    ("bin", "bin", "List /bin app records", ScenarioKind::Observe),
    (
        "storage_info",
        "storage info",
        "Storage geometry",
        ScenarioKind::Observe,
    ),
    (
        "drivers",
        "drivers",
        "Driver manager status",
        ScenarioKind::Observe,
    ),
    (
        "driver_info",
        "driver info block",
        "Block device info via driver",
        ScenarioKind::Observe,
    ),
    (
        "net_status",
        "net status",
        "Network service status",
        ScenarioKind::Observe,
    ),
    (
        "net_stats",
        "net stats",
        "Network TX/RX counters",
        ScenarioKind::Observe,
    ),
    (
        "run_hello",
        "run hello",
        "Run the hello application",
        ScenarioKind::Action,
    ),
    (
        "app_load_hello",
        "app load hello",
        "Load hello from storage-backed /bin",
        ScenarioKind::Action,
    ),
    (
        "app_state_hello",
        "app state hello",
        "Query loaded-app state",
        ScenarioKind::Observe,
    ),
    (
        "run_loaded_hello",
        "run loaded hello",
        "Run the loaded hello image",
        ScenarioKind::Action,
    ),
    (
        "app_unload_hello",
        "app unload hello",
        "Unload the hello image",
        ScenarioKind::Action,
    ),
    (
        "app_load_bad_magic",
        "app load invalid_bad_magic",
        "Loader rejects a corrupt image record",
        ScenarioKind::Action,
    ),
    (
        "net_send",
        "net send-test",
        "Send one synthetic 64-byte test packet",
        ScenarioKind::Action,
    ),
    (
        "net_rx",
        "net rx-count",
        "Synthetic RX counter",
        ScenarioKind::Observe,
    ),
    (
        "net_malformed",
        "net malformed",
        "Malformed network request is rejected",
        ScenarioKind::Action,
    ),
    (
        "run_fault_demo",
        "run fault_demo",
        "Capability-less app: denial + contained fault",
        ScenarioKind::Fault,
    ),
    (
        "run_demo",
        "run demo",
        "Faulty task: watchdog containment + recovery",
        ScenarioKind::Fault,
    ),
    (
        "driver_fault",
        "driver fault block",
        "Deliberate block-driver test fault",
        ScenarioKind::Fault,
    ),
    (
        "driver_restart",
        "driver restart block",
        "Restart the block driver",
        ScenarioKind::Action,
    ),
    (
        "net_fault",
        "net fault",
        "Deliberate network-driver test fault",
        ScenarioKind::Fault,
    ),
    (
        "net_restart",
        "net restart",
        "Restart the network driver",
        ScenarioKind::Action,
    ),
    (
        "shutdown",
        "shutdown",
        "Controlled shutdown (ends session)",
        ScenarioKind::Fault,
    ),
];

fn scenario(
    name: &str,
) -> Option<&'static (&'static str, &'static str, &'static str, ScenarioKind)> {
    SCENARIOS.iter().find(|(key, _, _, _)| *key == name)
}

/// Append a line to a bounded log, trimming the oldest bytes at a char
/// boundary once the bound is exceeded.
fn push_bounded(log: &mut String, line: &str, max: usize) {
    log.push_str(line);
    log.push('\n');
    if log.len() > max {
        let cut = log.len() - max;
        let start = (cut..log.len())
            .find(|&i| log.is_char_boundary(i))
            .unwrap_or(cut);
        *log = log[start..].to_string();
    }
}

struct State {
    busy: Option<&'static str>,
    demo: Job,
    verify: Job,
    kit: Job,
    fuzz: Job,
    session: Session,
    doctor: Vec<(String, String)>,
}

/// Which serial text the observation pages derive from: the live
/// session once one exists, else the last bounded demo run.
fn event_source(st: &State) -> (&'static str, &str) {
    if !st.session.log.is_empty() {
        ("live_session", &st.session.log)
    } else if !st.demo.log.is_empty() {
        ("demo_run", &st.demo.log)
    } else {
        ("none", "")
    }
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
        fuzz: Job::default(),
        session: Session::default(),
        doctor: doctor_info(&root),
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

/// Tool versions plus the repository state, gathered once at startup.
fn doctor_info(root: &Path) -> Vec<(String, String)> {
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
    let describe = Command::new("git")
        .args(["describe", "--tags", "--always", "--dirty"])
        .current_dir(root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    vec![
        ("repository".to_string(), describe),
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
        ("POST", "/api/run_fuzz") => api_start(&mut stream, state, root, "fuzz"),
        ("GET", "/api/events") => api_events(&mut stream, state),
        ("GET", "/api/policy") => respond(&mut stream, 200, "application/json", &policy_json()),
        ("POST", "/api/session/start") => api_session_start(&mut stream, state, root),
        ("POST", "/api/session/cmd") => api_session_cmd(&mut stream, state, query),
        ("POST", "/api/session/stop") => api_session_stop(&mut stream, state),
        ("GET", "/api/session") => api_session_state(&mut stream, state),
        ("GET", "/api/session/log") => {
            let log = tail(&state.lock().unwrap().session.log, 200_000);
            respond(&mut stream, 200, "text/plain; charset=utf-8", &log)
        }
        ("GET", "/api/log") => {
            let log = tail(&state.lock().unwrap().demo.log, 200_000);
            respond(&mut stream, 200, "text/plain; charset=utf-8", &log)
        }
        ("GET", "/api/fuzz_log") => {
            let log = tail(&state.lock().unwrap().fuzz.log, 200_000);
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
            | "/session"
            | "/scenarios"
            | "/tasks"
            | "/services"
            | "/scheduler"
            | "/faults"
            | "/ipc"
            | "/capabilities"
            | "/drivers"
            | "/network"
            | "/loader"
            | "/events"
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
        // Deterministic bounded fuzz evidence (docs/36 §6.1): the three
        // protocol targets with fixed seeds; each exits non-zero on any
        // KERNEL_INVARIANT_FAILURE, failing the whole job.
        "fuzz" => "for t in 'ipc 20260903' 'capability 20260903' 'syscall 20260904'; do \
             set -- $t; \
             echo \"=== fuzz target $1 seed $2 ===\"; \
             cargo run -q -p axiom-fuzz --target x86_64-unknown-linux-gnu -- \
             --fuzz-target $1 --seed $2 --iterations 10000 --max-len 128 2>&1 || exit 1; \
             done"
            .to_string(),
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
        "fuzz" => &mut st.fuzz,
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
        "{{\"busy\":{},\"demo\":{},\"verify\":{},\"kit\":{},\"fuzz\":{},\
         \"session\":\"{}\",\"doctor\":[{}]}}",
        st.busy
            .map(|b| format!("\"{b}\""))
            .unwrap_or_else(|| "null".to_string()),
        job(&st.demo),
        job(&st.verify),
        job(&st.kit),
        job(&st.fuzz),
        st.session.status.name(),
        doctor
    );
    respond(stream, 200, "application/json", &body)
}

// ---------------------------------------------------------------------
// Live session endpoints (AXIOM-STUDIO-001): build the os_boot kernel,
// boot it interactively under QEMU, and drive it exclusively through
// the fixed scenario whitelist.
// ---------------------------------------------------------------------

fn api_session_start(stream: &mut TcpStream, state: &Shared, root: &Path) -> std::io::Result<()> {
    {
        let mut st = state.lock().unwrap();
        if st.busy.is_some()
            || matches!(
                st.session.status,
                SessionStatus::Building | SessionStatus::Running
            )
        {
            return respond(
                stream,
                409,
                "application/json",
                "{\"started\":false,\"reason\":\"busy\"}",
            );
        }
        // The busy slot is held only while cargo builds; the running
        // QEMU session itself does not block other jobs.
        st.busy = Some("session-build");
        st.session = Session {
            status: SessionStatus::Building,
            ..Session::default()
        };
    }
    let state = Arc::clone(state);
    let root = root.to_path_buf();
    thread::spawn(move || run_session(&state, &root));
    respond(stream, 200, "application/json", "{\"started\":true}")
}

fn run_session(state: &Shared, root: &Path) {
    let build = Command::new("cargo")
        .args([
            "build",
            "--release",
            "--features",
            "os_boot",
            "-p",
            "kernel",
        ])
        .current_dir(root)
        .output();
    let built = matches!(&build, Ok(o) if o.status.success());
    {
        let mut st = state.lock().unwrap();
        st.busy = None;
        if !built {
            let detail = build
                .map(|o| String::from_utf8_lossy(&o.stderr).to_string())
                .unwrap_or_else(|e| e.to_string());
            push_bounded(
                &mut st.session.log,
                &format!("[studio] kernel build failed:\n{detail}"),
                SESSION_LOG_MAX,
            );
            st.session.status = SessionStatus::Failed;
            return;
        }
        push_bounded(
            &mut st.session.log,
            "[studio] os_boot kernel built; booting QEMU",
            SESSION_LOG_MAX,
        );
    }

    let child = Command::new("qemu-system-riscv64")
        .args([
            "-machine",
            "virt",
            "-smp",
            "1",
            "-m",
            "128M",
            "-nographic",
            "-bios",
            "default",
            "-kernel",
            "target/riscv64gc-unknown-none-elf/release/kernel",
        ])
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let mut child = match child {
        Ok(child) => child,
        Err(e) => {
            let mut st = state.lock().unwrap();
            push_bounded(
                &mut st.session.log,
                &format!("[studio] cannot start qemu-system-riscv64: {e}"),
                SESSION_LOG_MAX,
            );
            st.session.status = SessionStatus::Failed;
            return;
        }
    };
    let stdout = child.stdout.take();
    {
        let mut st = state.lock().unwrap();
        st.session.stdin = child.stdin.take();
        st.session.child = Some(child);
        st.session.status = SessionStatus::Running;
    }

    // Reader loop: serial output into the bounded session log.
    if let Some(out) = stdout {
        for line in BufReader::new(out).lines() {
            let Ok(line) = line else { break };
            let mut st = state.lock().unwrap();
            push_bounded(&mut st.session.log, &line, SESSION_LOG_MAX);
        }
    }
    // Serial EOF: QEMU exited (controlled shutdown or kill). Reap it.
    let mut st = state.lock().unwrap();
    st.session.stdin = None;
    if let Some(mut child) = st.session.child.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
    if st.session.status == SessionStatus::Running {
        st.session.status = SessionStatus::Stopped;
    }
    push_bounded(
        &mut st.session.log,
        "[studio] QEMU session ended",
        SESSION_LOG_MAX,
    );
}

fn api_session_cmd(stream: &mut TcpStream, state: &Shared, query: &str) -> std::io::Result<()> {
    let Some(name) = query_param(query, "name") else {
        return respond(stream, 400, "text/plain", "name required");
    };
    let Some((_, command, _, _)) = scenario(name) else {
        return respond(stream, 400, "text/plain", "unknown scenario");
    };
    let mut st = state.lock().unwrap();
    if st.session.status != SessionStatus::Running {
        return respond(
            stream,
            409,
            "application/json",
            "{\"sent\":false,\"reason\":\"no running session\"}",
        );
    }
    let Some(stdin) = st.session.stdin.as_mut() else {
        return respond(
            stream,
            409,
            "application/json",
            "{\"sent\":false,\"reason\":\"session input closed\"}",
        );
    };
    // The shell reads CR-terminated lines (as the QEMU test scripts do).
    let ok = stdin
        .write_all(command.as_bytes())
        .and_then(|_| stdin.write_all(b"\r"))
        .and_then(|_| stdin.flush())
        .is_ok();
    if ok {
        if st.session.commands.len() == SESSION_CMDS_MAX {
            st.session.commands.remove(0);
        }
        st.session.commands.push(command.to_string());
        respond(stream, 200, "application/json", "{\"sent\":true}")
    } else {
        respond(
            stream,
            409,
            "application/json",
            "{\"sent\":false,\"reason\":\"write failed\"}",
        )
    }
}

fn api_session_stop(stream: &mut TcpStream, state: &Shared) -> std::io::Result<()> {
    {
        let mut st = state.lock().unwrap();
        if st.session.status != SessionStatus::Running {
            return respond(
                stream,
                409,
                "application/json",
                "{\"stopped\":false,\"reason\":\"no running session\"}",
            );
        }
        // Ask the shell for a controlled shutdown (SBI system reset).
        if let Some(stdin) = st.session.stdin.as_mut() {
            let _ = stdin.write_all(b"shutdown\r");
            let _ = stdin.flush();
        }
    }
    // Fallback: if QEMU is still alive shortly after, kill it; the
    // reader thread then reaps it and marks the session Stopped.
    let state = Arc::clone(state);
    thread::spawn(move || {
        thread::sleep(std::time::Duration::from_secs(6));
        let mut st = state.lock().unwrap();
        if let Some(child) = st.session.child.as_mut() {
            let _ = child.kill();
        }
    });
    respond(stream, 200, "application/json", "{\"stopped\":true}")
}

fn api_session_state(stream: &mut TcpStream, state: &Shared) -> std::io::Result<()> {
    let st = state.lock().unwrap();
    let scenarios = SCENARIOS
        .iter()
        .map(|(key, command, label, kind)| {
            format!(
                "{{\"name\":\"{}\",\"command\":\"{}\",\"label\":\"{}\",\"kind\":\"{}\"}}",
                json_escape(key),
                json_escape(command),
                json_escape(label),
                kind.name()
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let commands = st
        .session
        .commands
        .iter()
        .map(|c| format!("\"{}\"", json_escape(c)))
        .collect::<Vec<_>>()
        .join(",");
    let body = format!(
        "{{\"status\":\"{}\",\"log_bytes\":{},\"commands\":[{}],\"scenarios\":[{}]}}",
        st.session.status.name(),
        st.session.log.len(),
        commands,
        scenarios
    );
    respond(stream, 200, "application/json", &body)
}

/// Task table derived from events (docs/24 §5.4): started tasks with
/// the last state the evidence supports. This is observed event state,
/// not direct kernel introspection.
fn derive_tasks(evs: &[Event]) -> Vec<(String, String)> {
    let mut tasks: Vec<(String, String)> = Vec::new();
    let field = |ev: &Event, k: &str| -> Option<String> {
        ev.fields
            .iter()
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.clone())
    };
    let set_or_push = |tasks: &mut Vec<(String, String)>, name: String, state: &str| match tasks
        .iter_mut()
        .find(|(n, _)| *n == name)
    {
        Some((_, s)) => *s = state.to_string(),
        None => tasks.push((name, state.to_string())),
    };
    let mut last_faulted: Option<String> = None;
    for ev in evs {
        match ev.kind.as_str() {
            "TASK_STARTED" => {
                if let Some(t) = field(ev, "task") {
                    tasks.push((t, "running".to_string()));
                }
            }
            // os_boot flow: services announce with SERVICE started=.
            "SERVICE" => {
                if let Some(t) = field(ev, "started") {
                    set_or_push(&mut tasks, t, "running");
                }
            }
            "TASK_EXITED" => {
                if let Some(t) = field(ev, "task") {
                    set_or_push(&mut tasks, t, "exited");
                }
            }
            "FAULT" | "TASK_FAULTED" => {
                if let Some(t) = field(ev, "task") {
                    set_or_push(&mut tasks, t.clone(), "faulted");
                    last_faulted = Some(t);
                }
            }
            "TASK_KILLED" => {
                if let Some(t) = field(ev, "task") {
                    set_or_push(&mut tasks, t, "killed");
                }
            }
            "TASK_RESTARTED" => {
                if let Some(t) = field(ev, "task") {
                    set_or_push(&mut tasks, t, "running");
                }
            }
            "RECOVERY_APPLIED" => {
                if let (Some(policy), Some(t)) = (field(ev, "policy"), last_faulted.clone()) {
                    if policy == "Kill" {
                        set_or_push(&mut tasks, t, "killed");
                    }
                }
            }
            _ => {}
        }
    }
    tasks
}

/// The boot-frozen service set (docs/25 §3): init plus the 15 table
/// entries. Used by the Services view so unobserved services render as
/// "not_observed" instead of silently disappearing.
const KNOWN_SERVICES: [&str; 16] = [
    "init_service",
    "supervisor_service",
    "logger_service",
    "console_service",
    "shell_service",
    "faulty_task",
    "app_loader_service",
    "hello",
    "fault_demo",
    "counter",
    "fs_service",
    "storage_service",
    "driver_manager",
    "block_driver_service",
    "net_driver_service",
    "net_service",
];

/// Per-service observed lifecycle: (name, state, faults, restarts).
/// Faults count kernel FAULT/TASK_FAULTED containment lines; restarts
/// count TASK_RESTARTED plus repeated SERVICE started= re-arms.
fn derive_services(evs: &[Event]) -> Vec<(String, String, usize, usize)> {
    let states = derive_tasks(evs);
    fn field<'a>(ev: &'a Event, k: &str) -> Option<&'a str> {
        ev.fields
            .iter()
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.as_str())
    }
    KNOWN_SERVICES
        .iter()
        .map(|name| {
            let state = states
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, s)| s.clone())
                .unwrap_or_else(|| "not_observed".to_string());
            let mut faults = 0usize;
            let mut restarts = 0usize;
            let mut starts = 0usize;
            for ev in evs {
                match ev.kind.as_str() {
                    "FAULT" | "TASK_FAULTED" if field(ev, "task") == Some(name) => faults += 1,
                    "TASK_RESTARTED" if field(ev, "task") == Some(name) => restarts += 1,
                    "SERVICE" if field(ev, "started") == Some(name) => {
                        starts += 1;
                        if starts > 1 {
                            restarts += 1;
                        }
                    }
                    _ => {}
                }
            }
            (name.to_string(), state, faults, restarts)
        })
        .collect()
}

/// Static boot capability policy (docs/25 §5, mirrored from the
/// os_boot.rs service table). This is the ARCHITECTURE model shown by
/// the Capabilities view — explicitly not live cap-table telemetry,
/// which the kernel does not export per slot.
fn policy_json() -> String {
    const POLICY: &[(&str, &[&str])] = &[
        (
            "init_service",
            &["control (task start/kill/restart/shutdown)"],
        ),
        (
            "supervisor_service",
            &["endpoint fault_channel (recv, control)"],
        ),
        ("logger_service", &["endpoint event_channel (recv)"]),
        (
            "console_service",
            &["endpoint line_channel (send)", "console (recv, send)"],
        ),
        (
            "shell_service",
            &[
                "endpoint line_channel (recv)",
                "console (send)",
                "info (read-only introspection)",
                "control (task start/kill/restart/shutdown)",
                "endpoint app_channel (send, recv)",
                "endpoint fs_channel (send, recv, fs_read, fs_list)",
                "endpoint storage_channel (send, recv, storage_info, storage_read)",
                "endpoint driver_mgr_channel (send, recv)",
                "endpoint net_channel (send, recv, net_status, net_tx, net_rx, net_control)",
            ],
        ),
        ("faulty_task", &[]),
        (
            "app_loader_service",
            &[
                "endpoint app_channel (recv, send)",
                "control (task start/kill/restart/shutdown)",
                "endpoint fs_channel (send, recv, fs_read)",
                "console (send)",
                "info (read-only introspection)",
            ],
        ),
        ("hello", &["console (send)"]),
        ("fault_demo", &[]),
        ("counter", &["console (send)"]),
        (
            "fs_service",
            &[
                "endpoint fs_channel (recv, send)",
                "endpoint storage_channel (send, recv, storage_read)",
            ],
        ),
        (
            "storage_service",
            &["endpoint storage_channel (recv, send)"],
        ),
        (
            "driver_manager",
            &[
                "endpoint driver_mgr_channel (recv, send)",
                "endpoint block_cmd_channel (send, recv)",
                "control (task start/kill/restart/shutdown)",
                "console (send)",
                "device block0 (driver_control)",
                "endpoint net_drv_channel (send, recv)",
                "device net0 (driver_control)",
            ],
        ),
        (
            "block_driver_service",
            &[
                "endpoint block_cmd_channel (recv, send)",
                "device block0 (info, mmio_read, dma_read, dma_write, irq_receive)",
                "endpoint driver_irq_channel (recv)",
            ],
        ),
        (
            "net_driver_service",
            &[
                "endpoint net_drv_channel (recv, send)",
                "device net0 (info, irq_receive, network_driver)",
                "endpoint net_irq_channel (recv)",
                "console (send)",
            ],
        ),
        (
            "net_service",
            &[
                "endpoint net_channel (recv, send, net_status, net_tx, net_rx, net_control)",
                "endpoint net_drv_channel (send, recv)",
                "endpoint driver_mgr_channel (send, recv)",
                "console (send)",
            ],
        ),
    ];
    let services = POLICY
        .iter()
        .map(|(name, caps)| {
            let caps = caps
                .iter()
                .map(|c| format!("\"{}\"", json_escape(c)))
                .collect::<Vec<_>>()
                .join(",");
            format!("{{\"name\":\"{}\",\"caps\":[{caps}]}}", json_escape(name))
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"source\":\"boot policy (docs/25 \\u00a75, mirrored from os_boot.rs) — \
         static architecture model, not live cap-table telemetry\",\
         \"services\":[{services}]}}"
    )
}

#[derive(Debug, PartialEq, Eq)]
struct NetworkView {
    state: String,
    tx: u64,
    rx: u64,
    mode: String,
    faults: usize,
    restarts: usize,
    last_event: String,
}

impl Default for NetworkView {
    fn default() -> Self {
        Self {
            state: "not_observed".to_string(),
            tx: 0,
            rx: 0,
            mode: "not_observed".to_string(),
            faults: 0,
            restarts: 0,
            last_event: "no network event in this log".to_string(),
        }
    }
}

fn event_field<'a>(ev: &'a Event, key: &str) -> Option<&'a str> {
    ev.fields
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.as_str())
}

/// Derive the latest bounded synthetic-network view from structured
/// events. Restart resets counters because both U-mode tasks re-enter
/// with zeroed service/driver state (docs/34 section 8).
fn derive_network(evs: &[Event]) -> NetworkView {
    let mut view = NetworkView::default();
    for ev in evs.iter().filter(|ev| ev.category == Category::Network) {
        view.last_event = ev.raw.clone();
        match ev.kind.as_str() {
            "NET_SERVICE" => {
                if let Some(state) = event_field(ev, "state") {
                    view.state = state.to_string();
                }
                if let Some(mode) = event_field(ev, "mode") {
                    view.mode = mode.to_string();
                }
            }
            "NET_DRIVER" => {
                if event_field(ev, "state") == Some("faulted") {
                    view.state = "faulted".to_string();
                    view.faults += 1;
                } else if event_field(ev, "restarted").is_some() {
                    view.state = "up".to_string();
                    view.tx = 0;
                    view.rx = 0;
                    view.restarts += 1;
                } else if event_field(ev, "started").is_some() {
                    view.state = "up".to_string();
                }
            }
            "NET_TX" => {
                view.tx = event_field(ev, "tx")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(view.tx);
            }
            "NET_RX" => {
                view.rx = event_field(ev, "rx")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(view.rx);
            }
            _ => {}
        }
    }
    view
}

fn api_events(stream: &mut TcpStream, state: &Shared) -> std::io::Result<()> {
    const EVENT_CAP: usize = 1500;
    let (source, log_text) = {
        let st = state.lock().unwrap();
        let (source, text) = event_source(&st);
        (source, text.to_string())
    };
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
    let services_json = derive_services(&parsed.events)
        .iter()
        .map(|(name, state, faults, restarts)| {
            format!(
                "[\"{}\",\"{}\",{faults},{restarts}]",
                json_escape(name),
                json_escape(state)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let events_json = parsed
        .events
        .iter()
        .take(EVENT_CAP)
        .map(events::to_json)
        .collect::<Vec<_>>()
        .join(",");
    let network = derive_network(&parsed.events);

    let body = format!(
        "{{\"source\":\"{source}\",\"total\":{},\"skipped\":{},\"shown\":{},\
         \"summary\":[{}],\"sched\":[{}],\"tasks\":[{}],\"services\":[{}],\
         \"network\":{{\"state\":\"{}\",\
         \"tx\":{},\"rx\":{},\"mode\":\"{}\",\"faults\":{},\"restarts\":{},\
         \"last_event\":\"{}\"}},\"events\":[{}]}}",
        parsed.events.len(),
        parsed.skipped,
        parsed.events.len().min(EVENT_CAP),
        summary,
        sched_json,
        tasks_json,
        services_json,
        json_escape(&network.state),
        network.tx,
        network.rx,
        json_escape(&network.mode),
        network.faults,
        network.restarts,
        json_escape(&network.last_event),
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
            "/drivers",
            "/network",
            "/loader",
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
    fn network_view_derives_state_counters_and_lifecycle() {
        let log = events::parse_log(
            "NET_DRIVER started=net_driver_service\n\
             NET_SERVICE state=up mode=synthetic\n\
             NET_TX bytes=64 tx=2\n\
             NET_RX rx=2\n\
             NET_DENIED reason=malformed\n\
             NET_DRIVER state=faulted\n\
             NET_DRIVER restarted=net_driver_service\n\
             NET_TX bytes=64 tx=1\n\
             NET_RX rx=1\n",
        );
        let network = derive_network(&log.events);
        assert_eq!(network.state, "up");
        assert_eq!(network.mode, "synthetic");
        assert_eq!(network.tx, 1);
        assert_eq!(network.rx, 1);
        assert_eq!(network.faults, 1);
        assert_eq!(network.restarts, 1);
        assert_eq!(network.last_event, "NET_RX rx=1");
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

    #[test]
    fn new_console_pages_route() {
        for p in ["/session", "/scenarios", "/services", "/events"] {
            assert!(page_route(p), "page {p} must serve the shell");
        }
    }

    // Every scenario key is unique and every command is a documented
    // shell command form (docs/26/27/28/29/31/34) — the browser can
    // never reach an unlisted command line.
    #[test]
    fn scenario_whitelist_is_closed_and_documented() {
        const ALLOWED: [&str; 24] = [
            "help",
            "version",
            "tasks",
            "caps",
            "ipc",
            "memory",
            "uptime",
            "events",
            "faults",
            "ls",
            "bin",
            "storage info",
            "drivers",
            "driver info block",
            "driver fault block",
            "driver restart block",
            "net status",
            "net stats",
            "net send-test",
            "net rx-count",
            "net malformed",
            "net fault",
            "net restart",
            "shutdown",
        ];
        for (index, (key, command, label, _)) in SCENARIOS.iter().enumerate() {
            assert!(!label.is_empty(), "scenario {key} needs a label");
            let documented = ALLOWED.contains(command)
                || command.starts_with("run ")
                || command.starts_with("app load ")
                || command.starts_with("app unload ")
                || command.starts_with("app state ")
                || command.starts_with("run loaded ");
            assert!(
                documented,
                "scenario {key} uses undocumented command {command:?}"
            );
            for (other_key, other_command, _, _) in SCENARIOS.iter().skip(index + 1) {
                assert_ne!(key, other_key, "duplicate scenario key");
                assert_ne!(command, other_command, "duplicate scenario command");
            }
        }
        assert!(scenario("net_fault").is_some());
        assert!(scenario("rm -rf").is_none());
        assert!(scenario("").is_none());
        // Deliberate test faults are marked so the UI can warn.
        for key in [
            "run_fault_demo",
            "run_demo",
            "driver_fault",
            "net_fault",
            "shutdown",
        ] {
            assert_eq!(scenario(key).unwrap().3, ScenarioKind::Fault, "{key}");
        }
    }

    #[test]
    fn session_log_stays_bounded() {
        let mut log = String::new();
        for i in 0..100 {
            push_bounded(&mut log, &format!("line {i} {}", "x".repeat(64)), 1024);
            assert!(log.len() <= 1024 + 1, "log grew past its bound");
        }
        assert!(log.contains("line 99"), "newest lines are retained");
        assert!(!log.contains("line 0 "), "oldest lines are trimmed");
    }

    #[test]
    fn event_source_prefers_live_session() {
        let mut st = State {
            busy: None,
            demo: Job::default(),
            verify: Job::default(),
            kit: Job::default(),
            fuzz: Job::default(),
            session: Session::default(),
            doctor: Vec::new(),
        };
        assert_eq!(event_source(&st).0, "none");
        st.demo.log = "TASK_STARTED task=a\n".to_string();
        assert_eq!(event_source(&st).0, "demo_run");
        st.session.log = "SERVICE started=shell_service\n".to_string();
        let (source, text) = event_source(&st);
        assert_eq!(source, "live_session");
        assert!(text.contains("shell_service"));
    }

    #[test]
    fn os_flow_lifecycle_updates_tasks_and_services() {
        let log = events::parse_log(
            "SERVICE started=net_driver_service\n\
             SERVICE started=net_service\n\
             SERVICE started=shell_service\n\
             FAULT type=PageFault task=net_driver_service\n\
             TASK_RESTARTED task=net_driver_service\n\
             TASK_KILLED task=hello\n",
        );
        let tasks = derive_tasks(&log.events);
        assert!(tasks.contains(&("net_service".to_string(), "running".to_string())));
        assert!(tasks.contains(&("net_driver_service".to_string(), "running".to_string())));
        assert!(tasks.contains(&("hello".to_string(), "killed".to_string())));

        let services = derive_services(&log.events);
        let net_driver = services
            .iter()
            .find(|(n, _, _, _)| n == "net_driver_service")
            .unwrap();
        assert_eq!(net_driver.1, "running");
        assert_eq!(net_driver.2, 1, "one contained fault");
        assert_eq!(net_driver.3, 1, "one restart");
        let unobserved = services
            .iter()
            .find(|(n, _, _, _)| n == "storage_service")
            .unwrap();
        assert_eq!(unobserved.1, "not_observed");
        assert_eq!(services.len(), KNOWN_SERVICES.len());
    }

    #[test]
    fn repeated_service_start_counts_as_restart() {
        let log = events::parse_log(
            "SERVICE started=hello\n\
             TASK_KILLED task=hello\n\
             SERVICE started=hello\n",
        );
        let services = derive_services(&log.events);
        let hello = services.iter().find(|(n, _, _, _)| n == "hello").unwrap();
        assert_eq!(hello.1, "running");
        assert_eq!(hello.3, 1, "re-arm counts as a restart");
    }

    // The static policy mirror must stay aligned with the boot-frozen
    // service set and its two deny-by-default anchors.
    #[test]
    fn boot_policy_covers_all_services_and_denies_by_default() {
        let policy = policy_json();
        for name in KNOWN_SERVICES {
            assert!(policy.contains(name), "policy missing service {name}");
        }
        assert!(policy.contains("\"name\":\"fault_demo\",\"caps\":[]"));
        assert!(policy.contains("\"name\":\"faulty_task\",\"caps\":[]"));
        // v1.5 decision: nobody holds mmio_write (docs/31 §10).
        assert!(!policy.contains("mmio_write"));
        assert!(policy.contains("not live cap-table telemetry"));
    }
}
