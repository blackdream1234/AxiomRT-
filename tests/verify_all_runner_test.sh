#!/bin/sh
# Verification-runner regression test (AXIOM-PLAN-003).
# Requirement reference: docs/36_ROBUSTNESS_AND_FUZZING.md section 6.2,
# docs/07_CODEX_RULES.md sections 1-5.
#
# Tests scripts/verify_all.sh itself. A runner that cannot report failure
# makes every downstream claim unchecked, so the runner needs its own
# regression coverage.
#
# Method: copy the REAL runner into an isolated temporary root, assert the
# copy is byte-identical, and drive it with command stubs on a CLOSED
# fixture PATH (stubs plus a few explicitly selected utilities; the ambient
# PATH is never appended). No real cargo, QEMU or Coq process runs inside a
# scenario, and nothing outside the temporary root is read or written.
#
# The stubs record every invocation, so the success scenario asserts that
# each suite, host test, campaign, the Coq step and the final build were
# actually executed - an omitted step fails the harness.
#
# These results are evidence about the runner's control flow only. They are
# never kernel evidence.
#
# Usage: ./tests/verify_all_runner_test.sh [scenario ...]
# Exit:  0 = all selected scenarios passed, 1 = any failed.

set -u

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
RUNNER="$REPO_ROOT/scripts/verify_all.sh"

# The sixteen suites the runner drives (docs/36 section 6.2).
QEMU_SUITES="boot_smoke_test memory_isolation_qemu_test two_task_qemu_test
timer_preemption_qemu_test watchdog_qemu_test ipc_rendezvous_qemu_test
capability_qemu_test supervisor_qemu_test
full_fault_containment_demo_qemu_test os_shell_qemu_test
app_loader_qemu_test readonly_fs_qemu_test storage_service_qemu_test
driver_framework_qemu_test restricted_loader_qemu_test
network_service_qemu_test"

# Host test packages the runner must exercise, including axiom-fuzz.
HOST_PACKAGES="kernel axiomctl studio axiom-fuzz"

# Fuzz targets and their selected seeds (AXIOM-PLAN-003 D1).
FUZZ_SPEC="smoke:20260903 ipc:20260903 capability:20260903
syscall:20260904 storage:20260904 fs:20260904 loader:20260904"

ALL_SCENARIOS="success child_test_failure final_build_failure
fuzz_host_test_failure fuzz_campaign_failure timeout
missing_cargo missing_qemu missing_coqc missing_timeout
invalid_timeout_text invalid_timeout_zero"

fail=0
SANDBOX=""

cleanup() {
    # Remove only the exact directory this test created.
    if [ -n "$SANDBOX" ] && [ -d "$SANDBOX" ]; then
        rm -rf "$SANDBOX"
    fi
}
trap cleanup EXIT INT TERM

ok()   { echo "ok: $1"; }
bad()  { echo "FAIL: $1"; fail=1; }

# ---------------------------------------------------------------------
# Fixture construction
# ---------------------------------------------------------------------

# Resolve a utility to its absolute path and link it into the closed
# fixture PATH. Fails loudly rather than silently degrading.
link_utility() {
    _u="$1"
    _p="$(command -v "$_u" 2>/dev/null)"
    if [ -z "$_p" ]; then
        echo "FAIL: prerequisite for this harness is missing: $_u"
        exit 1
    fi
    ln -s "$_p" "$SANDBOX/bin/$_u"
}

write_stub_cargo() {
    cat > "$SANDBOX/bin/cargo" <<'STUB'
#!/bin/sh
# Stub cargo. Matches arguments structurally, not by exact string, so
# --quiet and argument order cannot silently break the match.
printf 'cargo %s\n' "$*" >> "$AXIOM_STUB_LOG"
sub=""
pkg=""
prev=""
fuzz_target=""
has_corpus=0
for a in "$@"; do
    case "$a" in
        test|run|build) [ -z "$sub" ] && sub="$a" ;;
    esac
    [ "$prev" = "-p" ] && pkg="$a"
    [ "$prev" = "--fuzz-target" ] && fuzz_target="$a"
    [ "$a" = "--corpus" ] && has_corpus=1
    prev="$a"
