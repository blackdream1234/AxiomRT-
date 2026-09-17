# AxiomRT — Controlled Engineering Workplan and Codex Prompt Pack
Version: 0.1 — 2026-09-08
Status: PROPOSED. This document is a planning artifact, not architecture approval.
Owner: Youssef Arfouy. Implementation assistant: Codex.
Language: execution prompts in English; operating summary in Arabic.

## 1. طريقة العمل
هذه الخطة لا تمنح Codex تفويضاً لتنفيذ جميع المراحل. نفّذ مهمة واحدة فقط في كل دورة.
ابدأ بتدقيق الحالة، ثم إعداد مواصفة، ثم اعتمادها من المالك، ثم التنفيذ والتحقق والمراجعة.
المهام المستقبلية أدناه هي prompts لتحديد النطاق وإعداد المواصفات، وليست تعليمات تعديل جاهزة.
لا يمكن تثبيت allowed-files وABI للمرحلة E اعتماداً على شجرة المرحلة A.
حالة كل مهمة: PROPOSED → SPECIFIED → APPROVED → IMPLEMENTING → VERIFIED → REVIEWED → ACCEPTED.
BLOCKED وREJECTED حالتان صريحتان. VERIFIED لا تعني ACCEPTED.

## 2. Observed baseline, not a claim about the latest server state
Locally inspected review worktree: /workspace/scratch/8f3cb250aeb5/AxiomRT-review.
Detached HEAD: d13bdab7967a367210869b407d91e5674797f2c7; tracked status clean.
Local main workspace: /workspace/scratch/8f3cb250aeb5/AxiomRT.
Branch v1.8-robustness-fuzzing was one commit behind its cached tracking ref.
No fetch or branch movement was performed. New baseline audit must reconcile user-machine state.
Loader target and six corpus files exist in the inspected review tree.
No builds, QEMU tests, Coq proofs or fuzz campaigns were rerun for this plan.
Existing results are historical reports until independently reproduced.
The inspected verify_all.sh explicitly lists 16 QEMU suites, host suites and
three Coq compilations. It does not explicitly run axiom-fuzz tests and the
final default-build command is not folded into its fail accumulator: audit
these as verification gaps before trusting its aggregate PASS.

## 3. Scope and branch protection
- Preserve v1.7 published tags/history/evidence, .vscode/ and AxiomRTv1.7.md.
- Leave axiom-studio-001 untouched. IND-006 needs its own approved UI scope.
- Never reset to 8c9747b merely because an old prompt names it.
- No branch creation/switching, rebasing, force-push or remote writes implied.
- An owner-approved integration branch decision is required before implementation;
  do not silently turn v1.8 into a v2 rewrite.
- Existing ROBUST roadmap is retained. FOUND-010 reconciles remaining items; it
  does not declare ROBUST-008 onward complete.
- Existing architectural findings are hypotheses to revalidate at the selected
  tip, not assumed present forever.
- No new runtime dependencies, unsafe, heap-after-boot, ABI changes or generated
  tooling without explicit written task approval.

## 4. Architectural corrections to the high-level roadmap
1. Assurance planning, traceability, reproducibility and hazard/threat analysis
   start at phase 0 and accompany every task; phase C closes evidence gaps.
2. Select hardware early; minimal BSP work precedes B hardware evidence.
3. POSIX compatibility is a named subset implemented using client shims and
   user-space services, with explicitly necessary kernel mechanisms. A generic
   single personality service does not automatically provide POSIX semantics.
4. A partition manager can configure policy in user space, but trusted kernel
   mechanisms enforce CPU windows and spatial boundaries.
5. Not every RISC-V board has an IOMMU. Untrusted DMA must remain disabled or
   excluded unless actual hardware containment is demonstrated.
6. ASIDs are an optimization with correctness obligations, not a prerequisite
   for basic memory isolation; guard pages can precede them.
7. Shared model/runtime code reduces divergence but is not independent evidence
   or a formal refinement proof.
8. Measured worst observed latency is not a WCET upper bound.
9. A Rust compiler or theorem prover does not make the delivered system certified.
10. No zero-vulnerability, ARINC 653 compliance, DO-178C compliance, or superiority
    claim is authorized by this plan. This is an evaluation prototype program.

## 5. Phase gates
| Gate | Required exit evidence |
|---|---|
| G0 | Baseline, approved narrow product profile, trusted test-runner behavior, hardware feasibility and explicit integration branch decision |
| GA | Reproduced/repaired foundation defects, live negative tests, clean lifecycle, accepted residual-risk register, ROBUST reconciliation |
| GB | Single-hart object/temporal semantics, hardware boot and isolation evidence, approved model correspondence |
| GC | Scoped proofs, timing argument, fault/coverage/mutation reports, independently reviewed claims; may iterate with D |
| GD | Frozen hardware/config profile, repeatable demo and evaluation kit, explicit limitations, independent reproduction |
| GE | Separately funded/approved multicore, update, platforms and assessment scopes; no automatic entry |

No schedule or budget guarantee is inferred. Estimate each approved task after its
baseline and test feasibility are known. Limit work in progress to one implementation
task. Split broad rows below into approved child tasks; one green commit per child.

## 6. Universal Codex execution contract
Prepend this contract to every approved implementation prompt.

```text
You are the implementation assistant for AxiomRT, not its architecture approver.
Read applicable AGENTS.md and docs/07_CODEX_RULES.md completely.
Execute exactly one owner-approved task. Do not begin the next task.

Reject implementation unless the brief contains:
Task ID; exact base branch/full commit; approved requirement IDs and document
sections; approved design/ADR; exhaustive literal allowed-file paths (including
new tests/docs/evidence); forbidden files (all others); exact observable behavior,
errors/state transitions; exact tests; documentation updates; checkable DoD;
rollback condition and permitted recovery procedure.
No placeholders, directory-wide wildcards or "related files" in the allowlist.

Before changes:
- inspect status and preserve user work; verify the approved baseline;
- read requirements and affected implementation; report contradictions;
- record necessary tool versions and approved dependencies;
- if a decision is absent, stop without choosing architecture.

For a confirmed bug:
- first create a minimized deterministic reproducer and run it on the unfixed
  implementation; preserve its command, expected/actual result and failure log;
- do not call a crash of the harness proof of the kernel defect;
- then implement the approved smallest fix and rerun the same regression;
- retain the regression; do not weaken assertions or suppress failures.
Regression-before-fix is an ordering/evidence requirement. The final task commit
must be green; do not create an intentionally red intermediate commit unless an
explicitly approved workflow allows it.

Validate:
- targeted host tests and actual live-path/QEMU tests for kernel behavior;
- applicable fuzz target: same seed twice, different seed once, corpus replay;
- compare canonical outcome/trace/corpus digests, excluding documented volatile
  fields, and record PRNG version, exact commands, iterations and timeout;
- fmt and clippy for actual supported configurations, not guessed invocations;
- approved full verification script, including all relevant fuzz/tool suites;
- hardware tests for hardware claims. Missing tools/hardware => BLOCKED/NOT RUN,
  never PASS. Do not install dependencies or contact external systems implicitly.

Check scoped diff, docs-to-code traceability, unsafe justifications and residual
risks. Report failures honestly; no commit if the approved gate is not met.
Create one task commit only if the brief explicitly authorizes it; never push,
merge, tag, change protected history or automatically start another task.

Report: Task and baseline; diagnosis; changes; requirement-to-test mapping;
pre-fix/post-fix evidence; exact commands/exit codes; PASS/FAIL/BLOCKED/NOT RUN;
changed files; unsafe/dependencies/ABI changes; remaining risks; commit if any;
review checklist and rollback target.
Stop for human review. AI self-review is not independent assessment.
```

## 7. First runnable prompt: read-only baseline
Copy only this prompt for the first Codex turn.

```text
Task ID: AXIOM-PLAN-001
Mode: READ-ONLY BASELINE AUDIT. Do not implement any fix.

Requirement references:
- docs/07_CODEX_RULES.md (AXIOM-DOC-008), sections 1-5 and 8.
- docs/00_PROJECT_CHARTER.md; docs/01_SCOPE_AND_NON_GOALS.md.
- docs/11_VERIFICATION_PLAN.md; docs/36_ROBUSTNESS_AND_FUZZING.md.
Read their actual contents and cite relevant sections; report contradictions.

Allowed file writes: NONE. Reading project sources, history, configuration
and existing evidence is permitted. Output the report in the response only.
Forbidden writes: ALL files, including .vscode/, AxiomRTv1.7.md,
published v1.7 artifacts/history and the axiom-studio-001 branch/worktree.
No checkout, fetch, pull, reset, clean, commit, push, tool installation,
test execution, build, dependency change or architecture change.

Expected behavior:
1. Identify repository root; read every applicable AGENTS.md and Codex rule.
2. Report branch, full HEAD, worktrees, tracked/untracked changes and local
   remote-tracking refs. Preserve user changes; do not print secrets.
3. Historical reference only: ROBUST-006=8c9747b; reviewed ROBUST-007=
   d13bdab7967a367210869b407d91e5674797f2c7. Never reset to these hashes.
   Verify ancestry and whether current tip is newer. Remote-tracking refs
   are cached observations, not evidence of current server state.
4. Inspect installed tool availability without installing anything.
5. Inspect existing verification commands and evidence provenance; do not
   rerun tests in this read-only task. Distinguish observed code, historical
   reported test results and unverified assumptions.
6. Recheck alleged defects: shared IPC payload; broad lifecycle authority;
   service operation rights; fault classification/store/ACK; restart/unload;
   idle/watchdog progress; per-service mappings. Cite exact source symbols.
   Record the 64-byte loader FS receive window and current reachability.
7. Audit verify_all.sh child/final-build exit statuses and whether fuzz
   targets are covered. Do not repair it.
8. Enumerate unresolved ROBUST tasks, preserved blockers and roadmap overlap.
9. For FOUND-001, identify exact candidate files and required design decisions.
   Do not approve a design or emit an implementation-ready task with unknowns.

Tests required: NO test execution; this audit must not generate build outputs.
Validation: read-only git/source consistency checks and tool availability.
Documentation update: NONE.
Definition of done: report includes baseline, findings with confidence,
evidence inventory, blockers, candidate IPC task scope and next approval gate.
Rollback condition: on unexpected writes or scope uncertainty stop and
report exact affected paths; do not discard or automatically revert user data.

Required final sections:
Baseline / Rules / Findings / Verification status / Open decisions /
Candidate next task / Changed files (must be None).
Stop after this task.
```

