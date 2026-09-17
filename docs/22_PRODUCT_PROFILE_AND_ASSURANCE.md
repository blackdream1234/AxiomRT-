# 22 — Product Profile and Assurance Foundation

Document ID: AXIOM-DOC-010
**Status: DRAFT — OWNER REVIEW REQUIRED.**

This document is a draft prepared under AXIOM-DOC-010. It does **not**
approve architecture, does not close AXIOM-PLAN-002, and does not
authorize AXIOM-FOUND-002 implementation. Every proposal in sections 4,
6, 7 and 9 is a candidate for owner decision, not a settled requirement.

## 0. Provenance of this draft

| Field | Value |
|---|---|
| Inspected commit | `93b719a4c7a1f32dae49f520b4d765af47b0d046` |
| Branch | `v1.8-robustness-fuzzing` |
| Working tree at initial inspection | no tracked modifications; untracked `.vscode/`, `AxiomRT_Engineering_Workplan_v0_1.md`, `AxiomRTv1.7.md`; DOC-010 drafting subsequently added this untracked document and the separately preserved `docs/INDEX.md` row |
| Kernel configurations inspected | `--features os_boot`; `--features demo_ipc_payload`; default |
| Target | `riscv64gc-unknown-none-elf`, single hart, QEMU `virt` + OpenSBI |
| Requirement basis | docs/07_CODEX_RULES.md §3, §8; docs/20_REAL_OS_PRODUCT_DEFINITION.md §4, §5, §7, §11, §12; docs/01_SCOPE_AND_NON_GOALS.md §2; workplan AXIOM-PLAN-002 |

**Evidence provenance limitation.** The static stack figures quoted in §8
originate from the preserved and identifiable artifact
`target/axiom-found-001/stack-review/STACK-EVIDENCE-v3.md`. `target/` is
listed in `.gitignore` (line 2), so the artifact is not reproduced by a
fresh repository checkout and no durable archive location is identified.
Those limitations do not withdraw the prior scoped acceptance of
AXIOM-FOUND-001: Git is not the only valid evidence store, and reviewed
evidence may support a claim when its identity, configuration, inputs and
limitations are retained. Repository reproducibility, evidence durability
and review status are separate properties. Recorded as gap **EV-G-1** (§9).

## 1. Relationship to existing product documents

This document does **not** replace or restate the approved product
definition. It is subordinate to it.

* `docs/20_REAL_OS_PRODUCT_DEFINITION.md` remains the governing product
  document: Architecture Law (§4), service responsibilities (§5),
  hardware targets (§7), non-goals (§11), certification tiers (§12).
* `docs/01_SCOPE_AND_NON_GOALS.md` §2 remains the v0.1 non-goal list.
* `docs/INDUSTRIAL_EVALUATION_KIT.md` describes the existing v1.0 kit.

What this document adds, and what PLAN-002 asked for, is the material
absent from those: an explicitly **bounded evaluation profile**, asset
and trust-boundary identification, a threat model, safety assumptions, a
preliminary hazard register, an evidence-record schema, a tool inventory
and review roles.

Where this document and `docs/20` appear to differ, `docs/20` governs and
the difference is a defect in this draft.

## 2. Approved requirements (cited, not re-decided)

These are already approved elsewhere. They are listed so the profile can
reference them; approval is **not** conferred by this document.

| ID | Requirement | Approving document |
|---|---|---|
| EP-R-001 | The kernel contains only boot, traps, interrupt routing, address spaces, page tables, thread/process model, scheduler, IPC mechanism, capability lookup, syscall validation, timer, fault event creation, minimal HAL | docs/20 §4 |
| EP-R-002 | GUI, filesystem logic, network stack, shell, package manager, complex drivers, dynamic policy, AI logic, user accounts, logging backends and application frameworks are excluded from the kernel | docs/20 §4 |
| EP-R-003 | The ten user-space service responsibilities and their separation into distinct address spaces | docs/20 §5 |
| EP-R-004 | Single hart is the supported configuration until a dedicated multicore phase exists | docs/20 §7 |
| EP-R-005 | Hardware phases require a physical board; absence is recorded as a blocker, never simulated or claimed | docs/20 §7 |
| EP-R-006 | Bounded copy-based IPC is the only channel; shared-memory IPC is excluded | docs/20 §11 |
| EP-R-007 | The four certification tiers are never conflated; this repository may claim tier 1 today | docs/20 §12 |
| EP-R-008 | Every task carries the nine mandatory elements; no code before its document exists | docs/07 §3, §8 |
| EP-R-009 | Documentation states facts and rules, not aspirations; the current profile makes no unsupported certification, compliance, fitness-for-purpose or "never fails" claim | docs/07 §8, docs/01 |