done

case "$sub" in
    test)
        printf 'STUB_EVENT host_test pkg=%s\n' "$pkg" >> "$AXIOM_STUB_LOG"
        if [ "$pkg" = "axiom-fuzz" ] && [ "${AXIOM_STUB_FAIL:-}" = "fuzz_host_test" ]; then
            echo "stub: axiom-fuzz host tests failed"
            exit 1
        fi
        if [ "$pkg" != "axiom-fuzz" ] && [ "${AXIOM_STUB_FAIL:-}" = "host_test" ]; then
            echo "stub: host tests failed"
            exit 1
        fi
        echo "stub: test result: ok (pkg=$pkg)"
        ;;
    run)
        printf 'STUB_EVENT campaign target=%s corpus=%s\n' \
            "$fuzz_target" "$has_corpus" >> "$AXIOM_STUB_LOG"
        if [ "${AXIOM_STUB_FAIL:-}" = "fuzz_campaign" ]; then
            echo "stub: FUZZ RESULT: FAIL (target=$fuzz_target)"
            exit 1
        fi
        echo "stub: FUZZ RESULT: PASS (target=$fuzz_target)"
        ;;
    build)
        printf 'STUB_EVENT final_build\n' >> "$AXIOM_STUB_LOG"
        if [ "${AXIOM_STUB_FAIL:-}" = "final_build" ]; then
            echo "stub: error: could not compile kernel"
            exit 1
        fi
        echo "stub: Finished release profile"
        ;;
    *)
        echo "stub cargo: unrecognized invocation: $*"
        exit 99
        ;;
esac
exit 0
STUB
    chmod +x "$SANDBOX/bin/cargo"
}

write_stub_tool() {
    # $1 = tool name, recorded but otherwise inert.
    cat > "$SANDBOX/bin/$1" <<STUB
#!/bin/sh
printf '$1 %s\n' "\$*" >> "\$AXIOM_STUB_LOG"
echo "stub $1"
exit 0
STUB
    chmod +x "$SANDBOX/bin/$1"
}

write_stub_suites() {
    # $1 = name of the suite that must fail, or empty.
    # $2 = name of the suite that must hang, or empty.
    for suite in $QEMU_SUITES; do
        {
            echo '#!/bin/sh'
            echo "printf 'suite %s\\n' \"$suite\" >> \"\$AXIOM_STUB_LOG\""
            echo "printf 'STUB_EVENT suite %s\\n' \"$suite\" >> \"\$AXIOM_STUB_LOG\""
            if [ "$suite" = "$1" ]; then
                echo "echo 'stub: FAIL: $suite'"
                echo 'exit 1'
            elif [ "$suite" = "$2" ]; then
                echo "echo 'stub: hanging $suite'"
                echo 'sleep 30'
            fi
            echo "echo 'stub: PASS: $suite'"
            echo 'exit 0'
        } > "$SANDBOX/tests/$suite.sh"
        chmod +x "$SANDBOX/tests/$suite.sh"
    done
}