## 8. Task catalogue
Dependencies use the suffixes of full AXIOM task IDs. These are proposed planning
IDs; PLAN-001 must check for collisions. Technical objectives below are candidates
for owner approval, not amendments to approved project architecture.

| Task ID | Phase | Scope | Dependencies |
|---|---|---|---|
| AXIOM-PLAN-001 | 0 | Baseline and discrepancy register | None |
| AXIOM-PLAN-002 | 0 | Product profile and assurance foundation | PLAN-001 |
| AXIOM-PLAN-003 | 0 | Verification runner integrity | PLAN-001 |
| AXIOM-PLAN-004 | 0 | Hardware feasibility decision | PLAN-002 |
| AXIOM-FOUND-001 | A | IPC payload ownership | PLAN-001, PLAN-003 |
| AXIOM-FOUND-002 | A | Scoped lifecycle capabilities | FOUND-001, PLAN-002 |
| AXIOM-FOUND-003 | A | Enforceable service operation rights | FOUND-002 |
| AXIOM-FOUND-004 | A | Truthful fault classification | FOUND-001 |
| AXIOM-FOUND-005 | A | Bounded FaultStore and delivery | FOUND-002, FOUND-004 |
| AXIOM-FOUND-006 | A | Fault acknowledgement ABI and recovery | FOUND-005 |
| AXIOM-FOUND-007 | A | Clean restart and loader lifecycle | FOUND-006 |
| AXIOM-FOUND-008 | A | Idle path and watchdog progress | FOUND-007 |
| AXIOM-FOUND-009 | A | Per-service address-space mappings | FOUND-007 |
| AXIOM-FOUND-010 | A | Foundation acceptance and robustness reconciliation | FOUND-001 through FOUND-009 |
| AXIOM-CORE-001 | B | KernelState ownership | FOUND-010 |
| AXIOM-CORE-002 | B | Validated typed IDs and endpoint generations | CORE-001 |
| AXIOM-CORE-003 | B | Scheduling contexts | CORE-002 |
| AXIOM-CORE-004 | B | Budget enforcement and IPC accounting | CORE-003 |
| AXIOM-CORE-005 | B | Guard pages and ASID lifecycle | CORE-002, PLAN-004 |
| AXIOM-CORE-006 | B | Runtime/model correspondence | CORE-001 through CORE-004 |
| AXIOM-CORE-007 | B | Single-hart hardware baseline | CORE-004, CORE-005, PLAN-004 |
| AXIOM-EVID-001 | C | Executable specification and refinement scope | PLAN-002, CORE-006 |
| AXIOM-EVID-002 | C | Information-flow argument | EVID-001, FOUND-009 |
| AXIOM-EVID-003 | C | Timing analysis | CORE-007, CORE-004 |
| AXIOM-EVID-004 | C | Fault injection, coverage and mutation | FOUND-010, CORE-006 |
| AXIOM-EVID-005 | C | Assurance case and independent assessment | EVID-001 through EVID-004 |
| AXIOM-IND-001 | D | Static configuration and deterministic startup | PLAN-002, CORE-004 |
| AXIOM-IND-002 | D | BSP and DMA containment | CORE-007, IND-001 |
| AXIOM-IND-003 | D | Isolated block and network drivers | IND-002, FOUND-003 |
| AXIOM-IND-004 | D | Limited POSIX personality | FOUND-003, IND-001 |
| AXIOM-IND-005 | D | ARINC-inspired partition manager | IND-001, CORE-004, EVID-003 |
| AXIOM-IND-006 | D | Studio evidence interface | FOUND-006, IND-005 |
| AXIOM-IND-007 | D | Reproducible evaluation kit | IND-003 through IND-006, EVID-005 |
| AXIOM-ADV-001 | E | Multicore and interference control | IND-007, new funded product scope |
| AXIOM-ADV-002 | E | Verified IPC fastpath | ADV-001 or explicit single-hart scope |
| AXIOM-ADV-003 | E | Live update and high availability | IND-007, new product scope |
| AXIOM-ADV-004 | E | Platform expansion and assurance program | IND-007, independent assessor engagement |

## 9. Per-step specification prompts
For each row, send the corresponding prompt AFTER its dependencies have been
accepted. These prompts allow read-only inspection and response-only draft work.
They do not permit Codex to decide an architecture absent from approved documents.
Owner/architect resolves listed decisions; a separate docs-only task records the
approved ADR/requirements before implementation.

### AXIOM-PLAN-002 — Product profile and assurance foundation
```text
Task ID: AXIOM-PLAN-002
Mode: READ-ONLY SCOPE AND SPECIFICATION DRAFT; not implementation approval.
References: docs/07_CODEX_RULES.md sections 1-5 and 8, this workplan section 9
(AXIOM-PLAN-002), and the actual approved project requirement sections found by inspection.
Dependencies to verify: PLAN-001.
Allowed file writes: NONE. Forbidden writes: ALL files and git refs.
Expected behavior: inspect current sources/docs and prepare a response-only
implementation brief for this scope:
Propose one single-hart RV64 evaluation profile, bounded resources, trust boundaries, threat model, safety assumptions and non-goals. Define requirement IDs, hazard/threat registers, evidence schema, tool inventory and review roles. Separate evaluation from safety-critical deployment.
Acceptance objective:
Approved product profile; every architectural goal has a measurable acceptance criterion or an explicitly open decision.
Identify exact requirement references, literal candidate allowed files, actual
test commands/targets, pre-fix regression design where relevant, documentation
files, state/error semantics, unsafe/dependency needs and rollback conditions.
Mark all unapproved choices DECISION REQUIRED; do not select them yourself.
If scope exceeds one reviewable change, produce separately bounded child briefs
with collision-checked AXIOM-AREA-NNN IDs. Do not execute them.
Tests required now: read-only source/history/command inspection only; no tests
or builds. Enumerate tests required later and missing prerequisites.
Documentation update now: NONE.
Definition of done: complete candidate brief(s), traced requirements, decision
register, checkable gate and explicit approval boundary.
Rollback: no writes expected; stop/report unexpected mutation without discarding
user work. No commit, push, checkout, installation or next-task execution.
```

### AXIOM-PLAN-003 — Verification runner integrity
```text
Task ID: AXIOM-PLAN-003
Mode: READ-ONLY SCOPE AND SPECIFICATION DRAFT; not implementation approval.
References: docs/07_CODEX_RULES.md sections 1-5 and 8, this workplan section 9
(AXIOM-PLAN-003), and the actual approved project requirement sections found by inspection.
Dependencies to verify: PLAN-001.
Allowed file writes: NONE. Forbidden writes: ALL files and git refs.
Expected behavior: inspect current sources/docs and prepare a response-only
implementation brief for this scope:
Review exit status propagation, timeouts, missing-tool handling and suite coverage. Inspect scripts/verify_all.sh final default-build status and whether axiom-fuzz tests are included. Demonstrate each confirmed runner defect before repair; never treat a log string alone as success.
Acceptance objective:
Deliberately failed child command and final build produce nonzero result; missing tool is BLOCKED; relevant fuzz suites are accounted for.
Identify exact requirement references, literal candidate allowed files, actual
test commands/targets, pre-fix regression design where relevant, documentation
files, state/error semantics, unsafe/dependency needs and rollback conditions.
Mark all unapproved choices DECISION REQUIRED; do not select them yourself.
If scope exceeds one reviewable change, produce separately bounded child briefs
with collision-checked AXIOM-AREA-NNN IDs. Do not execute them.
Tests required now: read-only source/history/command inspection only; no tests
or builds. Enumerate tests required later and missing prerequisites.
Documentation update now: NONE.
Definition of done: complete candidate brief(s), traced requirements, decision
register, checkable gate and explicit approval boundary.
Rollback: no writes expected; stop/report unexpected mutation without discarding
user work. No commit, push, checkout, installation or next-task execution.
```

### AXIOM-PLAN-004 — Hardware feasibility decision
```text
Task ID: AXIOM-PLAN-004
Mode: READ-ONLY SCOPE AND SPECIFICATION DRAFT; not implementation approval.
References: docs/07_CODEX_RULES.md sections 1-5 and 8, this workplan section 9
(AXIOM-PLAN-004), and the actual approved project requirement sections found by inspection.
Dependencies to verify: PLAN-002.
Allowed file writes: NONE. Forbidden writes: ALL files and git refs.
Expected behavior: inspect current sources/docs and prepare a response-only
implementation brief for this scope:
Produce a board-selection ADR using authoritative board/SoC manuals. Check Sv39, interrupt/timer interface, boot firmware, cache, timer frequency, debug, DMA/IOMMU and accessible physical hardware. Specify unavailable-device exclusions. Do not purchase equipment or assume QEMU device equivalence.
Acceptance objective:
Named candidate board and evidence gaps; procurement is a separate user decision. Freeze hardware profile before hardware-dependent B tasks.
Identify exact requirement references, literal candidate allowed files, actual
test commands/targets, pre-fix regression design where relevant, documentation
files, state/error semantics, unsafe/dependency needs and rollback conditions.
Mark all unapproved choices DECISION REQUIRED; do not select them yourself.
If scope exceeds one reviewable change, produce separately bounded child briefs
with collision-checked AXIOM-AREA-NNN IDs. Do not execute them.
Tests required now: read-only source/history/command inspection only; no tests
or builds. Enumerate tests required later and missing prerequisites.
Documentation update now: NONE.
Definition of done: complete candidate brief(s), traced requirements, decision
register, checkable gate and explicit approval boundary.
Rollback: no writes expected; stop/report unexpected mutation without discarding
user work. No commit, push, checkout, installation or next-task execution.
```