## 3. Current implementation facts (source-cited)

These are **source constants and structure read from the tree at the
inspected commit**. They are compile-time facts. They are *not*
measurements of performance, capacity utilisation or timing, and the
presence of a constant does **not** imply that the design it encodes has
been approved.

`kernel/src/main.rs` compiles `dispatch.rs` only when
`target_arch = "riscv64"`; the module is feature-independent on that target
but remains inert until tasks are registered and `dispatch::start` activates
it. `os_boot.rs` and `ipc_payload_demo.rs` are separately gated by
`target_arch = "riscv64"` plus their named Cargo features. The configuration
column below uses those exact module scopes rather than the ambiguous word
"all".

| Fact | Value | Source | Configuration / module scope |
|---|---|---|---|
| Task slots (ceiling) | `MAX_TASKS = 16` | `kernel/src/arch/riscv64/dispatch.rs:252` | RISC-V dispatcher module; active dispatcher configurations only |
| User address spaces (ceiling) | `MAX_USER_AS = MAX_TASKS` | `kernel/src/arch/riscv64/paging_hw.rs:43` | RISC-V paging module |
| Capability slots per task (ceiling) | `CAPS_PER_TASK = 9` | `dispatch.rs:71` | RISC-V dispatcher module; active dispatcher configurations only |
| Endpoints (ceiling) | `NUM_ENDPOINTS = 12` | `dispatch.rs:287` | RISC-V dispatcher module; active dispatcher configurations only |
| IPC payload bound | `IPC_MSG_MAX = 128` bytes | `dispatch.rs:267` | RISC-V dispatcher module; active dispatcher configurations only |
| Per-endpoint payload storage | `EP_MSG[[u8;128];12]` | `dispatch.rs` (AXIOM-FOUND-001) | RISC-V dispatcher module; active dispatcher configurations only |
| Event ring | `RING_LEN = 32` | `dispatch.rs:1697` | RISC-V dispatcher module; active dispatcher configurations only |
| Info buffer | `INFO_MAX = 768` | `dispatch.rs:1909` | RISC-V dispatcher module; active dispatcher configurations only |
| Reserved fault endpoint | `EP_FAULT = 2` | `dispatch.rs:1614` | RISC-V dispatcher module; active dispatcher configurations only |
| Reserved event endpoint | `EP_EVENT = 3` | `dispatch.rs:1615` | RISC-V dispatcher module; active dispatcher configurations only |
| Object types | `ENDPOINT 0, CONSOLE 1, CONTROL 2, INFO 3, DEVICE 4` | `dispatch.rs` | RISC-V dispatcher module; active dispatcher configurations only |
| Rights bits | `SEND 1<<3, RECV 1<<4, CONTROL 1<<7` | `dispatch.rs` | RISC-V dispatcher module; active dispatcher configurations only |
| Boot service table entries | 15 `ServiceDef` entries | `kernel/src/arch/riscv64/os_boot.rs:128` | RISC-V `os_boot` feature module |
| Devices declared | `block0` (MMIO `0x1000_1000`, size `0x200`, DMA 4096 B, IRQ ep 8); `net0` (`network_synthetic`, no MMIO, IRQ ep 10) | `dispatch.rs` `static DEVICES` | RISC-V dispatcher module; `os_boot` supplies the inspected grants and routes |
| MMIO width validator | `device::access_in_bounds(size, offset, width)` | `kernel/src/device/mod.rs:220` | Kernel library host model and RISC-V dispatcher caller |
| Kernel trap stack | 8192 B in each inspected configuration | `os_boot.rs`, `ipc_payload_demo.rs` | RISC-V `os_boot` and `demo_ipc_payload` feature modules only |
| Boot stack | `BOOT_STACK_SIZE = 64K` (distinct from the trap stack) | `kernel/linker.ld` | Linked bare-metal RISC-V kernel configurations using this linker script |

### 3.1 Capacity ceiling versus available runtime capacity

`MAX_TASKS = 16` is a **compile-time ceiling on slots**, not a statement
of how much capacity is free at runtime. The `os_boot` service table
declares 15 entries; how many are concurrently live, and therefore how
many slots remain for loaded applications, is a property of the running
configuration and of boot policy, and has **not been measured**. No
residual-capacity figure is claimed here. Recorded as gap **EV-G-2**.