# Build a fresh isolated root. $1 = failing suite, $2 = hanging suite,
# $3 = space-separated tools to OMIT from the fixture PATH.
build_fixture() {
    _fail_suite="${1:-}"
    _hang_suite="${2:-}"
    _omit="${3:-}"

    cleanup
    SANDBOX="$(mktemp -d "${TMPDIR:-/tmp}/axiom-plan-003.XXXXXX")"
    mkdir -p "$SANDBOX/scripts" "$SANDBOX/tests" "$SANDBOX/bin" \
             "$SANDBOX/proofs/coq" "$SANDBOX/userland/supervisor"

    # The real runner, byte for byte. If this ever drifts, the harness
    # must fail rather than silently test a stale copy.
    cp "$RUNNER" "$SANDBOX/scripts/verify_all.sh"
    chmod +x "$SANDBOX/scripts/verify_all.sh"
    if cmp -s "$RUNNER" "$SANDBOX/scripts/verify_all.sh"; then
        :
    else
        bad "copied runner is not byte-identical to $RUNNER"
        return 1
    fi

    write_stub_suites "$_fail_suite" "$_hang_suite"

    # Closed fixture PATH: explicit stubs plus explicitly selected
    # utilities. The ambient PATH is never appended.
    # NOTE: use a prefixed loop variable. A bare `tool` here would clobber
    # the caller's variable (shell globals), silently making a
    # missing-tool scenario assert against the wrong tool.
    for _t in cargo coqc qemu-system-riscv64; do
        case " $_omit " in
            *" $_t "*) continue ;;
        esac
        if [ "$_t" = "cargo" ]; then
            write_stub_cargo
        else
            write_stub_tool "$_t"
        fi
    done
    case " $_omit " in
        *" timeout "*) : ;;
        *) link_utility timeout ;;
    esac
    # Explicitly selected utilities the runner genuinely needs. `dirname`
    # is load-bearing: without it REPO_ROOT collapses to "/" and every
    # suite fails for the wrong reason, which would make a scenario pass
    # while proving nothing. `sh` runs the Coq step; `mkdir` creates the
    # fuzz failure directory.
    link_utility dirname
    link_utility mkdir
    link_utility sh
    # `sleep` backs the hanging-suite fixture; without it the stub would
    # fall through and the timeout scenario would silently prove nothing.
    link_utility sleep

    # Minimal Coq sources so the stub step has something to name.
    for v in MemoryIsolation CapabilityAccess SchedulerPriority; do
        echo "(* stub *)" > "$SANDBOX/proofs/coq/$v.v"
    done
    echo "[package]" > "$SANDBOX/userland/supervisor/Cargo.toml"

    AXIOM_STUB_LOG="$SANDBOX/invocations.log"
    : > "$AXIOM_STUB_LOG"
    export AXIOM_STUB_LOG
    return 0
}

# Run the copied runner under the closed PATH. Output -> $SANDBOX/run.log,
# status -> $RUN_STATUS.
invoke_runner() {
    env -i \
        PATH="$SANDBOX/bin" \
        HOME="$SANDBOX" \
        AXIOM_STUB_LOG="$AXIOM_STUB_LOG" \
        AXIOM_STUB_FAIL="${AXIOM_STUB_FAIL:-}" \
        ${SUITE_TIMEOUT_S+SUITE_TIMEOUT_S="$SUITE_TIMEOUT_S"} \
        "$SANDBOX/scripts/verify_all.sh" > "$SANDBOX/run.log" 2>&1
    RUN_STATUS=$?
}

# Assert a tool cannot be resolved inside the fixture PATH.
assert_tool_absent() {
    if PATH="$SANDBOX/bin" command -v "$1" >/dev/null 2>&1; then
        bad "$2: '$1' is resolvable inside the fixture but must not be"
    else
        ok "$2: '$1' is not resolvable inside the fixture"
    fi
}

expect_status() {
    if [ "$RUN_STATUS" -eq "$1" ]; then
        ok "$2: exit status $1"
    else
        bad "$2: expected exit $1, got $RUN_STATUS"
    fi
}

expect_log() {
    if grep -Fq "$1" "$SANDBOX/run.log"; then
        ok "$2: found \"$1\""
    else
        bad "$2: missing \"$1\""
        sed -n '1,40p' "$SANDBOX/run.log"
    fi
}

expect_no_log() {
    if grep -Fq "$1" "$SANDBOX/run.log"; then
        bad "$2: must not contain \"$1\""
    else
        ok "$2: absent \"$1\""
    fi
}

expect_event() {
    if grep -Fq "STUB_EVENT $1" "$AXIOM_STUB_LOG"; then
        ok "$2: executed [$1]"
    else
        bad "$2: never executed [$1]"
    fi
}