### AXIOM-FOUND-001 — IPC payload ownership
```text
Task ID: AXIOM-FOUND-001
Mode: READ-ONLY SCOPE AND SPECIFICATION DRAFT; not implementation approval.
References: docs/07_CODEX_RULES.md sections 1-5 and 8, this workplan section 9
(AXIOM-FOUND-001), and the actual approved project requirement sections found by inspection.
Dependencies to verify: PLAN-001, PLAN-003.
Allowed file writes: NONE. Forbidden writes: ALL files and git refs.
Expected behavior: inspect current sources/docs and prepare a response-only
implementation brief for this scope:
First reproduce two blocked senders with distinct payloads on distinct endpoints before either receive, using the live dispatch path. Specify bounded payload ownership per pending endpoint message; zero/maximum lengths, copy failure, cancellation, kill/restart and stale bytes. Preserve queue cardinality and ABI unless separately approved.
Acceptance objective:
Regression fails on baseline and passes after fix on the live path; no cross-endpoint data substitution or delivery of uninitialized bytes.
Identify exact requirement references, literal candidate allowed files, actual
test commands/targets, pre-fix regression design where relevant, documentation
files, state/error semantics, unsafe/dependency needs and rollback conditions.
Mark all unapproved choices DECISION REQUIRED; do not select them yourself.
If scope exceeds one reviewable change, produce separately bounded child briefs
with collision-checked AXIOM-AREA-NNN IDs. Do not execute them.
Tests required now: read-only source/history/command inspection only; no tests
or builds. Enumerate tests required later and missing prerequisites.
Documentation update now: NONE.
Definition of done: complete candidate brief(s), traced requirements, decision
register, checkable gate and explicit approval boundary.
Rollback: no writes expected; stop/report unexpected mutation without discarding
user work. No commit, push, checkout, installation or next-task execution.
```

### AXIOM-FOUND-002 — Scoped lifecycle capabilities
```text
Task ID: AXIOM-FOUND-002
Mode: READ-ONLY SCOPE AND SPECIFICATION DRAFT; not implementation approval.
References: docs/07_CODEX_RULES.md sections 1-5 and 8, this workplan section 9
(AXIOM-FOUND-002), and the actual approved project requirement sections found by inspection.
Dependencies to verify: FOUND-001, PLAN-002.
Allowed file writes: NONE. Forbidden writes: ALL files and git refs.
Expected behavior: inspect current sources/docs and prepare a response-only
implementation brief for this scope:
Define target/domain-scoped lifecycle authority for loader, driver manager and supervisor. Specify capability representation, derivation, revocation, generation lifetime and explicit errors. Do not silently add broad control as a compatibility fallback.
Acceptance objective:
Loader cannot control unrelated tasks; denied requests preserve state; permitted lifecycle operations remain functional.
Identify exact requirement references, literal candidate allowed files, actual
test commands/targets, pre-fix regression design where relevant, documentation
files, state/error semantics, unsafe/dependency needs and rollback conditions.
Mark all unapproved choices DECISION REQUIRED; do not select them yourself.
If scope exceeds one reviewable change, produce separately bounded child briefs
with collision-checked AXIOM-AREA-NNN IDs. Do not execute them.
Tests required now: read-only source/history/command inspection only; no tests
or builds. Enumerate tests required later and missing prerequisites.
Documentation update now: NONE.
Definition of done: complete candidate brief(s), traced requirements, decision
register, checkable gate and explicit approval boundary.
Rollback: no writes expected; stop/report unexpected mutation without discarding
user work. No commit, push, checkout, installation or next-task execution.
```

### AXIOM-FOUND-003 — Enforceable service operation rights
```text
Task ID: AXIOM-FOUND-003
Mode: READ-ONLY SCOPE AND SPECIFICATION DRAFT; not implementation approval.
References: docs/07_CODEX_RULES.md sections 1-5 and 8, this workplan section 9
(AXIOM-FOUND-003), and the actual approved project requirement sections found by inspection.
Dependencies to verify: FOUND-002.
Allowed file writes: NONE. Forbidden writes: ALL files and git refs.
Expected behavior: inspect current sources/docs and prepare a response-only
implementation brief for this scope:
Prepare owner-approved selection of operation-specific endpoints or kernel-authenticated badges. Keep text protocol parsing outside kernel. Specify identity propagation across retries/delegation and exact READ/LIST/INFO authorization.
Acceptance objective:
Forged identity and SEND-only authority cannot invoke a disallowed operation; legitimate clients remain compatible.
Identify exact requirement references, literal candidate allowed files, actual
test commands/targets, pre-fix regression design where relevant, documentation
files, state/error semantics, unsafe/dependency needs and rollback conditions.
Mark all unapproved choices DECISION REQUIRED; do not select them yourself.
If scope exceeds one reviewable change, produce separately bounded child briefs
with collision-checked AXIOM-AREA-NNN IDs. Do not execute them.
Tests required now: read-only source/history/command inspection only; no tests
or builds. Enumerate tests required later and missing prerequisites.
Documentation update now: NONE.
Definition of done: complete candidate brief(s), traced requirements, decision
register, checkable gate and explicit approval boundary.
Rollback: no writes expected; stop/report unexpected mutation without discarding
user work. No commit, push, checkout, installation or next-task execution.
```

### AXIOM-FOUND-004 — Truthful fault classification
```text
Task ID: AXIOM-FOUND-004
Mode: READ-ONLY SCOPE AND SPECIFICATION DRAFT; not implementation approval.
References: docs/07_CODEX_RULES.md sections 1-5 and 8, this workplan section 9
(AXIOM-FOUND-004), and the actual approved project requirement sections found by inspection.
Dependencies to verify: FOUND-001.
Allowed file writes: NONE. Forbidden writes: ALL files and git refs.
Expected behavior: inspect current sources/docs and prepare a response-only
implementation brief for this scope:
Map trap cause, task identity and generation, PC and address to precisely specified fault records. Distinguish watchdog, illegal instruction and memory fault. Limit sensitive evidence exposure and define record version.
Acceptance objective:
Injected causes are distinguishable and attributed correctly; no false RECOVERY_APPLIED event.
Identify exact requirement references, literal candidate allowed files, actual
test commands/targets, pre-fix regression design where relevant, documentation
files, state/error semantics, unsafe/dependency needs and rollback conditions.
Mark all unapproved choices DECISION REQUIRED; do not select them yourself.
If scope exceeds one reviewable change, produce separately bounded child briefs
with collision-checked AXIOM-AREA-NNN IDs. Do not execute them.
Tests required now: read-only source/history/command inspection only; no tests
or builds. Enumerate tests required later and missing prerequisites.
Documentation update now: NONE.
Definition of done: complete candidate brief(s), traced requirements, decision
register, checkable gate and explicit approval boundary.
Rollback: no writes expected; stop/report unexpected mutation without discarding
user work. No commit, push, checkout, installation or next-task execution.
```

### AXIOM-FOUND-005 — Bounded FaultStore and delivery
```text
Task ID: AXIOM-FOUND-005
Mode: READ-ONLY SCOPE AND SPECIFICATION DRAFT; not implementation approval.
References: docs/07_CODEX_RULES.md sections 1-5 and 8, this workplan section 9
(AXIOM-FOUND-005), and the actual approved project requirement sections found by inspection.
Dependencies to verify: FOUND-002, FOUND-004.
Allowed file writes: NONE. Forbidden writes: ALL files and git refs.
Expected behavior: inspect current sources/docs and prepare a response-only
implementation brief for this scope:
Specify event IDs with task generations, bounded storage, event transitions, notification queue, backpressure, overflow policy and supervisor-not-waiting behavior. Clarify concurrent-event and wraparound semantics before code.
Acceptance objective:
No silent loss; duplicates cannot create multiple state transitions; full store follows explicit bounded fail-safe policy.
Identify exact requirement references, literal candidate allowed files, actual
test commands/targets, pre-fix regression design where relevant, documentation
files, state/error semantics, unsafe/dependency needs and rollback conditions.
Mark all unapproved choices DECISION REQUIRED; do not select them yourself.
If scope exceeds one reviewable change, produce separately bounded child briefs
with collision-checked AXIOM-AREA-NNN IDs. Do not execute them.
Tests required now: read-only source/history/command inspection only; no tests
or builds. Enumerate tests required later and missing prerequisites.
Documentation update now: NONE.
Definition of done: complete candidate brief(s), traced requirements, decision
register, checkable gate and explicit approval boundary.
Rollback: no writes expected; stop/report unexpected mutation without discarding
user work. No commit, push, checkout, installation or next-task execution.
```

### AXIOM-FOUND-006 — Fault acknowledgement ABI and recovery
```text
Task ID: AXIOM-FOUND-006
Mode: READ-ONLY SCOPE AND SPECIFICATION DRAFT; not implementation approval.
References: docs/07_CODEX_RULES.md sections 1-5 and 8, this workplan section 9
(AXIOM-FOUND-006), and the actual approved project requirement sections found by inspection.
Dependencies to verify: FOUND-005.
Allowed file writes: NONE. Forbidden writes: ALL files and git refs.
Expected behavior: inspect current sources/docs and prepare a response-only
implementation brief for this scope:
Resolve docs/04 versus live ABI through an approved ADR before code. Specify a0/a1/a2 or approved alternative, capability/event/decision validation, exact errors, stale/duplicate handling, action execution and failure reporting. Update every ABI consumer within an exhaustive allowlist.
Acceptance objective:
Only authorized pending events can trigger the specified action; applied evidence follows completed action; invalid ACK changes no protected state.
Identify exact requirement references, literal candidate allowed files, actual
test commands/targets, pre-fix regression design where relevant, documentation
files, state/error semantics, unsafe/dependency needs and rollback conditions.
Mark all unapproved choices DECISION REQUIRED; do not select them yourself.
If scope exceeds one reviewable change, produce separately bounded child briefs
with collision-checked AXIOM-AREA-NNN IDs. Do not execute them.
Tests required now: read-only source/history/command inspection only; no tests
or builds. Enumerate tests required later and missing prerequisites.
Documentation update now: NONE.
Definition of done: complete candidate brief(s), traced requirements, decision
register, checkable gate and explicit approval boundary.
Rollback: no writes expected; stop/report unexpected mutation without discarding
user work. No commit, push, checkout, installation or next-task execution.
```