The same distinction applies to `NUM_ENDPOINTS`, `CAPS_PER_TASK` and the
event ring: each is a declared ceiling, and occupancy under load is
unmeasured.

## 4. Proposed evaluation objectives (DRAFT)

Proposed bounded profile: a **single-hart RV64GC / Sv39 industrial
evaluation prototype** on QEMU `virt` with OpenSBI firmware.

| Aspect | Proposal | Status |
|---|---|---|
| Intended demonstration | Mechanism demonstration only: address-space isolation, fixed-priority scheduling, bounded synchronous IPC, capability enforcement, contained user faults with supervisor-driven recovery, and an interactive shell | derives from docs/20 §2, §8 |
| Workload | The declared boot services plus restricted application images loaded through `app_loader`; external stimulus limited to console input and the declared devices | **PROPOSED** |
| Service inventory | As declared in the `os_boot` table (§3), realising the roles of docs/20 §5 | approved roles, configuration **PROPOSED** |
| Resource ceilings | The ceilings in §3, used as *bounds*, with occupancy to be measured | **PROPOSED** |
| Startup | Static, compiled-in boot policy; no dynamic policy or runtime service discovery | approved direction (docs/20 §5.1); exact policy **PROPOSED** |
| Scheduling | Fixed priority, single hart, timer preemption | implemented; objectives **PROPOSED** |
| IPC | Bounded synchronous rendezvous, copy-based, per-endpoint payload ownership | approved (EP-R-006); implemented |
| Lifecycle scope | Which principals may start/stop/restart which targets | **DECISION REQUIRED** (D-1) |
| Fault-recovery scope | Which faults are contained, which are reported, which are recoverable | **DECISION REQUIRED** (D-4, D-5, D-6) |
| Timing objectives | None proposed. No timing budget exists and none is invented here | **DECISION REQUIRED** (D-10) |

### 4.1 Non-goals for this evaluation profile

Scoped to **this profile only**. These exclude work from the evaluation
prototype; they do **not** prohibit later roadmap phases, several of
which explicitly plan such work (workplan bands C and D).

* No multi-hart / SMP operation in this profile (docs/20 §7 permits a
  future dedicated phase).
* No hardware-board claim in this profile; hardware remains a separate
  gated phase (EP-R-005; workplan PLAN-004, CORE-007).
* No timing or worst-case-execution claim in this profile (workplan
  EVID-003 may introduce one later).
* No POSIX personality in this profile (workplan IND-004 plans a limited
  personality later).
* No writable filesystem or real network transport in this profile.
* No unsupported certification, compliance, customer-endorsement or
  fitness-for-purpose claim for the current profile (EP-R-007,
  EP-R-009). This restriction does not prohibit separately authorized
  future assurance work whose scope, evidence and approval basis are
  explicitly defined.

## 5. Assets, trust boundaries, threats and assumptions

### 5.1 Assets and trust boundaries

| ID | Asset | Trust position |
|---|---|---|
| AST-001 | Microkernel image (`.text`, `.rodata`) | Trusted computing base |
| AST-002 | Kernel writable state: task table, capability tables, `EP_MSG`, event ring, page tables | TCB; integrity-critical |
| AST-003 | Kernel trap stack (8192 B/config) | TCB; see HAZ-002 |
| AST-004 | OpenSBI firmware and M-mode | **Trusted, not verified by this project** (ASM-002) |
| AST-005 | User-space services (init, supervisor, logger, console, shell, fs, storage, driver manager, app loader, network) | Outside the microkernel TCB; trust depends on the property and authority being analysed |
| AST-006 | Restricted application images and their admission path | Images are untrusted input; `app_loader` is trusted for properties that depend on image validation and admission policy |
| AST-007 | Device MMIO windows and the `block0` DMA page (4096 B) | Trust depends on the claimed property; DMA crosses the service/kernel memory boundary (THR-004) |
| AST-008 | Build and analysis toolchain (§10) | Trusted by assumption (ASM-005); not reproducibility-verified |
| AST-009 | Evidence artifacts and logs | Integrity matters for acceptance; currently partly untracked (EV-G-1) |

The **microkernel TCB** comprises AST-001 through AST-003. OpenSBI and the
Sv39 hardware are external trusted platform assumptions for the current
mechanism claims. A component can be outside the microkernel TCB while
still being trusted for a particular security or recovery property.

