//! axiomctl — AxiomRT developer CLI.
//!
//! Requirement reference: docs/23_AXIOMCTL_CLI.md,
//! `AxiomrtFull Completion Mode.md` §10 (AXIOM-CLI-001..009).
//!
//! Host tooling only: wraps the authoritative repository scripts and
//! cargo commands; never re-implements a verification step. std only,
//! zero external dependencies.

mod events;

use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const USAGE: &str = "\
axiomctl — AxiomRT developer CLI

USAGE:
    cargo axiomctl <command> [args]
    cargo run --target x86_64-unknown-linux-gnu -p axiomctl -- <command>

COMMANDS:
    doctor           check toolchain, QEMU, Coq, and repository layout
    build            build the release kernel (default features)
    run              build and boot the kernel in QEMU (exit: Ctrl-A x)
    demo memory      run the memory-isolation demo (default build)
    demo full        run the full four-task fault-containment demo
    verify           run the full verification sweep (QEMU + host + Coq)
    evidence list    list archived evidence versions
    evidence open <ver> [file]
                     list one evidence directory / print one file
    events parse <logfile>
                     parse a serial log into NDJSON events (docs/21)
    events summary <logfile>
                     per-category event counts for a serial log
    kit build        assemble the industrial evaluation kit under release/
    release check    release hygiene checklist
    version          print axiomctl and repository versions
    help             print this help
";

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let cmd: Vec<&str> = args.iter().map(String::as_str).collect();

    match cmd.as_slice() {
        [] | ["help"] | ["-h"] | ["--help"] => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        ["version"] => cmd_version(),
        ["doctor"] => cmd_doctor(),
        ["build"] => cmd_build(),
        ["run", rest @ ..] => cmd_run(rest),
        ["demo", "memory"] => cmd_demo(false),
        ["demo", "full"] => cmd_demo(true),
        ["verify"] => cmd_verify(),
        ["evidence", "list"] => cmd_evidence_list(),
        ["evidence", "open", rest @ ..] => cmd_evidence_open(rest),
        ["events", "parse", file] => cmd_events(file, false),
        ["events", "summary", file] => cmd_events(file, true),
        ["kit", "build"] => cmd_kit_build(),
        ["release", "check"] => cmd_release_check(),
        other => {
            eprintln!("axiomctl: unknown command: {}", other.join(" "));
            eprintln!("Run `cargo axiomctl help` for usage.");
            ExitCode::FAILURE
        }
    }
}

// ---------------------------------------------------------------------
// Repository discovery and process helpers
// ---------------------------------------------------------------------