### AXIOM-FOUND-007 — Clean restart and loader lifecycle
```text
Task ID: AXIOM-FOUND-007
Mode: READ-ONLY SCOPE AND SPECIFICATION DRAFT; not implementation approval.
References: docs/07_CODEX_RULES.md sections 1-5 and 8, this workplan section 9
(AXIOM-FOUND-007), and the actual approved project requirement sections found by inspection.
Dependencies to verify: FOUND-006.
Allowed file writes: NONE. Forbidden writes: ALL files and git refs.
Expected behavior: inspect current sources/docs and prepare a response-only
implementation brief for this scope:
Specify transactional cleanup of IPC, IRQ subscriptions, capabilities, mappings, registers, stacks and private data. Define preserved state explicitly. Unload must not leave an app executing; handle faulted/exited reload and generation changes.
Acceptance objective:
Load-state-unload-load, invalid-valid, duplicate, absent unload and faulted/exited reload preserve lifecycle and isolation invariants.
Identify exact requirement references, literal candidate allowed files, actual
test commands/targets, pre-fix regression design where relevant, documentation
files, state/error semantics, unsafe/dependency needs and rollback conditions.
Mark all unapproved choices DECISION REQUIRED; do not select them yourself.
If scope exceeds one reviewable change, produce separately bounded child briefs
with collision-checked AXIOM-AREA-NNN IDs. Do not execute them.
Tests required now: read-only source/history/command inspection only; no tests
or builds. Enumerate tests required later and missing prerequisites.
Documentation update now: NONE.
Definition of done: complete candidate brief(s), traced requirements, decision
register, checkable gate and explicit approval boundary.
Rollback: no writes expected; stop/report unexpected mutation without discarding
user work. No commit, push, checkout, installation or next-task execution.
```

### AXIOM-FOUND-008 — Idle path and watchdog progress
```text
Task ID: AXIOM-FOUND-008
Mode: READ-ONLY SCOPE AND SPECIFICATION DRAFT; not implementation approval.
References: docs/07_CODEX_RULES.md sections 1-5 and 8, this workplan section 9
(AXIOM-FOUND-008), and the actual approved project requirement sections found by inspection.
Dependencies to verify: FOUND-007.
Allowed file writes: NONE. Forbidden writes: ALL files and git refs.
Expected behavior: inspect current sources/docs and prepare a response-only
implementation brief for this scope:
Specify idle scheduling, interrupt-enabled sleep and lost-wakeup avoidance on supported hardware. Replace syscall-entry-as-progress with an approved progress/temporal contract. Bound recovery activity.
Acceptance objective:
All-blocked system wakes for timer/IRQ; invalid syscall spam does not satisfy meaningful progress; idle is not reported as an application fault.
Identify exact requirement references, literal candidate allowed files, actual
test commands/targets, pre-fix regression design where relevant, documentation
files, state/error semantics, unsafe/dependency needs and rollback conditions.
Mark all unapproved choices DECISION REQUIRED; do not select them yourself.
If scope exceeds one reviewable change, produce separately bounded child briefs
with collision-checked AXIOM-AREA-NNN IDs. Do not execute them.
Tests required now: read-only source/history/command inspection only; no tests
or builds. Enumerate tests required later and missing prerequisites.
Documentation update now: NONE.
Definition of done: complete candidate brief(s), traced requirements, decision
register, checkable gate and explicit approval boundary.
Rollback: no writes expected; stop/report unexpected mutation without discarding
user work. No commit, push, checkout, installation or next-task execution.
```

### AXIOM-FOUND-009 — Per-service address-space mappings
```text
Task ID: AXIOM-FOUND-009
Mode: READ-ONLY SCOPE AND SPECIFICATION DRAFT; not implementation approval.
References: docs/07_CODEX_RULES.md sections 1-5 and 8, this workplan section 9
(AXIOM-FOUND-009), and the actual approved project requirement sections found by inspection.
Dependencies to verify: FOUND-007.
Allowed file writes: NONE. Forbidden writes: ALL files and git refs.
Expected behavior: inspect current sources/docs and prepare a response-only
implementation brief for this scope:
Specify individual linker sections and mappings for text, rodata, private data and stacks; identify minimal shared runtime. Validate linker relocations, page granularity, physical ownership and shared regions.
Acceptance objective:
PTE inspection and on-target negative access tests agree; no unintended cross-service data/code mappings, W+X or kernel user mappings.
Identify exact requirement references, literal candidate allowed files, actual
test commands/targets, pre-fix regression design where relevant, documentation
files, state/error semantics, unsafe/dependency needs and rollback conditions.
Mark all unapproved choices DECISION REQUIRED; do not select them yourself.
If scope exceeds one reviewable change, produce separately bounded child briefs
with collision-checked AXIOM-AREA-NNN IDs. Do not execute them.
Tests required now: read-only source/history/command inspection only; no tests
or builds. Enumerate tests required later and missing prerequisites.
Documentation update now: NONE.
Definition of done: complete candidate brief(s), traced requirements, decision
register, checkable gate and explicit approval boundary.
Rollback: no writes expected; stop/report unexpected mutation without discarding
user work. No commit, push, checkout, installation or next-task execution.
```

### AXIOM-FOUND-010 — Foundation acceptance and robustness reconciliation
```text
Task ID: AXIOM-FOUND-010
Mode: READ-ONLY SCOPE AND SPECIFICATION DRAFT; not implementation approval.
References: docs/07_CODEX_RULES.md sections 1-5 and 8, this workplan section 9
(AXIOM-FOUND-010), and the actual approved project requirement sections found by inspection.
Dependencies to verify: FOUND-001 through FOUND-009.
Allowed file writes: NONE. Forbidden writes: ALL files and git refs.
Expected behavior: inspect current sources/docs and prepare a response-only
implementation brief for this scope:
Re-run revised live-path tests and deterministic campaigns; reconcile ROBUST-008 onward with roadmap instead of silently skipping it. Keep 64-byte FS window issue tracked unless reachability is demonstrated; if demonstrated, reproduce then create a scoped repair task.
Acceptance objective:
All A tasks reviewed, original roadmap disposition explicit, regressions retained, unresolved findings have owner and disposition; no industrial-readiness claim.
Identify exact requirement references, literal candidate allowed files, actual
test commands/targets, pre-fix regression design where relevant, documentation
files, state/error semantics, unsafe/dependency needs and rollback conditions.
Mark all unapproved choices DECISION REQUIRED; do not select them yourself.
If scope exceeds one reviewable change, produce separately bounded child briefs
with collision-checked AXIOM-AREA-NNN IDs. Do not execute them.
Tests required now: read-only source/history/command inspection only; no tests
or builds. Enumerate tests required later and missing prerequisites.
Documentation update now: NONE.
Definition of done: complete candidate brief(s), traced requirements, decision
register, checkable gate and explicit approval boundary.
Rollback: no writes expected; stop/report unexpected mutation without discarding
user work. No commit, push, checkout, installation or next-task execution.
```

### AXIOM-CORE-001 — KernelState ownership
```text
Task ID: AXIOM-CORE-001
Mode: READ-ONLY SCOPE AND SPECIFICATION DRAFT; not implementation approval.
References: docs/07_CODEX_RULES.md sections 1-5 and 8, this workplan section 9
(AXIOM-CORE-001), and the actual approved project requirement sections found by inspection.
Dependencies to verify: FOUND-010.
Allowed file writes: NONE. Forbidden writes: ALL files and git refs.
Expected behavior: inspect current sources/docs and prepare a response-only
implementation brief for this scope:
Propose explicit single-hart interrupt/reentrancy ownership rules. Migrate one subsystem per approved child task; avoid an unsound global mutable borrow across callbacks/traps. Separate data movement from semantic changes.
Acceptance objective:
Equivalent pre/post behavior and defined aliasing/interrupt invariants; no broad all-at-once dispatcher rewrite.
Identify exact requirement references, literal candidate allowed files, actual
test commands/targets, pre-fix regression design where relevant, documentation
files, state/error semantics, unsafe/dependency needs and rollback conditions.
Mark all unapproved choices DECISION REQUIRED; do not select them yourself.
If scope exceeds one reviewable change, produce separately bounded child briefs
with collision-checked AXIOM-AREA-NNN IDs. Do not execute them.
Tests required now: read-only source/history/command inspection only; no tests
or builds. Enumerate tests required later and missing prerequisites.
Documentation update now: NONE.
Definition of done: complete candidate brief(s), traced requirements, decision
register, checkable gate and explicit approval boundary.
Rollback: no writes expected; stop/report unexpected mutation without discarding
user work. No commit, push, checkout, installation or next-task execution.
```

### AXIOM-CORE-002 — Validated typed IDs and endpoint generations
```text
Task ID: AXIOM-CORE-002
Mode: READ-ONLY SCOPE AND SPECIFICATION DRAFT; not implementation approval.
References: docs/07_CODEX_RULES.md sections 1-5 and 8, this workplan section 9
(AXIOM-CORE-002), and the actual approved project requirement sections found by inspection.
Dependencies to verify: CORE-001.
Allowed file writes: NONE. Forbidden writes: ALL files and git refs.
Expected behavior: inspect current sources/docs and prepare a response-only
implementation brief for this scope:
Specify checked ID constructors, bounds, object generations, stale-handle rejection and generation exhaustion. Bind blocked IPC and replies to correct object lifetime.
Acceptance objective:
Malformed/stale IDs fail before indexing or mutation; generation wrap cannot resurrect old authority.
Identify exact requirement references, literal candidate allowed files, actual
test commands/targets, pre-fix regression design where relevant, documentation
files, state/error semantics, unsafe/dependency needs and rollback conditions.
Mark all unapproved choices DECISION REQUIRED; do not select them yourself.
If scope exceeds one reviewable change, produce separately bounded child briefs
with collision-checked AXIOM-AREA-NNN IDs. Do not execute them.
Tests required now: read-only source/history/command inspection only; no tests
or builds. Enumerate tests required later and missing prerequisites.
Documentation update now: NONE.
Definition of done: complete candidate brief(s), traced requirements, decision
register, checkable gate and explicit approval boundary.
Rollback: no writes expected; stop/report unexpected mutation without discarding
user work. No commit, push, checkout, installation or next-task execution.
```