| Property | Additional trusted components or roles | Unresolved scope |
|---|---|---|
| Memory separation | Sv39 hardware, OpenSBI startup state and the kernel's page-table and trap paths | Separate address-space roots are present, but the complete mappings installed in each service address space have not been shown to be pairwise isolated (EV-G-6) |
| Capability and lifecycle control | Boot-time capability minting/distribution plus the kernel checks; `init`, `supervisor` and `app_loader` are privileged policy-bearing roles | Which holder may control which target, and for which lifecycle operations, remains D-1 / AXIOM-FOUND-002 |
| Fault recovery | Kernel containment and event paths plus the `supervisor` policy/acknowledgement role | Store, acknowledgement, restart and terminal-reporting obligations remain D-3, D-4 and D-7 |
| Image admission | `app_loader` validation/admission policy plus the kernel mapping mechanism | Host fuzz evidence does not establish the on-target path (EV-G-3) |

The inspected `os_boot` configuration constructs distinct address-space
roots for service instances. Distinct roots do not by themselves prove
that every service mapping excludes every other service's private pages;
that mapping-composition evidence remains open under EV-G-6 and planned
AXIOM-FOUND-009. The other U-mode services are therefore neither
classified as trusted for every property nor declared mutually isolated
without an authority and mapping analysis.

Principal boundaries: **U-mode service ↔ kernel** (syscall/trap), **service
↔ service** (endpoint IPC), **kernel ↔ firmware** (SBI), **device ↔
kernel/service** (MMIO + DMA), **host tooling ↔ target** (serial). The
lifecycle boundary among `init`, `supervisor`, `app_loader` and their
targets is unresolved (D-1).

### 5.2 Proposed threat model (DRAFT)

*Attacker capabilities assumed:* full control of the code and data of one
U-mode service or application image, able to issue arbitrary syscalls
with arbitrary register and buffer arguments whenever it is scheduled.
Execution is sequential with respect to another hart in this profile;
synchronous nested traps remain possible.

*Entry points:* the syscall ABI; IPC message content and length; restricted
application image content; filesystem and storage protocol messages;
console input; device MMIO/IRQ/DMA interactions.

*Explicit exclusions (assumptions, not defences):* hostile physical attack
or physical fault injection, side channels, malicious firmware or
toolchain, malicious hardware, and multi-hart races (single hart,
EP-R-004). Planned engineering fault-injection tests (workplan EVID-004)
are an evidence technique, not an included hostile physical threat.
Excluding a threat does not establish containment against it. CPU
exhaustion and starvation by a malicious or faulty service remain an
included risk because the current scheduler enforces no per-service
execution budget (workplan CORE-004).

| ID | Threat | Boundary | Current position |
|---|---|---|---|
| THR-001 | Service reads or writes another service's memory | service ↔ service | Separate Sv39 roots and representative QEMU isolation tests exist; complete per-service mapping isolation is not established (EV-G-6, AXIOM-FOUND-009) |
| THR-002 | Service obtains authority it was not granted | service ↔ kernel | Capability lookup + rights check; **scope of `RIGHT_CONTROL` open (D-1)** |
| THR-003 | Service corrupts another service's in-flight IPC payload | service ↔ kernel | Addressed by AXIOM-FOUND-001 (per-endpoint `EP_MSG`) |
| THR-004 | Device or driver uses DMA to reach memory outside its grant | device ↔ kernel | `block0_dma` is a fixed 4096 B page; containment argument **not established** (workplan IND-002) |
| THR-005 | Malformed application image subverts the loader | image ↔ service | `axiom-fuzz` loader target (host model only, EV-G-3) |
| THR-006 | Malformed protocol message subverts fs/storage service | service ↔ service | `axiom-fuzz` fs/storage targets (host model only) |
| THR-007 | User-supplied pointer/length induces a kernel-mode fault during copy | service ↔ kernel | Reachability **not established** (D-5) |
| THR-008 | Stale or reused handle grants authority over a different object | service ↔ kernel | No generation/epoch mechanism identified (D-1b) |
| THR-009 | A service exhausts CPU time and starves another service | service ↔ scheduler | Fixed priorities exist, but no enforced per-service execution budgets exist (workplan CORE-004) |

### 5.3 Safety assumptions