# ---------------------------------------------------------------------
# Scenarios
# ---------------------------------------------------------------------

scenario_success() {
    s="success"
    build_fixture "" "" "" || return
    AXIOM_STUB_FAIL="" invoke_runner
    expect_status 0 "$s"
    expect_log "VERIFY ALL: PASS" "$s"
    expect_log "16/16 QEMU tests" "$s"
    expect_no_log ">>> FAILED" "$s"
    expect_no_log ">>> TIMEOUT" "$s"
    expect_no_log "PREREQUISITE MISSING" "$s"

    # Invocation accounting: every documented step must really have run.
    for suite in $QEMU_SUITES; do
        expect_event "suite $suite" "$s"
    done
    for pkg in $HOST_PACKAGES; do
        expect_event "host_test pkg=$pkg" "$s"
    done
    for spec in $FUZZ_SPEC; do
        target="${spec%%:*}"
        seed="${spec##*:}"
        expect_event "campaign target=$target" "$s"
        # Exact parameters for this target's campaign.
        if grep -Fq -- "--fuzz-target $target --seed $seed --iterations 200 --max-len 128" \
                "$AXIOM_STUB_LOG"; then
            ok "$s: $target campaign used seed $seed, 200 iterations, max-len 128"
        else
            bad "$s: $target campaign parameters wrong or missing"
        fi
    done
    # The loader campaign, and only it, must carry the pinned corpus.
    if grep -Fq "STUB_EVENT campaign target=loader corpus=1" "$AXIOM_STUB_LOG"; then
        ok "$s: loader campaign used the pinned corpus"
    else
        bad "$s: loader campaign did not use --corpus"
    fi
    if grep -Fq -- "--corpus tools/axiom-fuzz/corpus/loader" "$AXIOM_STUB_LOG"; then
        ok "$s: loader corpus path is tools/axiom-fuzz/corpus/loader"
    else
        bad "$s: loader corpus path wrong"
    fi
    expect_event "final_build" "$s"
    if grep -Fq "coqc" "$AXIOM_STUB_LOG"; then
        ok "$s: executed [coq models]"
    else
        bad "$s: never executed [coq models]"
    fi
}

scenario_child_test_failure() {
    s="child_test_failure"
    build_fixture "capability_qemu_test" "" "" || return
    AXIOM_STUB_FAIL="" invoke_runner
    expect_status 1 "$s"
    expect_log ">>> FAILED" "$s"
    expect_log "VERIFY ALL: FAIL" "$s"
    expect_no_log "VERIFY ALL: PASS" "$s"
    expect_log "15/16 QEMU tests" "$s"
}

scenario_final_build_failure() {
    s="final_build_failure"
    build_fixture "" "" "" || return
    AXIOM_STUB_FAIL="final_build" invoke_runner
    # Guard against a false pass: the ONLY thing allowed to fail here is
    # the final build, so every suite must have succeeded first.
    expect_log "16/16 QEMU tests" "$s"
    expect_event "final_build" "$s"
    # The defect this task fixes: a failing restore build used to be run
    # with its status discarded, so the sweep still reported PASS.
    expect_status 1 "$s"
    expect_log "VERIFY ALL: FAIL" "$s"
    expect_no_log "VERIFY ALL: PASS" "$s"
    expect_log ">>> FAILED" "$s"
}

scenario_fuzz_host_test_failure() {
    s="fuzz_host_test_failure"
    build_fixture "" "" "" || return
    AXIOM_STUB_FAIL="fuzz_host_test" invoke_runner
    expect_event "host_test pkg=axiom-fuzz" "$s"
    expect_status 1 "$s"
    expect_log "VERIFY ALL: FAIL" "$s"
    expect_no_log "VERIFY ALL: PASS" "$s"
}

scenario_fuzz_campaign_failure() {
    s="fuzz_campaign_failure"
    build_fixture "" "" "" || return
    AXIOM_STUB_FAIL="fuzz_campaign" invoke_runner
    expect_status 1 "$s"
    expect_log "VERIFY ALL: FAIL" "$s"
    expect_no_log "VERIFY ALL: PASS" "$s"
}