### AXIOM-CORE-003 — Scheduling contexts
```text
Task ID: AXIOM-CORE-003
Mode: READ-ONLY SCOPE AND SPECIFICATION DRAFT; not implementation approval.
References: docs/07_CODEX_RULES.md sections 1-5 and 8, this workplan section 9
(AXIOM-CORE-003), and the actual approved project requirement sections found by inspection.
Dependencies to verify: CORE-002.
Allowed file writes: NONE. Forbidden writes: ALL files and git refs.
Expected behavior: inspect current sources/docs and prepare a response-only
implementation brief for this scope:
Define budget/period, admission, replenishment and accounting ownership as executable transition rules. Choose policy explicitly; do not infer that priority equals temporal isolation.
Acceptance objective:
Reference transition tests cover all boundary cases; resource bounds and scheduler API approved before live integration.
Identify exact requirement references, literal candidate allowed files, actual
test commands/targets, pre-fix regression design where relevant, documentation
files, state/error semantics, unsafe/dependency needs and rollback conditions.
Mark all unapproved choices DECISION REQUIRED; do not select them yourself.
If scope exceeds one reviewable change, produce separately bounded child briefs
with collision-checked AXIOM-AREA-NNN IDs. Do not execute them.
Tests required now: read-only source/history/command inspection only; no tests
or builds. Enumerate tests required later and missing prerequisites.
Documentation update now: NONE.
Definition of done: complete candidate brief(s), traced requirements, decision
register, checkable gate and explicit approval boundary.
Rollback: no writes expected; stop/report unexpected mutation without discarding
user work. No commit, push, checkout, installation or next-task execution.
```

### AXIOM-CORE-004 — Budget enforcement and IPC accounting
```text
Task ID: AXIOM-CORE-004
Mode: READ-ONLY SCOPE AND SPECIFICATION DRAFT; not implementation approval.
References: docs/07_CODEX_RULES.md sections 1-5 and 8, this workplan section 9
(AXIOM-CORE-004), and the actual approved project requirement sections found by inspection.
Dependencies to verify: CORE-003.
Allowed file writes: NONE. Forbidden writes: ALL files and git refs.
Expected behavior: inspect current sources/docs and prepare a response-only
implementation brief for this scope:
Specify timer charge points, kernel/IRQ/recovery overhead accounting, overruns, priority inversion and bounded server execution. Budget donation is optional and requires explicit semantics; no free CPU via service invocation.
Acceptance objective:
CPU-bound and syscall-heavy tasks cannot evade approved accounting; budget exhaustion and replenishment occur as specified.
Identify exact requirement references, literal candidate allowed files, actual
test commands/targets, pre-fix regression design where relevant, documentation
files, state/error semantics, unsafe/dependency needs and rollback conditions.
Mark all unapproved choices DECISION REQUIRED; do not select them yourself.
If scope exceeds one reviewable change, produce separately bounded child briefs
with collision-checked AXIOM-AREA-NNN IDs. Do not execute them.
Tests required now: read-only source/history/command inspection only; no tests
or builds. Enumerate tests required later and missing prerequisites.
Documentation update now: NONE.
Definition of done: complete candidate brief(s), traced requirements, decision
register, checkable gate and explicit approval boundary.
Rollback: no writes expected; stop/report unexpected mutation without discarding
user work. No commit, push, checkout, installation or next-task execution.
```

### AXIOM-CORE-005 — Guard pages and ASID lifecycle
```text
Task ID: AXIOM-CORE-005
Mode: READ-ONLY SCOPE AND SPECIFICATION DRAFT; not implementation approval.
References: docs/07_CODEX_RULES.md sections 1-5 and 8, this workplan section 9
(AXIOM-CORE-005), and the actual approved project requirement sections found by inspection.
Dependencies to verify: CORE-002, PLAN-004.
Allowed file writes: NONE. Forbidden writes: ALL files and git refs.
Expected behavior: inspect current sources/docs and prepare a response-only
implementation brief for this scope:
Introduce guard-page tests first; make ASID optimization a separate child task. Specify ASID width discovery, reuse, sfence.vma behavior and page-table teardown.
Acceptance objective:
Stack overflow is contained; address-space reuse does not expose stale translations; hardware-specific assumptions documented.
Identify exact requirement references, literal candidate allowed files, actual
test commands/targets, pre-fix regression design where relevant, documentation
files, state/error semantics, unsafe/dependency needs and rollback conditions.
Mark all unapproved choices DECISION REQUIRED; do not select them yourself.
If scope exceeds one reviewable change, produce separately bounded child briefs
with collision-checked AXIOM-AREA-NNN IDs. Do not execute them.
Tests required now: read-only source/history/command inspection only; no tests
or builds. Enumerate tests required later and missing prerequisites.
Documentation update now: NONE.
Definition of done: complete candidate brief(s), traced requirements, decision
register, checkable gate and explicit approval boundary.
Rollback: no writes expected; stop/report unexpected mutation without discarding
user work. No commit, push, checkout, installation or next-task execution.
```

### AXIOM-CORE-006 — Runtime/model correspondence
```text
Task ID: AXIOM-CORE-006
Mode: READ-ONLY SCOPE AND SPECIFICATION DRAFT; not implementation approval.
References: docs/07_CODEX_RULES.md sections 1-5 and 8, this workplan section 9
(AXIOM-CORE-006), and the actual approved project requirement sections found by inspection.
Dependencies to verify: CORE-001 through CORE-004.
Allowed file writes: NONE. Forbidden writes: ALL files and git refs.
Expected behavior: inspect current sources/docs and prepare a response-only
implementation brief for this scope:
Identify transition logic actually shared with live execution and retain an independent oracle for differential testing. Define hardware adapters and trusted assumptions; sharing code is not a refinement proof.
Acceptance objective:
Adversarial traces compare independent spec and runtime outcomes; no host-only model is presented as live-kernel evidence.
Identify exact requirement references, literal candidate allowed files, actual
test commands/targets, pre-fix regression design where relevant, documentation
files, state/error semantics, unsafe/dependency needs and rollback conditions.
Mark all unapproved choices DECISION REQUIRED; do not select them yourself.
If scope exceeds one reviewable change, produce separately bounded child briefs
with collision-checked AXIOM-AREA-NNN IDs. Do not execute them.
Tests required now: read-only source/history/command inspection only; no tests
or builds. Enumerate tests required later and missing prerequisites.
Documentation update now: NONE.
Definition of done: complete candidate brief(s), traced requirements, decision
register, checkable gate and explicit approval boundary.
Rollback: no writes expected; stop/report unexpected mutation without discarding
user work. No commit, push, checkout, installation or next-task execution.
```

### AXIOM-CORE-007 — Single-hart hardware baseline
```text
Task ID: AXIOM-CORE-007
Mode: READ-ONLY SCOPE AND SPECIFICATION DRAFT; not implementation approval.
References: docs/07_CODEX_RULES.md sections 1-5 and 8, this workplan section 9
(AXIOM-CORE-007), and the actual approved project requirement sections found by inspection.
Dependencies to verify: CORE-004, CORE-005, PLAN-004.
Allowed file writes: NONE. Forbidden writes: ALL files and git refs.
Expected behavior: inspect current sources/docs and prepare a response-only
implementation brief for this scope:
Port only the minimal approved boot/UART/timer/IRQ subset as child tasks. Run on physically identified hardware; record firmware, board revisions, instrumentation and load conditions.
Acceptance objective:
Physical boot, preemption, IPC, fault and isolation evidence reproducible; measured maxima are not labelled WCET bounds.
Identify exact requirement references, literal candidate allowed files, actual
test commands/targets, pre-fix regression design where relevant, documentation
files, state/error semantics, unsafe/dependency needs and rollback conditions.
Mark all unapproved choices DECISION REQUIRED; do not select them yourself.
If scope exceeds one reviewable change, produce separately bounded child briefs
with collision-checked AXIOM-AREA-NNN IDs. Do not execute them.
Tests required now: read-only source/history/command inspection only; no tests
or builds. Enumerate tests required later and missing prerequisites.
Documentation update now: NONE.
Definition of done: complete candidate brief(s), traced requirements, decision
register, checkable gate and explicit approval boundary.
Rollback: no writes expected; stop/report unexpected mutation without discarding
user work. No commit, push, checkout, installation or next-task execution.
```

### AXIOM-EVID-001 — Executable specification and refinement scope
```text
Task ID: AXIOM-EVID-001
Mode: READ-ONLY SCOPE AND SPECIFICATION DRAFT; not implementation approval.
References: docs/07_CODEX_RULES.md sections 1-5 and 8, this workplan section 9
(AXIOM-EVID-001), and the actual approved project requirement sections found by inspection.
Dependencies to verify: PLAN-002, CORE-006.
Allowed file writes: NONE. Forbidden writes: ALL files and git refs.
Expected behavior: inspect current sources/docs and prepare a response-only
implementation brief for this scope:
Formalize the selected transition system and its observable behavior. Define exact Rust refinement obligations, unsafe/hardware/compiler assumptions and proof boundaries. Carry this activity alongside A/B specifications, not only after implementation.
Acceptance objective:
Versioned statements linked to runtime symbols and tests; TODO, axioms and unsupported paths clearly exposed.
Identify exact requirement references, literal candidate allowed files, actual
test commands/targets, pre-fix regression design where relevant, documentation
files, state/error semantics, unsafe/dependency needs and rollback conditions.
Mark all unapproved choices DECISION REQUIRED; do not select them yourself.
If scope exceeds one reviewable change, produce separately bounded child briefs
with collision-checked AXIOM-AREA-NNN IDs. Do not execute them.
Tests required now: read-only source/history/command inspection only; no tests
or builds. Enumerate tests required later and missing prerequisites.
Documentation update now: NONE.
Definition of done: complete candidate brief(s), traced requirements, decision
register, checkable gate and explicit approval boundary.
Rollback: no writes expected; stop/report unexpected mutation without discarding
user work. No commit, push, checkout, installation or next-task execution.
```