| ID | Assumption | If false |
|---|---|---|
| ASM-001 | Single hart excludes concurrent execution on another hart; it does not exclude synchronous nested traps | Arguments that require cross-hart exclusion fail; nested-trap arguments must be established separately |
| ASM-002 | OpenSBI/M-mode behaves per specification and is not hostile | TCB is larger than analysed |
| ASM-003 | Kernel page tables and `satp` remain intact while the kernel is executing | Address translation, isolation and copy-validation arguments fail |
| ASM-004 | Sv39 hardware enforces the permissions the kernel programs | Isolation claims fail |
| ASM-005 | Compiler, linker and analysis tools are correct and unmodified | Static evidence in §8 is void |
| ASM-006 | QEMU `virt` behaviour is representative for the mechanisms demonstrated — **explicitly not for timing** | Timing inferences would be invalid; none are made |
| ASM-007 | The terminal fault-reporting path does not itself fault before reaching its bounded termination mechanism | Reporting may recurse or fail to terminate as claimed (HAZ-003, D-7) |

### 5.4 Preliminary hazard register (PRELIMINARY — not a safety analysis)

This is a preliminary engineering register. It is not FMEA, FTA or any
recognised safety-lifecycle artefact, and it does not correspond to any
standard or integrity level.

| ID | Hazard | Current position | Related |
|---|---|---|---|
| HAZ-001 | Kernel-mode fault during a SUM-gated user copy halts the system | Reaches `PANIC … kernel_page_fault` and halts; reachability from U-mode unestablished | THR-007, D-5 |
| HAZ-002 | Trap-stack overflow writes into adjacent kernel data instead of faulting | Confirmed by inspection: no guard page; the trap stack is a `.bss` object with writable data adjacent | D-6 |
| HAZ-003 | A fault inside the fault-reporting path recurses | Reporting path was measured frameless, but freedom from re-fault is the distinct assumption ASM-007; intact mappings under ASM-003 do not establish it | D-7 |
| HAZ-004 | Over-broad lifecycle authority allows one service to terminate another | `RIGHT_CONTROL` scope is not target-restricted | THR-002, D-1 |
| HAZ-005 | Fault records are lost or duplicated, so recovery acts on stale information | On-target acknowledgement is record-only; no authoritative bounded store | D-3, D-4 |
| HAZ-006 | Formal models diverge from the running kernel without detection | Three Coq models exist with no established refinement to the runtime | D-8 |
| HAZ-007 | DMA-capable device reaches memory outside its grant | No containment argument | THR-004 |

## 6. Decision register (all OPEN)

Owner for every entry: **project owner (human)**. No entry is approved by
this document.

| ID | Open decision | Current evidence | Missing evidence | Acceptance criterion (measurable) |
|---|---|---|---|---|
| **D-1** | **Target scope of lifecycle authority** — whether control authority is per-target, per-domain or global | `RIGHT_CONTROL` exists and is checked, but is not target-restricted | Reachability analysis from each holder to each target | A principal holding control authority only for its own scope cannot start/stop/restart a target outside that scope; a permitted operation on an in-scope target still succeeds |
| **D-1b** | **Generation / stale-handle protection** — a *separate* question from D-1: whether handles carry a generation or epoch so a reused slot cannot be addressed by an old handle | No generation mechanism identified in the capability representation | Whether slot reuse is reachable at all | An operation issued with a handle to a since-reused slot is rejected with a defined error rather than acting on the new occupant |
| **D-2** | **Recording of denied operations** — whether the selected policy retains per-denial records, an aggregate count, diagnostics, or another bounded representation; no mechanism is selected here | Denials currently emit a diagnostic and set a caller result; no retention policy is established | Owner selection of recording semantics, bound and retention policy | The selected policy is stated and bounded. If it requires per-denial retention, each retained record identifies requester, target and reason and has defined behaviour at the bound. A counter can satisfy only an aggregate-count policy; by itself it cannot satisfy requester/target/reason retention |
| **D-3** | **Authoritative fault store** — location, bound and overflow policy | On-target acknowledgement is record-only (docs/19 §4); a separate host model exists | Which representation is authoritative on target | The store has a stated bound and a stated overflow policy, and the policy is observable on target when the bound is exceeded |
| **D-4** | **Fault-acknowledgement ABI on target** | ROBUST-005B added an authority gate on the acknowledgement path | Owner choice of contract | Acknowledging a fault that is not pending yields a defined error; acknowledging a pending fault transitions it exactly once; neither case alters unrelated task state |
| **D-5** | **User-copy fault policy** — alternatives treated symmetrically, with no outcome prescribed | A kernel-mode copy fault currently halts. AXIOM-FOUND-001 guarantees only that validation rejects before any copy begins | **Whether a mid-copy unmapping is reachable at all from U-mode under the current address-space and validation design.** This is a reachability question, not an established exploit path | Criterion depends on the alternative chosen: (a) *recoverable copy* — the copy fault is contained and the syscall returns a defined error, kernel continues; (b) *pre-validated copy* — validation makes the fault unreachable, and the argument for unreachability is stated and checked; (c) *accept and document* — the halt is documented as intended behaviour with its trigger conditions. Each alternative is acceptable if its own criterion is met; none is preferred here |
| **D-6** | **Trap-stack overflow protection** — guard page, redzone check, or accept-and-document. Distinct from D-7 | HAZ-002 confirmed by inspection | Cost of a guard mapping per configuration | Overflow past the trap stack produces a detectable event rather than a silent write to adjacent data — or the absence of protection is documented as accepted with its consequence |
| **D-7** | **Recursive kernel-fault handling** — distinct from D-6: what happens when a fault occurs while handling a fault | Reporting path measured frameless in the inspected configurations; bounded termination currently relies on the distinct assumption ASM-007 | Reachability of synchronous nesting and faults in the terminal reporting path | A fault raised during fault reporting terminates in a bounded number of steps by a stated mechanism, rather than relying on ASM-007; ASM-003 remains a separate intact-mapping assumption |
| **D-8** | **Runtime/model correspondence method** | Three Coq models compile | Chosen correspondence technique | Each model names the runtime behaviour it constrains, and at least one executable check fails if the runtime diverges from the model |
| **D-9** | **Hardware target and hardware-evidence definition** | No hardware evidence of any kind | Board selection (deferred by docs/20 §7 to its own task) | Boot, isolation and fault containment reproduced on one physical Sv39 board, with archived logs; until then hardware status is a recorded blocker (EP-R-005) |
| **D-10** | **Whether timing objectives belong in this profile** | No timing budget exists; runner timeouts are operational scheduling bounds only | Owner intent and, if proposed, rationale, configuration, method and approval basis | A proposed quantitative requirement states the quantity, bound, rationale, configuration and measurement method and is approved by the owner. A measured result is recorded separately with its observed value, configuration, method, tool/version, command, artifact and review status. An unmeasured target is never presented as achieved performance |