scenario_timeout() {
    s="timeout"
    build_fixture "" "watchdog_qemu_test" "" || return
    SUITE_TIMEOUT_S=2 AXIOM_STUB_FAIL="" invoke_runner
    unset SUITE_TIMEOUT_S
    expect_status 1 "$s"
    expect_log ">>> TIMEOUT" "$s"
    expect_log "VERIFY ALL: FAIL" "$s"
    expect_no_log "VERIFY ALL: PASS" "$s"
}

# Each required prerequisite, absent.
scenario_missing_tool() {
    tool="$1"
    s="missing_$2"
    build_fixture "" "" "$tool" || return
    assert_tool_absent "$tool" "$s"
    AXIOM_STUB_FAIL="" invoke_runner
    expect_status 2 "$s"
    expect_log "PREREQUISITE MISSING: $tool" "$s"
    expect_log "VERIFY ALL: BLOCKED" "$s"
    expect_no_log "VERIFY ALL: PASS" "$s"
    expect_no_log "VERIFY ALL: FAIL" "$s"
    # Blocked means blocked: no suite may have run.
    if grep -Fq "STUB_EVENT suite " "$AXIOM_STUB_LOG"; then
        bad "$s: a suite ran despite a missing prerequisite"
    else
        ok "$s: no suite ran"
    fi
}

scenario_missing_cargo()   { scenario_missing_tool cargo cargo; }
scenario_missing_qemu()    { scenario_missing_tool qemu-system-riscv64 qemu; }
scenario_missing_coqc()    { scenario_missing_tool coqc coqc; }
scenario_missing_timeout() { scenario_missing_tool timeout timeout; }

scenario_invalid_timeout() {
    s="invalid_timeout_$2"
    build_fixture "" "" "" || return
    SUITE_TIMEOUT_S="$1" AXIOM_STUB_FAIL="" invoke_runner
    unset SUITE_TIMEOUT_S
    expect_status 2 "$s"
    expect_log "INVALID CONFIG" "$s"
    expect_no_log "VERIFY ALL: PASS" "$s"
    if grep -Fq "STUB_EVENT suite " "$AXIOM_STUB_LOG"; then
        bad "$s: a suite ran despite invalid configuration"
    else
        ok "$s: no suite ran"
    fi
}

scenario_invalid_timeout_text() { scenario_invalid_timeout "abc" text; }
scenario_invalid_timeout_zero() { scenario_invalid_timeout "0" zero; }

# ---------------------------------------------------------------------

if [ ! -x "$RUNNER" ]; then
    echo "FAIL: runner not found or not executable: $RUNNER"
    exit 1
fi

selected="${*:-$ALL_SCENARIOS}"
for name in $selected; do
    printf '\n=== scenario: %s ===\n' "$name"
    case "$name" in
        success)                scenario_success ;;
        child_test_failure)     scenario_child_test_failure ;;
        final_build_failure)    scenario_final_build_failure ;;
        fuzz_host_test_failure) scenario_fuzz_host_test_failure ;;
        fuzz_campaign_failure)  scenario_fuzz_campaign_failure ;;
        timeout)                scenario_timeout ;;
        missing_cargo)          scenario_missing_cargo ;;
        missing_qemu)           scenario_missing_qemu ;;
        missing_coqc)           scenario_missing_coqc ;;
        missing_timeout)        scenario_missing_timeout ;;
        invalid_timeout_text)   scenario_invalid_timeout_text ;;
        invalid_timeout_zero)   scenario_invalid_timeout_zero ;;
        *) echo "FAIL: unknown scenario: $name"; fail=1 ;;
    esac
done

echo ""
if [ "$fail" -eq 0 ]; then
    echo "PASS: verify_all runner regression test"
else
    echo "FAIL: verify_all runner regression test"
fi
exit "$fail"