### AXIOM-EVID-002 — Information-flow argument
```text
Task ID: AXIOM-EVID-002
Mode: READ-ONLY SCOPE AND SPECIFICATION DRAFT; not implementation approval.
References: docs/07_CODEX_RULES.md sections 1-5 and 8, this workplan section 9
(AXIOM-EVID-002), and the actual approved project requirement sections found by inspection.
Dependencies to verify: EVID-001, FOUND-009.
Allowed file writes: NONE. Forbidden writes: ALL files and git refs.
Expected behavior: inspect current sources/docs and prepare a response-only
implementation brief for this scope:
Define observation model, explicit communication/declassification and confidentiality/integrity properties. Treat timing/cache/DMA channels separately. Prove only named properties/configurations.
Acceptance objective:
Machine-checked results, assumptions and exclusions recorded; no system-wide noninterference claim from a small model.
Identify exact requirement references, literal candidate allowed files, actual
test commands/targets, pre-fix regression design where relevant, documentation
files, state/error semantics, unsafe/dependency needs and rollback conditions.
Mark all unapproved choices DECISION REQUIRED; do not select them yourself.
If scope exceeds one reviewable change, produce separately bounded child briefs
with collision-checked AXIOM-AREA-NNN IDs. Do not execute them.
Tests required now: read-only source/history/command inspection only; no tests
or builds. Enumerate tests required later and missing prerequisites.
Documentation update now: NONE.
Definition of done: complete candidate brief(s), traced requirements, decision
register, checkable gate and explicit approval boundary.
Rollback: no writes expected; stop/report unexpected mutation without discarding
user work. No commit, push, checkout, installation or next-task execution.
```

### AXIOM-EVID-003 — Timing analysis
```text
Task ID: AXIOM-EVID-003
Mode: READ-ONLY SCOPE AND SPECIFICATION DRAFT; not implementation approval.
References: docs/07_CODEX_RULES.md sections 1-5 and 8, this workplan section 9
(AXIOM-EVID-003), and the actual approved project requirement sections found by inspection.
Dependencies to verify: CORE-007, CORE-004.
Allowed file writes: NONE. Forbidden writes: ALL files and git refs.
Expected behavior: inspect current sources/docs and prepare a response-only
implementation brief for this scope:
Inventory bounded loops and blocking paths, hardware interference and interrupt latency. Produce WCET analysis only where justified by method and hardware model; report remaining empirical measurements honestly.
Acceptance objective:
Per-path timing evidence with conditions and uncertainty; deadline claims supported by schedulability reasoning.
Identify exact requirement references, literal candidate allowed files, actual
test commands/targets, pre-fix regression design where relevant, documentation
files, state/error semantics, unsafe/dependency needs and rollback conditions.
Mark all unapproved choices DECISION REQUIRED; do not select them yourself.
If scope exceeds one reviewable change, produce separately bounded child briefs
with collision-checked AXIOM-AREA-NNN IDs. Do not execute them.
Tests required now: read-only source/history/command inspection only; no tests
or builds. Enumerate tests required later and missing prerequisites.
Documentation update now: NONE.
Definition of done: complete candidate brief(s), traced requirements, decision
register, checkable gate and explicit approval boundary.
Rollback: no writes expected; stop/report unexpected mutation without discarding
user work. No commit, push, checkout, installation or next-task execution.
```

### AXIOM-EVID-004 — Fault injection, coverage and mutation
```text
Task ID: AXIOM-EVID-004
Mode: READ-ONLY SCOPE AND SPECIFICATION DRAFT; not implementation approval.
References: docs/07_CODEX_RULES.md sections 1-5 and 8, this workplan section 9
(AXIOM-EVID-004), and the actual approved project requirement sections found by inspection.
Dependencies to verify: FOUND-010, CORE-006.
Allowed file writes: NONE. Forbidden writes: ALL files and git refs.
Expected behavior: inspect current sources/docs and prepare a response-only
implementation brief for this scope:
Inject failures at loader commit/rollback, IPC copy, budget expiry, fault-store overflow and restart. Distinguish code coverage, property coverage and mutation survivors; prescribe deterministic seeds and minimized corpus.
Acceptance objective:
Failures minimized before fixes; branch/target coverage limitations and surviving mutants analyzed, not hidden.
Identify exact requirement references, literal candidate allowed files, actual
test commands/targets, pre-fix regression design where relevant, documentation
files, state/error semantics, unsafe/dependency needs and rollback conditions.
Mark all unapproved choices DECISION REQUIRED; do not select them yourself.
If scope exceeds one reviewable change, produce separately bounded child briefs
with collision-checked AXIOM-AREA-NNN IDs. Do not execute them.
Tests required now: read-only source/history/command inspection only; no tests
or builds. Enumerate tests required later and missing prerequisites.
Documentation update now: NONE.
Definition of done: complete candidate brief(s), traced requirements, decision
register, checkable gate and explicit approval boundary.
Rollback: no writes expected; stop/report unexpected mutation without discarding
user work. No commit, push, checkout, installation or next-task execution.
```

### AXIOM-EVID-005 — Assurance case and independent assessment
```text
Task ID: AXIOM-EVID-005
Mode: READ-ONLY SCOPE AND SPECIFICATION DRAFT; not implementation approval.
References: docs/07_CODEX_RULES.md sections 1-5 and 8, this workplan section 9
(AXIOM-EVID-005), and the actual approved project requirement sections found by inspection.
Dependencies to verify: EVID-001 through EVID-004.
Allowed file writes: NONE. Forbidden writes: ALL files and git refs.
Expected behavior: inspect current sources/docs and prepare a response-only
implementation brief for this scope:
Assemble claim-argument-evidence with requirements, hazards, assumptions and residual risks. Have a human reviewer independent of implementation inspect agreed scope. Same-model self-review is not independent assessment.
Acceptance objective:
Signed-off scope and findings disposition; unavailable independent reviewer leaves gate pending, not automatically passed.
Identify exact requirement references, literal candidate allowed files, actual
test commands/targets, pre-fix regression design where relevant, documentation
files, state/error semantics, unsafe/dependency needs and rollback conditions.
Mark all unapproved choices DECISION REQUIRED; do not select them yourself.
If scope exceeds one reviewable change, produce separately bounded child briefs
with collision-checked AXIOM-AREA-NNN IDs. Do not execute them.
Tests required now: read-only source/history/command inspection only; no tests
or builds. Enumerate tests required later and missing prerequisites.
Documentation update now: NONE.
Definition of done: complete candidate brief(s), traced requirements, decision
register, checkable gate and explicit approval boundary.
Rollback: no writes expected; stop/report unexpected mutation without discarding
user work. No commit, push, checkout, installation or next-task execution.
```

### AXIOM-IND-001 — Static configuration and deterministic startup
```text
Task ID: AXIOM-IND-001
Mode: READ-ONLY SCOPE AND SPECIFICATION DRAFT; not implementation approval.
References: docs/07_CODEX_RULES.md sections 1-5 and 8, this workplan section 9
(AXIOM-IND-001), and the actual approved project requirement sections found by inspection.
Dependencies to verify: PLAN-002, CORE-004.
Allowed file writes: NONE. Forbidden writes: ALL files and git refs.
Expected behavior: inspect current sources/docs and prepare a response-only
implementation brief for this scope:
Specify offline manifest schema and validation of memory, rights, budgets, endpoints and startup dependencies. Define fail-closed invalid configuration and runtime binding to manifest/image hashes. Create generator only after format approval.
Acceptance objective:
Invalid configuration installs no partial system; approved configuration reproduces object graph/startup order; timing is separately measured.
Identify exact requirement references, literal candidate allowed files, actual
test commands/targets, pre-fix regression design where relevant, documentation
files, state/error semantics, unsafe/dependency needs and rollback conditions.
Mark all unapproved choices DECISION REQUIRED; do not select them yourself.
If scope exceeds one reviewable change, produce separately bounded child briefs
with collision-checked AXIOM-AREA-NNN IDs. Do not execute them.
Tests required now: read-only source/history/command inspection only; no tests
or builds. Enumerate tests required later and missing prerequisites.
Documentation update now: NONE.
Definition of done: complete candidate brief(s), traced requirements, decision
register, checkable gate and explicit approval boundary.
Rollback: no writes expected; stop/report unexpected mutation without discarding
user work. No commit, push, checkout, installation or next-task execution.
```

### AXIOM-IND-002 — BSP and DMA containment
```text
Task ID: AXIOM-IND-002
Mode: READ-ONLY SCOPE AND SPECIFICATION DRAFT; not implementation approval.
References: docs/07_CODEX_RULES.md sections 1-5 and 8, this workplan section 9
(AXIOM-IND-002), and the actual approved project requirement sections found by inspection.
Dependencies to verify: CORE-007, IND-001.
Allowed file writes: NONE. Forbidden writes: ALL files and git refs.
Expected behavior: inspect current sources/docs and prepare a response-only
implementation brief for this scope:
Extend frozen BSP in per-device child tasks. Enable DMA only with demonstrated isolation or explicit trusted-driver constraints. If board lacks IOMMU, do not implement fictitious IOMMU support or claim untrusted DMA containment.
Acceptance objective:
Device-specific IRQ/MMIO/DMA authorization and teardown tested; unsupported isolation is a documented deployment blocker.
Identify exact requirement references, literal candidate allowed files, actual
test commands/targets, pre-fix regression design where relevant, documentation
files, state/error semantics, unsafe/dependency needs and rollback conditions.
Mark all unapproved choices DECISION REQUIRED; do not select them yourself.
If scope exceeds one reviewable change, produce separately bounded child briefs
with collision-checked AXIOM-AREA-NNN IDs. Do not execute them.
Tests required now: read-only source/history/command inspection only; no tests
or builds. Enumerate tests required later and missing prerequisites.
Documentation update now: NONE.
Definition of done: complete candidate brief(s), traced requirements, decision
register, checkable gate and explicit approval boundary.
Rollback: no writes expected; stop/report unexpected mutation without discarding
user work. No commit, push, checkout, installation or next-task execution.
```