Open decisions block only the work or claim named below; they are not a
new project-wide gate and need not all be settled before unrelated tasks.

| Decision | Task or claim blocked while deferred |
|---|---|
| D-1 | AXIOM-FOUND-002 lifecycle-scope brief and any later AXIOM-FOUND-007 restart authority that depends on it |
| D-1b | Generation semantics in AXIOM-FOUND-002 and AXIOM-CORE-002 |
| D-2 | Any denial-retention acceptance claim introduced by AXIOM-FOUND-002 or AXIOM-FOUND-003; neither task may assume a recording policy |
| D-3 | AXIOM-FOUND-005 bounded fault-store brief |
| D-4 | AXIOM-FOUND-006 acknowledgement-ABI brief |
| D-5 | Any implementation or acceptance claim for recoverable user-copy faults; no task is allocated, and the provisional AXIOM-FOUND-011 name remains unassigned |
| D-6 | The guard-page choice in AXIOM-CORE-005 |
| D-7 | Any bounded terminal-fault claim in AXIOM-FOUND-004 through AXIOM-FOUND-006 |
| D-8 | AXIOM-CORE-006 and AXIOM-EVID-001 correspondence claims |
| D-9 | AXIOM-PLAN-004, AXIOM-CORE-007 and AXIOM-IND-002 hardware claims |
| D-10 | AXIOM-EVID-003 only if the owner first includes a timing requirement in the profile; it does not block review of this draft |

**Note on AXIOM-FOUND-011.** An identifier of that form was proposed
during AXIOM-FOUND-001 review for recoverable user-copy behaviour. It
remains **provisional**. It is *not* allocated by this document, no
roadmap entry is created, and D-5 is not bound to it.

## 7. Requirement traceability

Each approved requirement is mapped to the current implementation/evidence
status, a bounded acceptance statement or open decision, and its gap.

