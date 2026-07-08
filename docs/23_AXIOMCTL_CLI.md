# 23 — axiomctl Developer CLI

Document ID: created by AXIOM-CLI-001 (Real OS Phase 2).
Requirement reference: `AxiomrtFull Completion Mode.md` §10,
docs/20_REAL_OS_PRODUCT_DEFINITION.md §6.

## 1. Purpose

`axiomctl` is the host-side developer CLI for AxiomRT. It gives a new
user one obvious entry point for building, running, demoing, verifying,
and packaging the system without reading the internal scripts. It is a
**wrapper**: every operation delegates to the same scripts and cargo
commands the repository already treats as authoritative
(`scripts/run_qemu.sh`, `scripts/verify_all.sh`,
`scripts/build_eval_kit.sh`). axiomctl never re-implements a
verification step, so CLI output and script output can never diverge.

axiomctl is host tooling. It is not part of the kernel, not part of the
TCB, and makes no safety claim.

## 2. Placement and build

* Crate: `tools/axiomctl` (Rust, **std, zero external dependencies** —
  the repository-wide no-dependency discipline applies to host tools
  too; argument parsing is hand-rolled).
* Workspace: member of the root workspace, but **not** a default
  member. `default-members = ["kernel"]` keeps every existing
  documented command (`cargo build --release`,
  `cargo build --release --features demo_full`, all test scripts)
  building the bare-metal kernel exactly as before.
* Target: the repository's default cargo target is
  `riscv64gc-unknown-none-elf` (bare metal), so axiomctl must be built
  with an explicit host target:

  ```sh
  cargo run --target x86_64-unknown-linux-gnu -p axiomctl -- doctor
  ```

  A cargo alias makes this the documented short form:

  ```sh
  cargo axiomctl doctor
  ```

  **Disclosed deviation** from the roadmap gate's literal
  `cargo run -p axiomctl -- doctor`: without `--target` cargo would
  build axiomctl for bare-metal RISC-V, which cannot host a std CLI.
  The alias provides the equivalent one-command experience. The alias
  hardcodes the x86_64 Linux host triple; non-x86_64 hosts use the
  explicit `--target` form.

## 3. Subcommands

| Command | Behavior |
|---|---|
| `axiomctl doctor` | Check rustc, cargo, riscv64gc target, qemu-system-riscv64, coqc (optional), and required repo files; print OK/MISSING per item; exit nonzero if a required item is missing. |
| `axiomctl build` | `cargo build --release` at the repo root (default-feature kernel, riscv64 target via `.cargo/config.toml`). |
| `axiomctl run` | Delegate to `scripts/run_qemu.sh` (builds default kernel, boots QEMU interactively; exit with Ctrl-A x). Extra args pass through to QEMU. |
| `axiomctl demo memory` | The memory-isolation demo is the default build: build default features, boot QEMU. |
| `axiomctl demo full` | `cargo build --release --features demo_full -p kernel`, then boot QEMU with the same flags as `run_qemu.sh` (which cannot be reused verbatim because it rebuilds the default features). Prints a reminder that the next `axiomctl build` restores the default kernel. |
| `axiomctl demo drivers` | `cargo build --release --features os_boot -p kernel`, then boot the interactive OS in QEMU (v1.5 driver framework, docs/31): the operator drives `drivers` / `driver info block` / `driver fault block` / `driver restart block` at the `axiom>` prompt. Same restore reminder as `demo full`. |
| `axiomctl demo loader` | Same os_boot build/boot (v1.6 restricted loader, docs/32): the operator drives `bin` / `app load hello` / `app state hello` / `run loaded hello` / `app unload hello` / `app load invalid_bad_magic` at the `axiom>` prompt. Same restore reminder. |
| `axiomctl verify` | Delegate to `scripts/verify_all.sh`; propagate its exit code. |
| `axiomctl evidence list` | List `evidence/<version>/` directories with file counts. |
| `axiomctl evidence open <ver> [file]` | List the files of one evidence directory; with `file`, print that file. |
| `axiomctl kit build` | Delegate to `scripts/build_eval_kit.sh`. |
| `axiomctl release check` | Release hygiene checklist: clean git tree, HEAD tagged, 7 kit documents present, clean verification log present with `VERIFY ALL: PASS` and zero warnings, run/verify/kit scripts present. PASS/FAIL per item; exit nonzero on any FAIL. |
| `axiomctl version` | axiomctl version and `git describe --tags` of the repo. |
| `axiomctl help` | Usage. |

## 4. Behavior rules

1. axiomctl locates the repository root by walking up from the current
   directory until it finds `scripts/verify_all.sh` next to
   `kernel/Cargo.toml`; it fails with a clear message outside the repo.
2. Child processes inherit stdio, so QEMU serial output, cargo
   progress, and test output appear unmodified.
3. Exit codes propagate: `axiomctl verify` fails exactly when
   `verify_all.sh` fails.
4. axiomctl performs no destructive git operations and never writes
   outside `target/`, `release/`, and the streams it inherits.
5. No color, no TTY tricks — output must be readable in CI logs.

## 5. Verification

* `cargo build --target x86_64-unknown-linux-gnu -p axiomctl` is
  warning-free.
* Gate (roadmap §10): a new user runs `doctor`, `demo full`, `verify`
  and gets useful output without reading internal scripts.
* `axiomctl verify` output is byte-identical to `verify_all.sh` output
  (it is the same process).

## 6. Limitations

* The `cargo axiomctl` alias assumes an x86_64 Linux host.
* `demo full` duplicates the six QEMU flags of `run_qemu.sh`; a change
  there must be mirrored (noted in both files).
* `evidence open` prints files as-is; it does not render Markdown.