### AXIOM-IND-003 — Isolated block and network drivers
```text
Task ID: AXIOM-IND-003
Mode: READ-ONLY SCOPE AND SPECIFICATION DRAFT; not implementation approval.
References: docs/07_CODEX_RULES.md sections 1-5 and 8, this workplan section 9
(AXIOM-IND-003), and the actual approved project requirement sections found by inspection.
Dependencies to verify: IND-002, FOUND-003.
Allowed file writes: NONE. Forbidden writes: ALL files and git refs.
Expected behavior: inspect current sources/docs and prepare a response-only
implementation brief for this scope:
Specify device ownership, buffer lifetime, DMA boundaries, malformed descriptors, reset and service failure. Keep experimental drivers outside safety-critical use until their evidence is accepted.
Acceptance objective:
Driver fault or bad request cannot cross approved memory/authority boundaries; reset and queue cleanup are repeatable.
Identify exact requirement references, literal candidate allowed files, actual
test commands/targets, pre-fix regression design where relevant, documentation
files, state/error semantics, unsafe/dependency needs and rollback conditions.
Mark all unapproved choices DECISION REQUIRED; do not select them yourself.
If scope exceeds one reviewable change, produce separately bounded child briefs
with collision-checked AXIOM-AREA-NNN IDs. Do not execute them.
Tests required now: read-only source/history/command inspection only; no tests
or builds. Enumerate tests required later and missing prerequisites.
Documentation update now: NONE.
Definition of done: complete candidate brief(s), traced requirements, decision
register, checkable gate and explicit approval boundary.
Rollback: no writes expected; stop/report unexpected mutation without discarding
user work. No commit, push, checkout, installation or next-task execution.
```

### AXIOM-IND-004 — Limited POSIX personality
```text
Task ID: AXIOM-IND-004
Mode: READ-ONLY SCOPE AND SPECIFICATION DRAFT; not implementation approval.
References: docs/07_CODEX_RULES.md sections 1-5 and 8, this workplan section 9
(AXIOM-IND-004), and the actual approved project requirement sections found by inspection.
Dependencies to verify: FOUND-003, IND-001.
Allowed file writes: NONE. Forbidden writes: ALL files and git refs.
Expected behavior: inspect current sources/docs and prepare a response-only
implementation brief for this scope:
Freeze an explicit subset: names and semantics for supported calls, errors, descriptors, blocking, process/thread model and resource bounds. Use client libc/shims plus user-space services. Do not assume all POSIX can be placed in a single service without supporting kernel primitives.
Acceptance objective:
Port one selected application using only approved subset; unsupported calls fail explicitly; kernel contains no POSIX policy/parser.
Identify exact requirement references, literal candidate allowed files, actual
test commands/targets, pre-fix regression design where relevant, documentation
files, state/error semantics, unsafe/dependency needs and rollback conditions.
Mark all unapproved choices DECISION REQUIRED; do not select them yourself.
If scope exceeds one reviewable change, produce separately bounded child briefs
with collision-checked AXIOM-AREA-NNN IDs. Do not execute them.
Tests required now: read-only source/history/command inspection only; no tests
or builds. Enumerate tests required later and missing prerequisites.
Documentation update now: NONE.
Definition of done: complete candidate brief(s), traced requirements, decision
register, checkable gate and explicit approval boundary.
Rollback: no writes expected; stop/report unexpected mutation without discarding
user work. No commit, push, checkout, installation or next-task execution.
```

### AXIOM-IND-005 — ARINC-inspired partition manager
```text
Task ID: AXIOM-IND-005
Mode: READ-ONLY SCOPE AND SPECIFICATION DRAFT; not implementation approval.
References: docs/07_CODEX_RULES.md sections 1-5 and 8, this workplan section 9
(AXIOM-IND-005), and the actual approved project requirement sections found by inspection.
Dependencies to verify: IND-001, CORE-004, EVID-003.
Allowed file writes: NONE. Forbidden writes: ALL files and git refs.
Expected behavior: inspect current sources/docs and prepare a response-only
implementation brief for this scope:
Specify major-frame windows, intra-partition scheduling, communication, health policy and startup modes. Kernel enforces temporal/memory boundaries; user service configures policy but cannot substitute for privileged enforcement.
Acceptance objective:
CPU overrun, window boundary IPC, fault storm and cold/warm restart follow approved policy; no ARINC 653 compliance claim.
Identify exact requirement references, literal candidate allowed files, actual
test commands/targets, pre-fix regression design where relevant, documentation
files, state/error semantics, unsafe/dependency needs and rollback conditions.
Mark all unapproved choices DECISION REQUIRED; do not select them yourself.
If scope exceeds one reviewable change, produce separately bounded child briefs
with collision-checked AXIOM-AREA-NNN IDs. Do not execute them.
Tests required now: read-only source/history/command inspection only; no tests
or builds. Enumerate tests required later and missing prerequisites.
Documentation update now: NONE.
Definition of done: complete candidate brief(s), traced requirements, decision
register, checkable gate and explicit approval boundary.
Rollback: no writes expected; stop/report unexpected mutation without discarding
user work. No commit, push, checkout, installation or next-task execution.
```

### AXIOM-IND-006 — Studio evidence interface
```text
Task ID: AXIOM-IND-006
Mode: READ-ONLY SCOPE AND SPECIFICATION DRAFT; not implementation approval.
References: docs/07_CODEX_RULES.md sections 1-5 and 8, this workplan section 9
(AXIOM-IND-006), and the actual approved project requirement sections found by inspection.
Dependencies to verify: FOUND-006, IND-005.
Allowed file writes: NONE. Forbidden writes: ALL files and git refs.
Expected behavior: inspect current sources/docs and prepare a response-only
implementation brief for this scope:
Specify versioned telemetry with loss counters, source identity, image/config hashes and measurement timestamps. Use a separately authorized UI workstream; do not touch axiom-studio-001 under core tasks. Default to read-only; controls need authenticated scoped authority.
Acceptance objective:
No fabricated health/recovery states; disconnect and lost events visible; display reflects actual target events.
Identify exact requirement references, literal candidate allowed files, actual
test commands/targets, pre-fix regression design where relevant, documentation
files, state/error semantics, unsafe/dependency needs and rollback conditions.
Mark all unapproved choices DECISION REQUIRED; do not select them yourself.
If scope exceeds one reviewable change, produce separately bounded child briefs
with collision-checked AXIOM-AREA-NNN IDs. Do not execute them.
Tests required now: read-only source/history/command inspection only; no tests
or builds. Enumerate tests required later and missing prerequisites.
Documentation update now: NONE.
Definition of done: complete candidate brief(s), traced requirements, decision
register, checkable gate and explicit approval boundary.
Rollback: no writes expected; stop/report unexpected mutation without discarding
user work. No commit, push, checkout, installation or next-task execution.
```

### AXIOM-IND-007 — Reproducible evaluation kit
```text
Task ID: AXIOM-IND-007
Mode: READ-ONLY SCOPE AND SPECIFICATION DRAFT; not implementation approval.
References: docs/07_CODEX_RULES.md sections 1-5 and 8, this workplan section 9
(AXIOM-IND-007), and the actual approved project requirement sections found by inspection.
Dependencies to verify: IND-003 through IND-006, EVID-005.
Allowed file writes: NONE. Forbidden writes: ALL files and git refs.
Expected behavior: inspect current sources/docs and prepare a response-only
implementation brief for this scope:
Pin source, toolchain, firmware/config and instructions; include licenses, known issues, SBOM and evidence manifest. Compare two independent clean builds for claimed byte reproducibility; signing and external provisioning are separately authorized.
Acceptance objective:
An independent evaluator rebuilds and repeats selected hardware scenarios; deviations explicit; no evidence authentication claim from a bare hash.
Identify exact requirement references, literal candidate allowed files, actual
test commands/targets, pre-fix regression design where relevant, documentation
files, state/error semantics, unsafe/dependency needs and rollback conditions.
Mark all unapproved choices DECISION REQUIRED; do not select them yourself.
If scope exceeds one reviewable change, produce separately bounded child briefs
with collision-checked AXIOM-AREA-NNN IDs. Do not execute them.
Tests required now: read-only source/history/command inspection only; no tests
or builds. Enumerate tests required later and missing prerequisites.
Documentation update now: NONE.
Definition of done: complete candidate brief(s), traced requirements, decision
register, checkable gate and explicit approval boundary.
Rollback: no writes expected; stop/report unexpected mutation without discarding
user work. No commit, push, checkout, installation or next-task execution.
```

### AXIOM-ADV-001 — Multicore and interference control
```text
Task ID: AXIOM-ADV-001
Mode: READ-ONLY SCOPE AND SPECIFICATION DRAFT; not implementation approval.
References: docs/07_CODEX_RULES.md sections 1-5 and 8, this workplan section 9
(AXIOM-ADV-001), and the actual approved project requirement sections found by inspection.
Dependencies to verify: IND-007, new funded product scope.
Allowed file writes: NONE. Forbidden writes: ALL files and git refs.
Expected behavior: inspect current sources/docs and prepare a response-only
implementation brief for this scope:
Prepare new architecture/safety review for hart ownership, locking, memory ordering, TLB shootdown, cache/memory-bandwidth/DMA interference and time partitioning. Split implementation by approved mechanism.
Acceptance objective:
Single-hart evidence not reused as multicore proof; measured and bounded interference claims separated.
Identify exact requirement references, literal candidate allowed files, actual
test commands/targets, pre-fix regression design where relevant, documentation
files, state/error semantics, unsafe/dependency needs and rollback conditions.
Mark all unapproved choices DECISION REQUIRED; do not select them yourself.
If scope exceeds one reviewable change, produce separately bounded child briefs
with collision-checked AXIOM-AREA-NNN IDs. Do not execute them.
Tests required now: read-only source/history/command inspection only; no tests
or builds. Enumerate tests required later and missing prerequisites.
Documentation update now: NONE.
Definition of done: complete candidate brief(s), traced requirements, decision
register, checkable gate and explicit approval boundary.
Rollback: no writes expected; stop/report unexpected mutation without discarding
user work. No commit, push, checkout, installation or next-task execution.
```