| Requirement | Implementation / evidence status | Acceptance criterion or explicit decision | Relevant gap |
|---|---|---|---|
| EP-R-001 | Kernel mechanisms were located in the inspected RISC-V dispatcher, paging, trap and device modules (§3) | Accept only the mechanism inventory for the stated module/configuration scope; no broader configuration claim | Feature reachability outside §3 was not exhaustively analysed |
| EP-R-002 | The inspected `os_boot` table places policy roles in U-mode services; property-specific trust remains as stated in §5.1 | For the evaluated configuration, listed policy remains outside the microkernel TCB; privileged service authority is analysed separately | D-1 and EV-G-6 |
| EP-R-003 | Declared roles have separate address-space roots in the inspected `os_boot` construction | Accept role separation only when lifecycle authority and installed mappings satisfy their task criteria | D-1 / AXIOM-FOUND-002; EV-G-6 / AXIOM-FOUND-009 |
| EP-R-004 | Evaluated configuration is single-hart | No concurrent execution on another hart is claimed; synchronous nested traps are handled separately | ASM-001, ASM-007 and D-7 |
| EP-R-005 | No physical-board evidence exists | D-9 remains OPEN; hardware claims require AXIOM-PLAN-004 and AXIOM-CORE-007 evidence | EV-G-5 |
| EP-R-006 | Dispatcher has a 128-byte copy bound and per-endpoint payload storage; AXIOM-FOUND-001 has prior scoped acceptance | Accept payload-ownership evidence only for its inspected configurations and stated static/runtime evidence classes | EV-G-1 and user-copy policy D-5 |
| EP-R-007 | Certification tiers remain defined by docs/20; this profile adds no higher-tier claim | Current claims must stay within reviewed evidence; separately authorized future assurance work remains possible | D-8, D-9 and any adopted D-10 requirement |
| EP-R-008 | Task format and document-before-code rules are governed by docs/07 | Apply the required checklist per task; deferred decisions block only tasks named in §6, not the whole roadmap | No new PLAN-002-wide gate |
| EP-R-009 | This document remains DRAFT and identifies unsupported claims, assumptions and limits | Owner review must confirm each retained claim has appropriate evidence and explicit limitations | Open §6 decisions and applicable §9 gaps |

The AST, THR, ASM, HAZ, D and EV-G identifiers are local draft registers.
Their traceability here does not approve architecture or convert every
tool-version or archival gap into a universal AXIOM-PLAN-002 gate.

## 8. Evidence classes and current standing

Evidence classes are kept separate. A result in one class does not
substitute for another.

Evidence durability, repository reproducibility and review status are
also separate. An artifact outside Git may support a reviewed claim when
it is preserved, identifiable and tied to its configuration, inputs and
limitations. It does not support a fresh-checkout reproducibility claim
unless that reproduction path is separately established. The prior
scoped AXIOM-FOUND-001 acceptance remains recorded on that basis.

| Class | What exists at the inspected commit | What it does not support |
|---|---|---|
| **Live runtime (QEMU serial assertion)** | 17 suites driven by `scripts/verify_all.sh` (16 `*_qemu_test.sh` plus `boot_smoke_test`), including `ipc_payload_ownership_qemu_test` | Timing; hardware behaviour; exhaustive interleaving |
| **Host model** | `kernel` host tests; `axiom-fuzz` targets; supervisor, axiomctl and studio suites | **Nothing about the on-target dispatcher.** `kernel::ipc` is a separate implementation; a model test alone does not establish dispatcher behaviour |
| **Static analysis** | `cargo fmt`, `clippy` across kernel configurations and the workspace; the AXIOM-FOUND-001 stack review | Runtime peaks; execution-path coverage |
| **Formal model** | `proofs/coq/MemoryIsolation.v`, `CapabilityAccess.v`, `SchedulerPriority.v` | No established refinement to the running kernel (HAZ-006, D-8) |
| **Hardware** | **None** | Every hardware claim (EP-R-005) |

### 8.1 Static stack figures carried forward

From the AXIOM-FOUND-001 stack review, with its configuration and
assumptions attached. These are **conservative static call-chain bounds
computed from compiler-emitted code — they are not runtime peak
measurements.** No runtime high-water mark was taken in any
configuration.

| Configuration | Static bound, deepest call chain incl. trap frame | Declared trap stack |
|---|---|---|
| `os_boot` at `611ef15…` | 1712 B | 8192 B |
| `os_boot` at `93b719a…` | 1584 B | 8192 B |
| `demo_ipc_payload` at `93b719a…` | 1488 B | 8192 B |

Attached conditions:

* Applies only to the three binaries above; other feature configurations
  were not analysed.
* Bounds are over the **static call graph**. Enumerated call-graph paths
  are not execution paths and do not establish execution-path coverage.
* One level of synchronous nested kernel fault was included in the static
  bound. Further bounded termination depends on ASM-007; ASM-003 is the
  separate assumption that page tables and `satp` remain intact.
