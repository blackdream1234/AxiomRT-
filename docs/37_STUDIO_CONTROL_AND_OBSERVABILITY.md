# 37 — Studio Control and Observability Console

Document ID: created by AXIOM-STUDIO-001 (v1.8 checkpoint).
Requirement reference: docs/21_EVENT_FORMAT.md, docs/24_STUDIO.md,
docs/25_OS_BOOT_FLOW.md, docs/26_SHELL.md, docs/31, docs/34,
docs/36_ROBUSTNESS_AND_FUZZING.md.

## 1. Purpose

AXIOM-STUDIO-001 turns the docs/24 evidence dashboard into an interactive
engineering console: an operator can boot AxiomRT under QEMU, drive it
through documented shell commands, watch faults being contained and
recovered live, inspect the capability architecture, and collect
verification evidence — all from one local page. It is a demonstration
and observability tool for the emulator-oriented research prototype; it
makes no certification, production, or real-hardware claim.

## 2. Architecture and trust boundary

```text
AxiomRT (os_boot kernel) in QEMU
      | serial (structured event lines, docs/21)
      v
studio host process
      | axiomctl event parser (shared library, docs/21 contract)
      v
normalized JSON APIs (127.0.0.1 only)
      | polling fetch
      v
embedded single-page console (no external assets)
```

Studio is host-side only. It contains no kernel code, adds no kernel or
U-mode API, holds no capability, and cannot bypass any check: every
control action is one *documented shell command* typed into the target's
serial console, exactly as the QEMU test scripts type it. The kernel and
services treat that input as untrusted operator input, as always.

The server binds 127.0.0.1 exclusively and must not be port-forwarded
(docs/24 §6). File endpoints accept single validated path components
under fixed repository roots.

## 3. Live session

`POST /api/session/start` builds the os_boot kernel and boots
`qemu-system-riscv64 -machine virt -nographic` with the serial console
attached to Studio. The session states are
`idle / building / running / stopped / failed`.

Commands are issued only through `POST /api/session/cmd?name=<key>`,
where `<key>` must be an entry of the fixed `SCENARIOS` whitelist in
`studio/src/main.rs`. The browser can never send a free-form command
line. Each entry is one documented shell command (docs/26/27/28/29/31/34)
— e.g. `tasks`, `run hello`, `app load hello`, `net send-test`,
`driver fault block`, `net restart`, `shutdown`. Deliberate test faults
are typed `fault` and rendered with an explicit DELIBERATE TEST FAULT
marking; the UI disables a button while its command is pending and never
reports success — outcomes appear only as serial evidence.

`POST /api/session/stop` requests the shell's controlled `shutdown`
(SBI system reset) and falls back to killing QEMU after a few seconds.

Bounded retention (docs/36 §5.12 discipline): the serial log keeps the
newest 1 MiB; the issued-command history keeps 200 entries; the event
API serves at most 1500 parsed events. Studio adds no unbounded history.
ROBUST-010 (host event-ingestion fuzzing) remains a separate future task
and is not weakened by this console.

## 4. Data sources and state honesty

Every observation page derives from **parsed serial events** of exactly
one source, shown as a badge:

* **LIVE SESSION** — the current/most recent interactive session log;
* **DEMO RUN (bounded capture)** — the non-interactive demo_full run;
* archived evidence files are shown under Evidence as **REPLAYED
  EVIDENCE** and are never presented as a live system.

Derived state is explicitly *observed event state*, not direct kernel
introspection: a service the log never mentioned renders as
`not_observed`, never as up or down. Unknown serial lines are skipped by
the shared parser and counted, never guessed at. Studio invents no
state and prefills no results.

The Capabilities view separates two layers:

* **Boot policy** — the static deny-by-default capability model of the
  docs/25 §5 service table (mirrored from os_boot.rs), labeled as an
  architecture model because the kernel exports no per-slot cap-table
  telemetry;
* **Live observed authority activity** — grant announcements and
  CAP/MMIO/DMA/IRQ/DEVICE/IPC/NET denial events actually parsed from
  the selected source.

## 5. Views

Overview (identity, session/job status, observed snapshot, architecture
constants, layer diagram) · Live session (serial console + issued
commands) · Demo scenarios (whitelisted controls + guided sequence) ·
Demo run (bounded non-interactive demo_full capture) · Tasks · Services
(boot-frozen set with observed lifecycle, fault and restart counts) ·
Capabilities (policy + observed) · IPC · Drivers (block skeleton +
synthetic net) · Network (explicit SYNTHETIC NETWORKING banner) ·
Faults & containment (kernel-alive summary) · Events (filterable,
bounded, raw preserved) · Scheduler · Loader · Verification · Proofs ·
Evidence · Limitations · Release.

## 6. Guided demonstration sequence

The Demo scenarios page encodes the docs/20-style storyline over live
commands: confirm liveness → run hello → filesystem/storage → synthetic
network TX/RX → capability denial (`run fault_demo`) → unrelated-service
survival → block-driver fault/containment/restart → network-driver
fault (`driver_down`)/restart/counter reset → run hello again → finish
on the Verification page.

## 7. Verification view

Separate evidence categories, none prefilled:

* full sweep (`scripts/verify_all.sh`: QEMU serial-assertion tests,
  host suites, Coq models) run as a job with the live log shown;
* deterministic fuzz evidence: a job running the `ipc`, `capability`,
  and `syscall` targets (10,000 cases each, fixed seeds 20260903/
  20260904); any `KERNEL_INVARIANT_FAILURE` exits non-zero and the raw
  counters/digests are displayed from the actual run;
* Coq model status with the explicit model-vs-implementation boundary;
* the per-version evidence archive and the release checklist.

Fuzzing shown here is robustness evidence for the exercised cases, not
proof (docs/36 §9).

## 8. How to run

```sh
cargo studio            # alias: cargo run -p studio (host target)
# open http://127.0.0.1:8787/
```

Live sessions need `qemu-system-riscv64` and the riscv64 Rust target on
the same machine. Without QEMU the console still serves all evidence,
policy, and replay views; session controls simply fail closed.

## 9. Security limitations and known gaps

* Studio is a local development tool: no authentication, so it must
  stay bound to 127.0.0.1 and never be exposed.
* Control is limited to the scenario whitelist by design; there is no
  free shell passthrough.
* Observed state is event reconstruction; it can lag or miss what the
  bounded log no longer contains.
* The boot-policy view is a maintained mirror of os_boot.rs, guarded by
  tests for the service set and the deny-by-default anchors; it is not
  generated from the kernel source.
* Host event-parser robustness under adversarial logs remains
  AXIOM-ROBUST-010.
* This console is a v2.5 presentation building block; polish items
  (recorded walkthroughs, timing panels) arrive with their evidence
  milestones (v1.9+).