### AXIOM-ADV-002 — Verified IPC fastpath
```text
Task ID: AXIOM-ADV-002
Mode: READ-ONLY SCOPE AND SPECIFICATION DRAFT; not implementation approval.
References: docs/07_CODEX_RULES.md sections 1-5 and 8, this workplan section 9
(AXIOM-ADV-002), and the actual approved project requirement sections found by inspection.
Dependencies to verify: ADV-001 or explicit single-hart scope.
Allowed file writes: NONE. Forbidden writes: ALL files and git refs.
Expected behavior: inspect current sources/docs and prepare a response-only
implementation brief for this scope:
Specify fastpath preconditions, equivalent slowpath semantics and fallback. Benchmark only after correctness/refinement obligations defined.
Acceptance objective:
Fastpath and reference match for admitted states; invalid preconditions fall back safely; comparable benchmarks.
Identify exact requirement references, literal candidate allowed files, actual
test commands/targets, pre-fix regression design where relevant, documentation
files, state/error semantics, unsafe/dependency needs and rollback conditions.
Mark all unapproved choices DECISION REQUIRED; do not select them yourself.
If scope exceeds one reviewable change, produce separately bounded child briefs
with collision-checked AXIOM-AREA-NNN IDs. Do not execute them.
Tests required now: read-only source/history/command inspection only; no tests
or builds. Enumerate tests required later and missing prerequisites.
Documentation update now: NONE.
Definition of done: complete candidate brief(s), traced requirements, decision
register, checkable gate and explicit approval boundary.
Rollback: no writes expected; stop/report unexpected mutation without discarding
user work. No commit, push, checkout, installation or next-task execution.
```

### AXIOM-ADV-003 — Live update and high availability
```text
Task ID: AXIOM-ADV-003
Mode: READ-ONLY SCOPE AND SPECIFICATION DRAFT; not implementation approval.
References: docs/07_CODEX_RULES.md sections 1-5 and 8, this workplan section 9
(AXIOM-ADV-003), and the actual approved project requirement sections found by inspection.
Dependencies to verify: IND-007, new product scope.
Allowed file writes: NONE. Forbidden writes: ALL files and git refs.
Expected behavior: inspect current sources/docs and prepare a response-only
implementation brief for this scope:
Specify quiescence, state transfer, capability lifetime, version migration, rollback, health checks and crash consistency. Kernel live update is excluded unless separately authorized.
Acceptance objective:
Faults at each update phase preserve stated invariants; availability metrics include supervisor/common-mode failures.
Identify exact requirement references, literal candidate allowed files, actual
test commands/targets, pre-fix regression design where relevant, documentation
files, state/error semantics, unsafe/dependency needs and rollback conditions.
Mark all unapproved choices DECISION REQUIRED; do not select them yourself.
If scope exceeds one reviewable change, produce separately bounded child briefs
with collision-checked AXIOM-AREA-NNN IDs. Do not execute them.
Tests required now: read-only source/history/command inspection only; no tests
or builds. Enumerate tests required later and missing prerequisites.
Documentation update now: NONE.
Definition of done: complete candidate brief(s), traced requirements, decision
register, checkable gate and explicit approval boundary.
Rollback: no writes expected; stop/report unexpected mutation without discarding
user work. No commit, push, checkout, installation or next-task execution.
```

### AXIOM-ADV-004 — Platform expansion and assurance program
```text
Task ID: AXIOM-ADV-004
Mode: READ-ONLY SCOPE AND SPECIFICATION DRAFT; not implementation approval.
References: docs/07_CODEX_RULES.md sections 1-5 and 8, this workplan section 9
(AXIOM-ADV-004), and the actual approved project requirement sections found by inspection.
Dependencies to verify: IND-007, independent assessor engagement.
Allowed file writes: NONE. Forbidden writes: ALL files and git refs.
Expected behavior: inspect current sources/docs and prepare a response-only
implementation brief for this scope:
Choose one additional platform per qualified demand. Define intended system use, applicable standards/editions, tool qualification needs and assessment responsibilities with experts. No autonomous commitments, certification applications or commercial promises.
Acceptance objective:
Platform-specific evidence and agreed assessment plan; certification/compliance status only from substantiated process outcomes.
Identify exact requirement references, literal candidate allowed files, actual
test commands/targets, pre-fix regression design where relevant, documentation
files, state/error semantics, unsafe/dependency needs and rollback conditions.
Mark all unapproved choices DECISION REQUIRED; do not select them yourself.
If scope exceeds one reviewable change, produce separately bounded child briefs
with collision-checked AXIOM-AREA-NNN IDs. Do not execute them.
Tests required now: read-only source/history/command inspection only; no tests
or builds. Enumerate tests required later and missing prerequisites.
Documentation update now: NONE.
Definition of done: complete candidate brief(s), traced requirements, decision
register, checkable gate and explicit approval boundary.
Rollback: no writes expected; stop/report unexpected mutation without discarding
user work. No commit, push, checkout, installation or next-task execution.
```

## 10. Approved implementation brief template
This is a NON-EXECUTABLE template until every field is filled and approved.

```text
Task ID: AXIOM-AREA-NNN
Status: APPROVED by [owner], [date], [approval reference]
Base: [branch] at [full SHA]
Dependencies: [accepted task IDs and commits]
Requirement references: [IDs + document paths + sections]
Approved ADR: [path + decision revision]
Objective: [one bounded observable change]
Allowed files: [exhaustive individual existing/new paths]
Forbidden files: all other files; protected paths/refs listed explicitly
Behavior: [input domains, validation order, state machine, errors,
           mutation/rollback atomicity, concurrency/interrupt assumptions,
           resource bounds, generation/overflow handling]
ABI: [unchanged or exact approved migration with docs/04 first]
Unsafe: [none or enumerated justification and invariant]
Dependencies: [none or explicitly approved exact pins]
Regression before fix: [input/sequence + live path + exact failure oracle]
Tests: [actual commands/configurations + expected results + deterministic seeds]
Evidence outputs: [exact allowed paths; no secrets or invented results]
Documentation updates: [exact paths or none with reason]
Definition of done: [observable, reproducible acceptance checks]
Rollback condition: [specific failure/regression/scope mismatch]
Rollback procedure: stop and preserve logs/patch; only owner-authorized revert
of the exact task commit after verifying a clean relevant worktree; no hard reset
Commit authority: [yes/no; exact message]
Push/merge/tag authority: NO
Next step: stop for reviewer acceptance
```

## 11. Review prompt after every implementation
```text
Review exactly the delivered AxiomRT task against its approved brief.
Read-only; allowed file writes NONE, forbidden writes ALL.
Check base and diff scope, requirement traceability, design fidelity, regression
ordering, live-versus-model evidence, exit statuses, dependencies, unsafe, ABI,
bounded resources, stale handles, failure atomicity and documentation truth.
Do not repair the patch or change assertions. Label findings by severity with
source evidence, and mark gate ACCEPTABLE FOR HUMAN REVIEW / REJECT / BLOCKED.
Do not claim this AI review is independent safety assessment.
Tests now: none unless separately authorized; inspect supplied artifacts and
call out missing reproduction. Documentation updates: none.
DoD: verdict with evidence gaps and exact unmet acceptance conditions.
Rollback: no writes; report accidental mutation and preserve user work.
```

## 12. Evidence and configuration record
Each accepted task binds:
- requirement / hazard or threat / design decision / source symbol;
- test ID, platform, tool versions, command, seed/corpus and exit status;
- exact source revision and working-tree diff hash when pre-commit;
- configuration, firmware, binary hash and board identity for on-target runs;
- raw-log hashes, normalized digest rules and reproducibility limits;
- pre-fix failing input and post-fix replay for defects;
- reviewer identity/scope, open issues and acceptance decision.

Avoid a circular final-commit hash embedded in files inside that same commit.
Use parent revision plus patch/content hashes for pre-commit evidence, then bind
final commit to evidence via a subsequent release manifest or CI attestation under
a separately allowed task. Hashes identify bytes; they do not prove authenticity.
Artifact signing needs explicit key-management design and separate authorization.

Tool qualification strategy begins with intended tool use and reliance on outputs,
not a blanket requirement to qualify everything. Record compiler/linker/prover/
test-generator versions and verification of their outputs. Exact standard edition,
assurance level, qualification approach and assessor responsibilities remain open
until the intended system and assessment basis are agreed.

## 13. References and limits
- Local governing rules inspected: docs/07_CODEX_RULES.md, AXIOM-DOC-008.
- FAA AC 20-115D recognizes DO-178C and supplements including DO-330 and DO-333;
  it is an acceptable means, not the sole means, of showing specified compliance.
  https://www.faa.gov/regulations_policies/advisory_circulars/index.cfm/go/document.information/documentID/1032046
- seL4 MCS documentation describes scheduling contexts with budget/period and CPU
  time authority; inspiration does not transfer seL4 proofs to AxiomRT.
  https://docs.sel4.systems/Tutorials/mcs.html
- These references were inspected 2026-09-08. No ARINC 653 licensed standard text
  was inspected for this plan. No ARINC API-conformance requirements are asserted.
- No customer requirement from Airbus/Thales has been supplied; proposed industrial
  capabilities are product hypotheses, not requirements attributed to those firms.

