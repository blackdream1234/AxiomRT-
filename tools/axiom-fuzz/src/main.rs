//! axiom-fuzz command-line entry point.

use axiom_fuzz::limits::{MAX_INPUT_LEN, MAX_ITERATIONS};
use axiom_fuzz::targets::capability::{self, CapabilityTarget};
use axiom_fuzz::targets::ipc::{self, IpcTarget};
use axiom_fuzz::targets::smoke::{self, SmokeTarget};
use axiom_fuzz::targets::syscall::{self, SyscallTarget};
use axiom_fuzz::{Corpus, Engine, FailureArtifact, RunConfig};
use std::env;
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
axiom-fuzz — deterministic bounded AxiomRT fuzz harness

RUN:
    cargo run -p axiom-fuzz --target x86_64-unknown-linux-gnu -- \\
        --fuzz-target <smoke|ipc|capability|syscall> --seed <u64> --iterations <u64> --max-len <usize> \\
        [--corpus <directory>] [--failure-dir <directory>]

REPLAY:
    cargo run -p axiom-fuzz --target x86_64-unknown-linux-gnu -- \\
        --replay <failure-file>

RULES:
    --seed is required; no random or clock-derived default is used.
    max_len must be <= 1048576; iterations must be <= 10000000.
    Available targets: smoke, ipc, capability, syscall.
";

#[derive(Clone, Debug, Eq, PartialEq)]
enum Command {
    Help,
    Run {
        fuzz_target: String,
        seed: u64,
        iterations: u64,
        max_len: usize,
        corpus: Option<PathBuf>,
        failure_dir: PathBuf,
    },
    Replay {
        failure_file: PathBuf,
    },
}

fn main() -> ExitCode {
    match execute(env::args().skip(1).collect()) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("axiom-fuzz: {error}");
            eprintln!("Run with --help for usage.");
            ExitCode::FAILURE
        }
    }
}

fn execute(arguments: Vec<String>) -> io::Result<u8> {
    match parse_arguments(arguments)? {
        Command::Help => {
            print!("{USAGE}");
            Ok(0)
        }
        Command::Run {
            fuzz_target,
            seed,
            iterations,
            max_len,
            corpus,
            failure_dir,
        } => {
            if !matches!(
                fuzz_target.as_str(),
                smoke::NAME | ipc::NAME | capability::NAME | syscall::NAME
            ) {
                return Err(invalid_input(format!(
                    "unknown fuzz target {fuzz_target:?}; available targets: smoke, ipc, capability, syscall"
                )));
            }
            let corpus = match corpus {
                Some(path) => Corpus::load(&path, max_len)?,
                None => Corpus::empty(),
            };
            let config = RunConfig {
                target: fuzz_target,
                seed,
                iterations,
                max_len,
                failure_dir,
            };
            let summary = match config.target.as_str() {
                smoke::NAME => {
                    smoke::validate_infrastructure().map_err(invalid_input)?;
                    Engine.run(&config, corpus, &mut SmokeTarget)?
                }
                ipc::NAME => Engine.run(&config, corpus, &mut IpcTarget)?,
                capability::NAME => Engine.run(&config, corpus, &mut CapabilityTarget)?,
                syscall::NAME => Engine.run(&config, corpus, &mut SyscallTarget)?,
                _ => unreachable!("target validated above"),
            };
            print!("{}", summary.render());
            Ok(summary.exit_code())
        }
        Command::Replay { failure_file } => {
            let artifact = FailureArtifact::read(&failure_file)?;
            let summary = match artifact.target.as_str() {
                smoke::NAME => Engine.replay(&artifact, &mut SmokeTarget)?,
                ipc::NAME => Engine.replay(&artifact, &mut IpcTarget)?,
                capability::NAME => Engine.replay(&artifact, &mut CapabilityTarget)?,
                syscall::NAME => Engine.replay(&artifact, &mut SyscallTarget)?,
                _ => {
                    return Err(invalid_input(format!(
                        "no implementation is registered for replay target {:?}",
                        artifact.target
                    )))
                }
            };
            println!(
                "REPLAY target={} seed={} iteration={} input_len={}",
                artifact.target,
                artifact.seed,
                artifact.iteration,
                artifact.input.len()
            );
            print!("{}", summary.render());
            Ok(summary.exit_code())
        }
    }
}