/// Walk up from the current directory to the repository root, identified
/// by `scripts/verify_all.sh` next to `kernel/Cargo.toml`.
fn repo_root() -> Option<PathBuf> {
    let mut dir = env::current_dir().ok()?;
    loop {
        if dir.join("scripts/verify_all.sh").is_file()
            && dir.join("kernel/Cargo.toml").is_file()
        {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

fn require_repo_root() -> Result<PathBuf, ExitCode> {
    repo_root().ok_or_else(|| {
        eprintln!(
            "axiomctl: not inside an AxiomRT repository \
             (looked for scripts/verify_all.sh and kernel/Cargo.toml)"
        );
        ExitCode::FAILURE
    })
}

/// Run a command with inherited stdio; map its exit status to ours.
fn run_inherit(root: &Path, program: &str, args: &[&str]) -> ExitCode {
    match Command::new(program).args(args).current_dir(root).status() {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        Ok(status) => {
            eprintln!("axiomctl: `{program}` exited with {status}");
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("axiomctl: failed to start `{program}`: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Capture the first line of a command's stdout, if it runs at all.
fn first_line(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().next().unwrap_or("").trim();
    if line.is_empty() {
        None
    } else {
        Some(line.to_string())
    }
}

// ---------------------------------------------------------------------
// Subcommands
// ---------------------------------------------------------------------

fn cmd_version() -> ExitCode {
    println!("axiomctl {}", env!("CARGO_PKG_VERSION"));
    match require_repo_root() {
        Ok(root) => {
            let describe = Command::new("git")
                .args(["describe", "--tags", "--always", "--dirty"])
                .current_dir(&root)
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            println!("AxiomRT {describe} ({})", root.display());
            ExitCode::SUCCESS
        }
        Err(code) => code,
    }
}

fn cmd_doctor() -> ExitCode {
    // (label, program, args, required)
    let tools: &[(&str, &str, &[&str], bool)] = &[
        ("rustc", "rustc", &["--version"], true),
        ("cargo", "cargo", &["--version"], true),
        ("qemu-system-riscv64", "qemu-system-riscv64", &["--version"], true),
        ("coqc (optional)", "coqc", &["--version"], false),
    ];

    let mut failed = false;
    println!("axiomctl doctor");
    println!("---------------");

    for (label, program, args, required) in tools {
        match first_line(program, args) {
            Some(line) => println!("OK       {label}: {line}"),
            None if *required => {
                println!("MISSING  {label} (required)");
                failed = true;
            }
            None => println!("absent   {label} — proof recompilation unavailable"),
        }
    }

    // riscv target: only checkable when rustup manages the toolchain.
    match Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
    {
        Ok(out) if out.status.success() => {
            let installed = String::from_utf8_lossy(&out.stdout);
            if installed.lines().any(|l| l.trim() == "riscv64gc-unknown-none-elf") {
                println!("OK       target riscv64gc-unknown-none-elf installed");
            } else {
                println!(
                    "MISSING  target riscv64gc-unknown-none-elf \
                     (rustup target add riscv64gc-unknown-none-elf)"
                );
                failed = true;
            }
        }
        _ => println!("unknown  riscv64gc target (rustup not found; assuming vendored toolchain)"),
    }

    match require_repo_root() {
        Ok(root) => {
            for f in [
                "kernel/Cargo.toml",
                "kernel/linker.ld",
                "scripts/run_qemu.sh",
                "scripts/verify_all.sh",
                "scripts/build_eval_kit.sh",
                "tests/boot_smoke_test.sh",
            ] {
                if root.join(f).is_file() {
                    println!("OK       {f}");
                } else {
                    println!("MISSING  {f} (required)");
                    failed = true;
                }
            }
        }
        Err(code) => return code,
    }

    println!("---------------");
    if failed {
        println!("doctor: FAIL — fix the MISSING items above");
        ExitCode::FAILURE
    } else {
        println!("doctor: PASS");
        ExitCode::SUCCESS
    }
}

fn cmd_build() -> ExitCode {
    let root = match require_repo_root() {
        Ok(r) => r,
        Err(code) => return code,
    };
    // Default features, riscv64 target via .cargo/config.toml.
    run_inherit(&root, "cargo", &["build", "--release"])
}

fn cmd_run(extra: &[&str]) -> ExitCode {
    let root = match require_repo_root() {
        Ok(r) => r,
        Err(code) => return code,
    };
    println!("axiomctl: booting QEMU (exit: Ctrl-A then x)");
    run_inherit(&root, "./scripts/run_qemu.sh", extra)
}

fn cmd_demo(full: bool) -> ExitCode {
    let root = match require_repo_root() {
        Ok(r) => r,
        Err(code) => return code,
    };

    if full {
        // run_qemu.sh rebuilds the default features, so the full demo
        // builds explicitly and boots QEMU itself (docs/23 §3).
        println!("axiomctl: building demo_full kernel");
        let build = run_inherit(
            &root,
            "cargo",
            &["build", "--release", "--features", "demo_full", "-p", "kernel"],
        );
        if build != ExitCode::SUCCESS {
            return build;
        }
        println!("axiomctl: booting full fault-containment demo (exit: Ctrl-A then x)");
        println!("axiomctl: note — run `axiomctl build` afterwards to restore the default kernel");
        // Keep in sync with scripts/run_qemu.sh.
        run_inherit(
            &root,
            "qemu-system-riscv64",
            &[
                "-machine", "virt",
                "-smp", "1",
                "-m", "128M",
                "-nographic",
                "-bios", "default",
                "-kernel", "target/riscv64gc-unknown-none-elf/release/kernel",
            ],
        )
    } else {
        // The memory-isolation demo is the default build.
        println!("axiomctl: memory-isolation demo is the default kernel build");
        cmd_run(&[])
    }
}

fn cmd_verify() -> ExitCode {
    let root = match require_repo_root() {
        Ok(r) => r,
        Err(code) => return code,
    };
    run_inherit(&root, "./scripts/verify_all.sh", &[])
}

fn cmd_evidence_list() -> ExitCode {
    let root = match require_repo_root() {
        Ok(r) => r,
        Err(code) => return code,
    };
    let evidence = root.join("evidence");
    let mut versions: Vec<String> = match std::fs::read_dir(&evidence) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .filter_map(|e| e.file_name().into_string().ok())
            .collect(),
        Err(e) => {
            eprintln!("axiomctl: cannot read {}: {e}", evidence.display());
            return ExitCode::FAILURE;
        }
    };
    versions.sort();
    println!("evidence archives ({}):", evidence.display());
    for v in &versions {
        let count = std::fs::read_dir(evidence.join(v))
            .map(|d| d.filter_map(|e| e.ok()).count())
            .unwrap_or(0);
        println!("  {v}  ({count} files)");
    }
    println!("open one with: axiomctl evidence open <version> [file]");
    ExitCode::SUCCESS
}

fn cmd_evidence_open(rest: &[&str]) -> ExitCode {
    let root = match require_repo_root() {
        Ok(r) => r,
        Err(code) => return code,
    };
    let (version, file) = match rest {
        [v] => (*v, None),
        [v, f] => (*v, Some(*f)),
        _ => {
            eprintln!("usage: axiomctl evidence open <version> [file]");
            return ExitCode::FAILURE;
        }
    };
    let dir = root.join("evidence").join(version);
    if !dir.is_dir() {
        eprintln!("axiomctl: no evidence directory {}", dir.display());
        return ExitCode::FAILURE;
    }
    match file {
        Some(f) => {
            let path = dir.join(f);
            match std::fs::read_to_string(&path) {
                Ok(text) => {
                    print!("{text}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("axiomctl: cannot read {}: {e}", path.display());
                    ExitCode::FAILURE
                }
            }
        }
        None => {
            println!("{}:", dir.display());
            let mut names: Vec<String> = std::fs::read_dir(&dir)
                .map(|d| {
                    d.filter_map(|e| e.ok())
                        .filter_map(|e| e.file_name().into_string().ok())
                        .collect()
                })
                .unwrap_or_default();
            names.sort();
            for n in names {
                println!("  {n}");
            }
            println!("print one with: axiomctl evidence open {version} <file>");
            ExitCode::SUCCESS
        }
    }
}

fn cmd_events(file: &str, want_summary: bool) -> ExitCode {
    let text = match std::fs::read_to_string(file) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("axiomctl: cannot read {file}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let log = events::parse_log(&text);
    // NDJSON is meant to be piped (head, jq); a closed consumer is a
    // normal way to stop, not a panic.
    use std::io::Write;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let result = if want_summary {
        write!(out, "{}", events::summary(&log))
    } else {
        log.events
            .iter()
            .try_for_each(|ev| writeln!(out, "{}", events::to_json(ev)))
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("axiomctl: write failed: {e}");
            ExitCode::FAILURE
        }
    }
}

fn cmd_kit_build() -> ExitCode {
    let root = match require_repo_root() {
        Ok(r) => r,
        Err(code) => return code,
    };
    run_inherit(&root, "./scripts/build_eval_kit.sh", &[])
}

fn cmd_release_check() -> ExitCode {
    let root = match require_repo_root() {
        Ok(r) => r,
        Err(code) => return code,
    };
    let mut failed = false;
    let mut check = |ok: bool, label: &str| {
        println!("{}  {label}", if ok { "PASS" } else { "FAIL" });
        if !ok {
            failed = true;
        }
    };

    // 1. Clean git tree.
    let clean = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&root)
        .output()
        .map(|o| o.status.success() && o.stdout.is_empty())
        .unwrap_or(false);
    check(clean, "git tree is clean (no uncommitted changes)");

    // 2. HEAD is tagged.
    let tagged = Command::new("git")
        .args(["tag", "--points-at", "HEAD"])
        .current_dir(&root)
        .output()
        .map(|o| o.status.success() && !o.stdout.is_empty())
        .unwrap_or(false);
    check(tagged, "HEAD carries a release tag");

    // 3. The seven kit documents exist.
    for d in [
        "LIMITATIONS",
        "ASSUMPTIONS_OF_USE",
        "SAFETY_CONCEPT",
        "SECURITY_CONCEPT",
        "VERIFICATION_REPORT",
        "TEST_REPORT",
        "FINAL_REPORT",
    ] {
        let path = format!("kit/{d}.md");
        let ok = root.join(&path).is_file();
        check(ok, &path);
    }

    // 4. Clean verification log: present, passing, warning-free.
    let log_path = root.join("evidence/v1.0/verify_all_clean.log");
    match std::fs::read_to_string(&log_path) {
        Ok(log) => {
            check(
                log.contains("VERIFY ALL: PASS"),
                "verify_all_clean.log records VERIFY ALL: PASS",
            );
            check(
                !log.contains("warning"),
                "verify_all_clean.log contains zero warnings",
            );
        }
        Err(_) => {
            check(false, "evidence/v1.0/verify_all_clean.log exists");
        }
    }

    // 5. Authoritative scripts present.
    for s in [
        "scripts/run_qemu.sh",
        "scripts/verify_all.sh",
        "scripts/build_eval_kit.sh",
    ] {
        check(root.join(s).is_file(), s);
    }

    println!("---------------");
    if failed {
        println!("release check: FAIL");
        ExitCode::FAILURE
    } else {
        println!("release check: PASS");
        ExitCode::SUCCESS
    }
}
