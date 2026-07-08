# 21 — Structured Event Format

Document ID: created by AXIOM-EVENT-001 (Real OS Phase 3).
Requirement reference: `AxiomrtFull Completion Mode.md` §11,
docs/11_RUNTIME_MONITORING.md (kernel-side event export),
docs/20_REAL_OS_PRODUCT_DEFINITION.md §6.

## 1. Purpose

The kernel already emits one structured line per safety/security
relevant action on the serial port (docs/11_RUNTIME_MONITORING.md).
This document freezes that line format as a **parsing contract** and
defines the host-side JSON export used by `axiomctl` and AxiomRT
Studio. The kernel is not changed by this phase: the host parses what
the kernel already says. Events are evidence; the parser must be
lossless (raw line always preserved) and must never invent fields.

## 2. Serial Line Format (kernel → host, existing)

One event per line, ASCII, `\n`-terminated:

```text
KIND [flag ...] [key=value ...]
```

* `KIND` — first whitespace-separated token, uppercase.
* `flag` — bare tokens after the kind (e.g. `recv`, `delivered`,
  `fault_event`).
* `key=value` — attribute pairs; values contain no spaces.
* The monitor module additionally emits `EVT type=<KIND> ...` lines
  (docs/11 §4); for these the effective kind is the `type=` value.

### 2.1 Kind vocabulary (v1.0, observed and documented)

| Kind | Category | Example |
|---|---|---|
| `TASK_STARTED` `TASK_EXITED` `TASK_FAULTED` | task | `TASK_STARTED task=critical_task` |
| `SCHED` | scheduler | `SCHED selected=critical_task` |
| `SYSCALL` | syscall | `SYSCALL name=sys_yield task=critical_task` |
| `IPC` `IPC_DENIED` | ipc | `IPC delivered fault_event to=supervisor_task from=faulty_task` |
| `CAP_DENIED` | capability | `CAP_DENIED task=faulty_task reason=no_valid_capability` |
| `FAULT` (except `type=WatchdogTimeout`) `PAGE_FAULT` `DEADLINE_MISSED` `CONTAIN` | fault | `CONTAIN scope=user reason=watchdog_timeout action=faulted kernel=alive` |
| `FAULT type=WatchdogTimeout` `WATCHDOG_TIMEOUT` | watchdog | `FAULT type=WatchdogTimeout task=faulty_task` |
| `RECOVERY_APPLIED` | recovery | `RECOVERY_APPLIED policy=Kill` |
| `TIMER` | timer | `TIMER tick=3` |
| `SUPERVISOR` `LOGGER` | service | `SUPERVISOR decision=Kill by=supervisor_task` |
| `MMU`, `AxiomRT kernel booted`, bare `key=value` boot lines | boot | `MMU status=enabled mode=sv39 scope=kernel` |
| `DEVICE` `DEVICE_DENIED` `MMIO` `MMIO_DENIED` `DMA` `DMA_DENIED` `IRQ` `IRQ_DENIED` `IRQ_DROPPED` `DRIVER` `DRIVER_MANAGER` | driver | `MMIO grant task=block_driver_service device=block0 region=virtio_mmio0` (v1.5, docs/31) |
| `APP_IMAGE` | loader | `APP_IMAGE loaded=hello source=/bin/hello.app` / `APP_IMAGE rejected=invalid_bad_magic reason=bad_image` (v1.6, docs/32) |

Lines whose first token is not in this vocabulary (OpenSBI banner,
cargo output, blank lines) are **skipped and counted**, never guessed
at. The categories cover the roadmap gate list: task, scheduler, IPC,
capability, fault, watchdog, recovery.

Kernel rule (unchanged from docs/11): absent fields are omitted, never
zero-filled; overflowing monitor lines carry an explicit `!truncated`
marker.

## 3. JSON Export (host-side, this phase)

`axiomctl events parse <logfile>` emits **NDJSON**: one JSON object
per parsed event, in file order:

```json
{"seq":17,"category":"capability","kind":"CAP_DENIED","flags":[],"fields":{"task":"faulty_task","reason":"no_valid_capability"},"raw":"CAP_DENIED task=faulty_task reason=no_valid_capability"}
```

* `seq` — 1-based index over parsed events (not raw lines).
* `category` — table above.
* `kind` — effective kind (for `EVT` lines: the `type=` value).
* `flags` — bare tokens, in order.
* `fields` — key/value pairs, in line order; values stay strings
  (the parser does not guess types; `ts=128` exports as `"128"`).
* `raw` — the exact input line, JSON-escaped. Losslessness rule: the
  original log is always reconstructible from `raw`.

`axiomctl events summary <logfile>` prints per-category and per-kind
counts plus the skipped-line count — the quick human check that a demo
log contains the expected story (task starts, capability denial,
watchdog fault, recovery, scheduler continuing).

The roadmap's illustrative object (`{"time":…,"kind":"fault",…}`) is
realized by this schema: `time` is `fields.ts`/`fields.tick` when the
kernel provides one; nothing is fabricated when it does not.

## 4. Implementation and Tests

* Parser: `tools/axiomctl/src/events.rs` (std only, zero external
  dependencies; the JSON writer is hand-rolled with full string
  escaping — control chars, quotes, backslashes).
* AXIOM-EVENT-002: `parse_line`/`parse_log` over the §2 contract.
* AXIOM-EVENT-003: `axiomctl events parse|summary` subcommands.
* AXIOM-EVENT-004: unit tests in the same module use verbatim lines
  captured from a real `demo_full` QEMU run; run with
  `cargo test --target x86_64-unknown-linux-gnu -p axiomctl` (part of
  `scripts/verify_all.sh`).

## 5. Stability

The §2 vocabulary is append-only: new kinds may be added with their
category documented here; existing kinds/fields are never renamed
silently, because archived evidence logs must stay parseable. A kernel
line-format change without a matching update to this document and the
parser tests is a defect.