fn parse_arguments(arguments: Vec<String>) -> io::Result<Command> {
    if arguments.is_empty()
        || matches!(
            arguments.as_slice(),
            [value] if matches!(value.as_str(), "-h" | "--help" | "help")
        )
    {
        return Ok(Command::Help);
    }

    if let [flag, failure_file] = arguments.as_slice() {
        if flag == "--replay" {
            return Ok(Command::Replay {
                failure_file: PathBuf::from(failure_file),
            });
        }
    }
    if arguments.iter().any(|argument| argument == "--replay") {
        return Err(invalid_input(
            "--replay must be used alone with exactly one failure file",
        ));
    }

    let mut fuzz_target = None;
    let mut seed = None;
    let mut iterations = None;
    let mut max_len = None;
    let mut corpus = None;
    let mut failure_dir = None;
    let mut index = 0;

    while index < arguments.len() {
        let option = &arguments[index];
        let value = arguments
            .get(index + 1)
            .ok_or_else(|| invalid_input(format!("missing value for {option}")))?;
        match option.as_str() {
            "--fuzz-target" => set_once(&mut fuzz_target, value.clone(), option)?,
            "--seed" => set_once(&mut seed, parse_u64(value, option)?, option)?,
            "--iterations" => set_once(&mut iterations, parse_u64(value, option)?, option)?,
            "--max-len" => set_once(&mut max_len, parse_usize(value, option)?, option)?,
            "--corpus" => set_once(&mut corpus, PathBuf::from(value), option)?,
            "--failure-dir" => set_once(&mut failure_dir, PathBuf::from(value), option)?,
            _ => return Err(invalid_input(format!("unknown option {option}"))),
        }
        index += 2;
    }

    let fuzz_target = fuzz_target.ok_or_else(|| invalid_input("--fuzz-target is required"))?;
    let seed = seed.ok_or_else(|| invalid_input("--seed is required; no default seed is used"))?;
    let iterations = iterations.ok_or_else(|| invalid_input("--iterations is required"))?;
    let max_len = max_len.ok_or_else(|| invalid_input("--max-len is required"))?;
    if iterations > MAX_ITERATIONS {
        return Err(invalid_input(format!(
            "--iterations {iterations} exceeds hard limit {MAX_ITERATIONS}"
        )));
    }
    if max_len > MAX_INPUT_LEN {
        return Err(invalid_input(format!(
            "--max-len {max_len} exceeds hard limit {MAX_INPUT_LEN}"
        )));
    }

    Ok(Command::Run {
        fuzz_target,
        seed,
        iterations,
        max_len,
        corpus,
        failure_dir: failure_dir.unwrap_or_else(|| PathBuf::from("fuzz_failures")),
    })
}

fn set_once<T>(slot: &mut Option<T>, value: T, option: &str) -> io::Result<()> {
    if slot.replace(value).is_some() {
        Err(invalid_input(format!("duplicate option {option}")))
    } else {
        Ok(())
    }
}

fn parse_u64(value: &str, option: &str) -> io::Result<u64> {
    value
        .parse()
        .map_err(|_| invalid_input(format!("{option} requires an unsigned decimal integer")))
}

fn parse_usize(value: &str, option: &str) -> io::Result<usize> {
    value
        .parse()
        .map_err(|_| invalid_input(format!("{option} requires an unsigned decimal integer")))
}

fn invalid_input(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

#[cfg(test)]
mod tests {
    use super::{parse_arguments, Command};
    use std::path::PathBuf;

    #[test]
    fn seed_is_required() {
        let error = parse_arguments(vec![
            "--fuzz-target".into(),
            "smoke".into(),
            "--iterations".into(),
            "1".into(),
            "--max-len".into(),
            "8".into(),
        ])
        .expect_err("missing seed must fail");
        assert!(error.to_string().contains("--seed is required"));
    }

    #[test]
    fn complete_run_arguments_parse_with_zero_iterations() {
        let command = parse_arguments(vec![
            "--fuzz-target".into(),
            "smoke".into(),
            "--seed".into(),
            "20260903".into(),
            "--iterations".into(),
            "0".into(),
            "--max-len".into(),
            "0".into(),
        ])
        .expect("valid run");
        assert_eq!(
            command,
            Command::Run {
                fuzz_target: "smoke".to_string(),
                seed: 20260903,
                iterations: 0,
                max_len: 0,
                corpus: None,
                failure_dir: PathBuf::from("fuzz_failures"),
            }
        );
    }

    #[test]
    fn replay_is_exclusive() {
        let error = parse_arguments(vec![
            "--replay".into(),
            "failure.txt".into(),
            "--seed".into(),
            "1".into(),
        ])
        .expect_err("mixed replay must fail");
        assert!(error.to_string().contains("--replay must be used alone"));
    }

    #[test]
    fn replay_path_parses() {
        assert_eq!(
            parse_arguments(vec!["--replay".into(), "failure.txt".into()]).expect("replay"),
            Command::Replay {
                failure_file: PathBuf::from("failure.txt")
            }
        );
    }
}