* **No overflow threshold is established.** Figures previously expressed
  as an "11th to 20th nesting level" range are illustrative fixed-cost
  scenarios that assume a constant per-level cost and a fixed starting
  depth. Neither assumption is guaranteed for an arbitrary fault
  sequence, and no such threshold is claimed here.
* The generating artifact is gitignored but identifiable; its durability
  and fresh-checkout reproducibility limits are recorded in EV-G-1.

## 9. Evidence-record schema and gaps

Claims whose repeatability or auditability depends on configuration,
tool identity, inputs or preserved output should record the applicable
fields below. The owner selects the fields and preservation method needed
for each claim. Tool-version capture and evidence archival are named
obligations for claims that require them, not universal AXIOM-PLAN-002
gates. Adoption of this candidate schema remains subject to owner review.

| Field | Meaning |
|---|---|
| `claim` | The single statement being supported |
| `requirement` | Requirement or decision ID (§7) |
| `commit` | Full SHA of the inspected tree |
| `configuration` | Feature set, target triple, profile |
| `tool` / `version` | Tool used and its observed version, or `UNKNOWN` |
| `command` | Exact command line |
| `input` / `seed` | Input corpus or PRNG seed where applicable |
| `result` / `exit_status` | Observed outcome and process exit status |
| `artifact` / `hash` | Stored artifact path and content hash |
| `assumptions` | Assumption IDs the claim depends on |
| `limitations` | What the claim does not establish |
| `evidence_class` | live-runtime / host-model / static / formal-model / hardware |
| `review_status` | `unreviewed` / `owner-reviewed` / `rejected` |

Current gaps:

| ID | Gap |
|---|---|
| EV-G-1 | The identifiable stack-review artifact is preserved under gitignored `target/`, so no fresh checkout reproduces it and no durable archive location is recorded; this does not withdraw its prior scoped review acceptance |
| EV-G-2 | Runtime occupancy of task, endpoint, capability and event-ring ceilings is unmeasured (§3.1) |
| EV-G-3 | Loader, fs and storage robustness evidence is host-model only; no equivalent on-target evidence |
| EV-G-4 | No runtime stack high-water measurement in any configuration |
| EV-G-5 | No hardware evidence of any kind |
| EV-G-6 | Separate address-space roots are constructed, but complete per-service mapping contents, ownership, shared regions and negative isolation evidence are not established (AXIOM-FOUND-009) |

## 10. Tool inventory

Versions observed in the inspection environment at the inspected commit.
Tools that could not be observed are recorded as `UNKNOWN` rather than
assumed.

| Tool | Version | Basis |
|---|---|---|
| `rustc` | 1.96.1 (`31fca3adb`, 2026-06-26), LLVM 22.1.2 | observed |
| `cargo` | 1.96.1 (`356927216`, 2026-06-26) | observed |
| `gdb` | GNU gdb 17.2, supports `riscv:rv64` | observed |
| `qemu-system-riscv64` | **UNKNOWN** | not present in the inspection sandbox; QEMU suites run in a different environment |
| `coqc` | **UNKNOWN** | not present in the inspection sandbox |
| `objdump` (binutils) | present but **does not support** riscv64 (`architecture: UNKNOWN!`) | observed |
| OpenSBI | **UNKNOWN** | supplied by QEMU `-bios default`; version not recorded by any suite |

Recording a version as `UNKNOWN` is a gap, not a defect in the tool.
For a claim that depends on tool identity or reproducibility, its evidence
plan must capture the applicable version and preservation information.
Pinning or archival is required only when that claim's accepted method
requires it; it is not a universal gate for this draft.

## 11. Review roles

| Role | Holder | Authority |
|---|---|---|
| Architecture decision authority | Project owner (human) | Approves or rejects every entry in §6; only the owner may change a status from OPEN |
| Implementation assistant | AI assistant | Implements precisely specified tasks only; never designs, never approves, must stop and report when a decision is required (docs/07 §1) |
| Change reviewer | Project owner (human) | Applies the docs/07 §4 checklist to each change |
| Independent assessor | **NOT APPOINTED** | No independent reviewer, assessor or certification body exists for this project. No independent review has taken place. This row must not be filled in with an assumed party |

## 12. Draft status

This document is a draft. AXIOM-PLAN-002 is **not** closed by its
existence; see the acceptance conditions recorded with the task that
produced it. Every decision in §6 remains OPEN, and no architectural
choice is promoted to approved status by this document. Explicitly
deferred decisions block only the tasks or claims named in §6; decisions
not needed for those tasks are not silently turned into prerequisites for
reviewing this draft.
