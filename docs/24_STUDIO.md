# 24 — AxiomRT Studio (Local Dashboard)

Document ID: created by AXIOM-STUDIO-001 (Real OS Phase 4).
Requirement reference: `AxiomrtFull Completion Mode.md` §12,
docs/20_REAL_OS_PRODUCT_DEFINITION.md §6, docs/21_EVENT_FORMAT.md.

## 1. Purpose

AxiomRT Studio is the local graphical dashboard: one place to run the
flagship demo, watch the event timeline, and inspect tasks, faults,
IPC, capability denials, test results, proof status, evidence archives,
limitations, and the release checklist — visually, without reading
internal scripts.

Studio is **host tooling only**. It is not part of the kernel or TCB,
adds no code to the target image, and makes no safety claim. The
Architecture Law's "no GUI in the kernel" is preserved by construction:
Studio lives in `studio/` and talks to the system only through the
repository's authoritative scripts and serial logs.

## 2. Technology choice (disclosed deviation)

The roadmap offers "Next.js + Tailwind **or another simple local web
UI**". Studio takes the second option: a **zero-dependency Rust local
web server** (std `TcpListener`, hand-rolled minimal HTTP/1.1) serving
one embedded HTML/JS page. Reasons:

* The repository-wide zero-external-dependency discipline
  (docs/07_CODEX_RULES.md) would be broken by a Node toolchain with
  hundreds of transitive packages.
* The installer story stays "Rust + QEMU only" — no Node runtime.
* The dashboard's needs (tables, timeline, buttons, polling) do not
  require a framework.

## 3. Architecture

```text
browser (vanilla JS, fetch-polling)
   |            single embedded page, client-side panel routing
   v
studio  (Rust, std only, 127.0.0.1:8787)
   |    shares axiomctl's event-parser library  -> docs/21 contract
   v
authoritative scripts + cargo   (run_qemu flags, verify_all.sh,
                                 build_eval_kit.sh)  + serial logs
```

* Crate: `studio/` — workspace member, **not** a default member (same
  rule as axiomctl; the default cargo target is bare-metal riscv64).
* `tools/axiomctl` is split into lib + bin so `axiomctl::events` is
  shared. Studio parses logs with exactly the code the CLI uses
  (AXIOM-STUDIO-002's "backend wrapper" is realized as a shared
  library plus the same scripts — CLI and UI cannot diverge).
* Long actions (demo run, verify sweep, kit build) run on a background
  thread; state lives in an `Arc<Mutex<…>>` and the page polls
  `/api/state`.
* The demo run builds `--features demo_full`, boots QEMU with the
  `run_qemu.sh` flag set under a bounded timeout, captures the serial
  log, and parses it into events.

## 4. Routes

Every page path serves the same dashboard shell; the path selects the
active panel: `/`, `/run`, `/tasks`, `/scheduler`, `/faults`, `/ipc`,
`/capabilities`, `/drivers`, `/tests`, `/proofs`, `/evidence`,
`/limitations`, `/release`.

API (JSON unless noted):

| Route | Method | Behavior |
|---|---|---|
| `/api/state` | GET | run/verify/kit status, exit results, doctor summary |
| `/api/run_demo` | POST | start full-demo run (rejected while busy) |
| `/api/run_verify` | POST | start `verify_all.sh` sweep |
| `/api/kit_build` | POST | run `build_eval_kit.sh` |
| `/api/events` | GET | parsed events of the last demo run (docs/21 NDJSON schema, JSON array) |
| `/api/log` | GET (text) | raw serial log of the last demo run |
| `/api/verify_log` | GET (text) | output of the last verify run, else the archived clean log |
| `/api/evidence` | GET | versions and files under `evidence/` |
| `/api/evidence/file?ver=X&file=Y` | GET (text) | one evidence file (names validated: no separators, no `..`) |
| `/api/doc?name=limitations\|assumptions` | GET (text) | kit documents |
| `/api/release_check` | GET | the axiomctl release checklist, as items |

## 5. Panels (roadmap §12 list)

1. System status — doctor-style toolchain summary + kernel/run state.
2. Run — "Run Full Demo" button, live terminal output, run result.
3. Timeline — parsed events in order (boot → task starts → denial →
   watchdog → recovery → scheduler continues), with category badges.
4. Tasks — table derived from events: started tasks, last observed
   state (running / blocked / faulted / killed).
5. Scheduler — SCHED selection counts per task.
6. Faults — FAULT/CONTAIN table.
7. IPC — IPC event table.
8. Capabilities — CAP_DENIED table.
8b. Drivers — driver-framework events (docs/31): device registration,
   MMIO/DMA grants and denials, IRQ delivery/drops, driver lifecycle.
9. Tests — sections and PASS/FAIL from the verify log.
10. Proofs — Coq section of the verify log + refinement-TODO notice.
11. Evidence — version browser and file viewer.
12. Limitations — kit/LIMITATIONS.md rendered as text.
13. Release — release checklist + "Assemble kit" button.

## 6. Security posture

* Binds `127.0.0.1` only; never exposed to the network.
* No authentication — acceptable only because of the localhost bind;
  documented as a limitation. Do not port-forward Studio.
* File-serving endpoints accept single path components only
  (`[A-Za-z0-9._-]`, no `..`, no separators) under fixed roots.
* POST actions are the same ones the CLI offers; nothing destructive.

## 7. Verification

* `cargo build --target x86_64-unknown-linux-gnu -p studio` warning-free.
* Unit tests: request-line routing, evidence-name validation, task
  state derivation from events.
* Gate (roadmap §12): open Studio, click "Run Full Demo", watch the
  behavior visually — exercised manually via curl-driven checks of
  every endpoint in the phase evidence.

## 8. Limitations

* Fetch-polling (1 s), not websockets/SSE — fine at dashboard scale.
* The demo run is bounded (default 25 s of QEMU) so the UI always
  returns; the full 10^5-schedule soak remains the QEMU test suite's
  job (tests/full_fault_containment_demo_qemu_test.sh).
* No Markdown rendering; kit documents display as preformatted text.
