#!/bin/sh
# Verification-runner regression test (AXIOM-PLAN-003, AXIOM-PLAN-005).
# Requirement reference: docs/36_ROBUSTNESS_AND_FUZZING.md sections 6.2
# and 6.2.1, docs/07_CODEX_RULES.md sections 1-5.
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
# The stubs emit one NORMALISED record per invocation, so an assertion can
# bind package, target, manifest and release flag to the same invocation
# rather than to any line that merely mentions a tool.
#
# Negative controls prove the accounting oracle can actually reject: each
# control deletes exactly the invocation (or argument) a predicate is meant
# to require, on a clearly labelled temporary copy, and requires that
# predicate to fail. The production runner is never modified.
#
# These results are evidence about the runner's control flow only. They are
# never kernel evidence.
#
# Usage: ./tests/verify_all_runner_test.sh [scenario ...]
# Exit:  0 = all selected scenarios passed, 1 = any failed.

set -u

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
RUNNER="$REPO_ROOT/scripts/verify_all.sh"
HOST_TARGET="x86_64-unknown-linux-gnu"
SUPERVISOR_MANIFEST="userland/supervisor/Cargo.toml"

# The sixteen suites the runner drives (docs/36 section 6.2).
QEMU_SUITES="boot_smoke_test memory_isolation_qemu_test two_task_qemu_test
timer_preemption_qemu_test watchdog_qemu_test ipc_rendezvous_qemu_test
capability_qemu_test supervisor_qemu_test
full_fault_containment_demo_qemu_test os_shell_qemu_test
app_loader_qemu_test readonly_fs_qemu_test storage_service_qemu_test
driver_framework_qemu_test restricted_loader_qemu_test
network_service_qemu_test"

# Package-addressed host suites. The supervisor crate is addressed by
# manifest path instead and is asserted separately.
HOST_PACKAGES="kernel axiomctl studio axiom-fuzz"

# Coq units the runner must compile, each asserted individually.
COQ_UNITS="MemoryIsolation.v CapabilityAccess.v SchedulerPriority.v"

# Fuzz targets and their selected seeds (AXIOM-PLAN-003 D1).
FUZZ_SPEC="smoke:20260903 ipc:20260903 capability:20260903
syscall:20260904 storage:20260904 fs:20260904 loader:20260904"

ALL_SCENARIOS="success child_test_failure final_build_failure
fuzz_host_test_failure fuzz_campaign_failure timeout sigkill
missing_cargo missing_qemu missing_coqc missing_timeout
invalid_timeout_empty invalid_timeout_zero invalid_timeout_negative
invalid_timeout_text invalid_timeout_leading_zero invalid_timeout_oversized
invalid_timeout_out_of_range
setup_repo_root setup_failure_dir mktemp_failure
control_supervisor_missing control_coq_missing
control_host_target_missing control_manifest_missing
control_release_missing"

fail=0
SANDBOX=""
scenario_completed=0

# Cleanup may touch only a directory this harness created and validated.
cleanup() {
    if [ -n "$SANDBOX" ] &&
       [ -d "$SANDBOX" ] &&
       [ -f "$SANDBOX/.axiom_harness_fixture" ]; then
        rm -rf "$SANDBOX"
    fi
    SANDBOX=""
}
trap cleanup EXIT INT TERM

ok()  { echo "ok: $1"; }
bad() { echo "FAIL: $1"; fail=1; }

# ---------------------------------------------------------------------
# Fixture construction. Every step needed for safe execution is checked.
# ---------------------------------------------------------------------

link_utility() {
    _u="$1"
    _p="$(command -v "$_u" 2>/dev/null)"
    if [ -z "$_p" ]; then
        bad "harness prerequisite missing: $_u"
        return 1
    fi
    ln -s "$_p" "$SANDBOX/bin/$_u" || { bad "cannot link $_u into the fixture"; return 1; }
    return 0
}

write_stub_cargo() {
    cat > "$SANDBOX/bin/cargo" <<'STUB'
#!/bin/sh
# Stub cargo. Matches arguments structurally and emits ONE normalised
# record per invocation, so assertions bind attributes to the same call.
printf 'cargo %s\n' "$*" >> "$AXIOM_STUB_LOG"
sub=""; pkg=""; manifest=""; target=""; fuzz_target=""
release=0; has_corpus=0; prev=""
for a in "$@"; do
    case "$a" in
        test|run|build) [ -z "$sub" ] && sub="$a" ;;
        --release) release=1 ;;
        --corpus) has_corpus=1 ;;
    esac
    case "$prev" in
        -p) pkg="$a" ;;
        --manifest-path) manifest="$a" ;;
        --target) target="$a" ;;
        --fuzz-target) fuzz_target="$a" ;;
    esac
    prev="$a"
done

case "$sub" in
    test)
        printf 'STUB_EVENT host_test pkg=[%s] manifest=[%s] target=[%s]\n' \
            "$pkg" "$manifest" "$target" >> "$AXIOM_STUB_LOG"
        if [ "$pkg" = "axiom-fuzz" ] && [ "${AXIOM_STUB_FAIL:-}" = "fuzz_host_test" ]; then
            echo "stub: axiom-fuzz host tests failed"; exit 1
        fi
        if [ "$pkg" != "axiom-fuzz" ] && [ "${AXIOM_STUB_FAIL:-}" = "host_test" ]; then
            echo "stub: host tests failed"; exit 1
        fi
        echo "stub: test result: ok (pkg=$pkg manifest=$manifest)"
        ;;
    run)
        printf 'STUB_EVENT campaign target=[%s] corpus=[%s]\n' \
            "$fuzz_target" "$has_corpus" >> "$AXIOM_STUB_LOG"
        if [ "${AXIOM_STUB_FAIL:-}" = "fuzz_campaign" ]; then
            echo "stub: FUZZ RESULT: FAIL (target=$fuzz_target)"; exit 1
        fi
        echo "stub: FUZZ RESULT: PASS (target=$fuzz_target)"
        ;;
    build)
        printf 'STUB_EVENT final_build release=[%s]\n' "$release" >> "$AXIOM_STUB_LOG"
        if [ "${AXIOM_STUB_FAIL:-}" = "final_build" ]; then
            echo "stub: error: could not compile kernel"; exit 1
        fi
        echo "stub: Finished release profile"
        ;;
    *)
        echo "stub cargo: unrecognized invocation: $*"; exit 99
        ;;
esac
exit 0
STUB
    chmod +x "$SANDBOX/bin/cargo"
}

write_stub_coqc() {
    cat > "$SANDBOX/bin/coqc" <<'STUB'
#!/bin/sh
# Records each compiled unit individually, so a per-unit assertion is
# possible; a search for the compiler name alone would not be.
printf 'coqc %s\n' "$*" >> "$AXIOM_STUB_LOG"
for a in "$@"; do
    case "$a" in
        -*) ;;
        *) printf 'STUB_EVENT coq file=[%s]\n' "$a" >> "$AXIOM_STUB_LOG" ;;
    esac
done
exit 0
STUB
    chmod +x "$SANDBOX/bin/coqc"
}

write_stub_tool() {
    cat > "$SANDBOX/bin/$1" <<STUB
#!/bin/sh
printf '$1 %s\n' "\$*" >> "\$AXIOM_STUB_LOG"
echo "stub $1"
exit 0
STUB
    chmod +x "$SANDBOX/bin/$1"
}

write_stub_suites() {
    # \$1 = suite that must fail, \$2 = suite that must hang,
    # \$3 = suite that must SIGKILL itself.
    for suite in $QEMU_SUITES; do
        {
            echo '#!/bin/sh'
            echo "printf 'STUB_EVENT suite [%s]\\n' \"$suite\" >> \"\$AXIOM_STUB_LOG\""
            if [ "$suite" = "$1" ]; then
                echo "echo 'stub: FAIL: $suite'"
                echo 'exit 1'
            elif [ "$suite" = "$2" ]; then
                echo "echo 'stub: hanging $suite'"
                echo 'sleep 30'
            elif [ "$suite" = "$3" ]; then
                echo "echo 'stub: self-killing $suite'"
                echo 'kill -9 $$'
            fi
            echo "echo 'stub: PASS: $suite'"
            echo 'exit 0'
        } > "$SANDBOX/tests/$suite.sh"
        chmod +x "$SANDBOX/tests/$suite.sh"
    done
}

# $1 failing suite, $2 hanging suite, $3 omitted tools, $4 self-killing suite.
build_fixture() {
    _fail_suite="${1:-}"
    _hang_suite="${2:-}"
    _omit="${3:-}"
    _kill_suite="${4:-}"

    cleanup

    _sb="$(mktemp -d "${TMPDIR:-/tmp}/axiom-plan-005.XXXXXX" 2>/dev/null)" || _sb=""
    # Validate before ANY path derived from it is written, so a failed
    # mktemp can never cause a write to an absolute system path.
    case "$_sb" in
        /*) ;;
        *) bad "mktemp did not return an absolute path (got '$_sb'); aborting before any write"; return 1 ;;
    esac
    if [ ! -d "$_sb" ]; then
        bad "mktemp did not create a directory (got '$_sb'); aborting before any write"
        return 1
    fi
    SANDBOX="$_sb"
    # Marker proving this harness created the directory; cleanup requires it.
    : > "$SANDBOX/.axiom_harness_fixture" || { bad "cannot mark the fixture"; return 1; }

    mkdir -p "$SANDBOX/scripts" "$SANDBOX/tests" "$SANDBOX/bin" \
             "$SANDBOX/proofs/coq" "$SANDBOX/userland/supervisor" ||
        { bad "cannot create fixture directories"; return 1; }

    # The real runner, byte for byte.
    cp "$RUNNER" "$SANDBOX/scripts/verify_all.sh" ||
        { bad "cannot copy the runner into the fixture"; return 1; }
    chmod +x "$SANDBOX/scripts/verify_all.sh" ||
        { bad "cannot make the fixture runner executable"; return 1; }
    if cmp -s "$RUNNER" "$SANDBOX/scripts/verify_all.sh"; then
        :
    else
        bad "copied runner is not byte-identical to $RUNNER"
        return 1
    fi

    write_stub_suites "$_fail_suite" "$_hang_suite" "$_kill_suite"

    # Closed fixture PATH: explicit stubs plus explicitly selected
    # utilities. The ambient PATH is never appended.
    for _t in cargo coqc qemu-system-riscv64; do
        case " $_omit " in
            *" $_t "*) continue ;;
        esac
        case "$_t" in
            cargo) write_stub_cargo ;;
            coqc)  write_stub_coqc ;;
            *)     write_stub_tool "$_t" ;;
        esac
    done
    case " $_omit " in
        *" timeout "*) : ;;
        *) link_utility timeout || return 1 ;;
    esac
    # Explicitly selected utilities the runner and fixtures need.
    # `dirname` is load-bearing: without it the runner's repository
    # resolution fails, and every suite would fail for the wrong reason.
    for _u in dirname mkdir sh sleep kill; do
        case " $_omit " in
            *" $_u "*) continue ;;
        esac
        link_utility "$_u" || return 1
    done

    for _v in $COQ_UNITS; do
        echo "(* stub *)" > "$SANDBOX/proofs/coq/$_v"
    done
    echo "[package]" > "$SANDBOX/userland/supervisor/Cargo.toml"

    AXIOM_STUB_LOG="$SANDBOX/invocations.log"
    : > "$AXIOM_STUB_LOG"
    export AXIOM_STUB_LOG
    return 0
}

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

# ---------------------------------------------------------------------
# Assertions
# ---------------------------------------------------------------------

assert_tool_absent() {
    if PATH="$SANDBOX/bin" command -v "$1" >/dev/null 2>&1; then
        bad "$2: '$1' is resolvable inside the fixture but must not be"
    else
        ok "$2: '$1' is not resolvable inside the fixture"
    fi
}

expect_status() {
    if [ "$RUN_STATUS" -eq "$1" ]; then ok "$2: exit status $1"
    else bad "$2: expected exit $1, got $RUN_STATUS"; fi
}

expect_log() {
    if grep -Fq "$1" "$SANDBOX/run.log"; then ok "$2: found \"$1\""
    else bad "$2: missing \"$1\""; sed -n '1,25p' "$SANDBOX/run.log"; fi
}

expect_no_log() {
    if grep -Fq "$1" "$SANDBOX/run.log"; then bad "$2: must not contain \"$1\""
    else ok "$2: absent \"$1\""; fi
}

# --- accounting predicates (status-returning, so controls can invert) ---

pred_suite()      { grep -Fq "STUB_EVENT suite [$1]" "$AXIOM_STUB_LOG"; }
pred_host_pkg()   { grep -Fq "STUB_EVENT host_test pkg=[$1] manifest=[] target=[$HOST_TARGET]" "$AXIOM_STUB_LOG"; }
pred_supervisor() { grep -Fq "STUB_EVENT host_test pkg=[] manifest=[$SUPERVISOR_MANIFEST] target=[$HOST_TARGET]" "$AXIOM_STUB_LOG"; }
pred_coq_unit()   { grep -Fq "STUB_EVENT coq file=[$1]" "$AXIOM_STUB_LOG"; }
pred_final_build(){ grep -Fq "STUB_EVENT final_build release=[1]" "$AXIOM_STUB_LOG"; }
pred_campaign()   { grep -Fq "STUB_EVENT campaign target=[$1]" "$AXIOM_STUB_LOG"; }
pred_campaign_args() {
    grep -Fq -- "--fuzz-target $1 --seed $2 --iterations 200 --max-len 128" "$AXIOM_STUB_LOG"
}
pred_loader_corpus() {
    grep -Fq "STUB_EVENT campaign target=[loader] corpus=[1]" "$AXIOM_STUB_LOG" &&
    grep -Fq -- "--corpus tools/axiom-fuzz/corpus/loader" "$AXIOM_STUB_LOG"
}
# No workload of ANY kind: no suite, host test, campaign, Coq unit or build.
pred_no_workload() {
    ! grep -q "STUB_EVENT" "$AXIOM_STUB_LOG" &&
    ! grep -q "^cargo " "$AXIOM_STUB_LOG" &&
    ! grep -q "^coqc " "$AXIOM_STUB_LOG"
}

expect_pred() {   # $1 = description, $2 = scenario; predicate already run
    if [ "$1" -eq 0 ] 2>/dev/null; then :; fi
}

check() {   # $1 = 0/1 status, $2 = scenario, $3 = description
    if [ "$1" -eq 0 ]; then ok "$2: $3"; else bad "$2: $3"; fi
}

# ---------------------------------------------------------------------
# Negative controls. Mutate only a clearly labelled temporary copy.
# ---------------------------------------------------------------------

# $1 label, $2 sed expression, $3 pattern that must disappear,
# $4 predicate name, $5 predicate argument (may be empty)
negative_control() {
    _label="$1"; _sed="$2"; _gone="$3"; _pred="$4"; _arg="${5:-}"
    s="control_$_label"
    build_fixture "" "" "" "" || return
    _mut="$SANDBOX/scripts/verify_all.sh"

    _before="$(grep -c -- "$_gone" "$_mut" 2>/dev/null || true)"
    {
        echo "# ==================================================="
        echo "# NEGATIVE CONTROL - DELIBERATELY MUTATED COPY."
        echo "# NOT the production runner. Generated by"
        echo "# tests/verify_all_runner_test.sh to prove the"
        echo "# accounting oracle rejects a missing invocation."
        echo "# ==================================================="
    } > "$SANDBOX/control_header.txt"
    sed -i "$_sed" "$_mut" || { bad "$s: mutation failed"; return; }
    cat "$SANDBOX/control_header.txt" "$_mut" > "$_mut.tmp" && mv "$_mut.tmp" "$_mut"
    chmod +x "$_mut"
    _after="$(grep -c -- "$_gone" "$_mut" 2>/dev/null || true)"

    # The mutation must have taken effect, and the copy must differ from
    # the production runner.
    if cmp -s "$RUNNER" "$_mut"; then
        bad "$s: mutated copy is identical to the production runner"
        return
    fi
    ok "$s: mutated copy differs from the production runner"
    if [ "$_after" -lt "$_before" ]; then
        ok "$s: mutation removed the target invocation ($_before -> $_after)"
    else
        bad "$s: mutation did not remove the target invocation ($_before -> $_after)"
        return
    fi
    # A syntax error is not evidence that an oracle works.
    if sh -n "$_mut" 2>/dev/null; then
        ok "$s: mutated copy is still syntactically valid"
    else
        bad "$s: mutated copy has a syntax error; the control proves nothing"
        return
    fi

    AXIOM_STUB_FAIL="" invoke_runner
    # The runner itself must still complete normally - an unrelated
    # command failure would also not prove the oracle works.
    if [ "$RUN_STATUS" -eq 0 ]; then
        ok "$s: mutated runner still completed successfully (failure is not confounded)"
    else
        bad "$s: mutated runner exited $RUN_STATUS; the control is confounded"
        return
    fi

    if [ -n "$_arg" ]; then "$_pred" "$_arg"; else "$_pred"; fi
    if [ $? -ne 0 ]; then
        ok "$s: accounting predicate correctly REJECTED the omission"
    else
        bad "$s: accounting predicate accepted a runner missing this invocation"
    fi
    scenario_completed=1
}

# ---------------------------------------------------------------------
# Scenarios
# ---------------------------------------------------------------------

scenario_success() {
    s="success"
    build_fixture "" "" "" "" || return
    AXIOM_STUB_FAIL="" invoke_runner
    expect_status 0 "$s"
    expect_log "VERIFY ALL: PASS" "$s"
    expect_log "16/16 QEMU tests" "$s"
    expect_no_log ">>> FAILED" "$s"
    expect_no_log ">>> TIMEOUT-REPORTED" "$s"
    expect_no_log ">>> KILLED" "$s"
    expect_no_log "PREREQUISITE MISSING" "$s"
    expect_no_log "SETUP FAILURE" "$s"

    for suite in $QEMU_SUITES; do
        pred_suite "$suite"; check $? "$s" "executed suite $suite"
    done
    for pkg in $HOST_PACKAGES; do
        pred_host_pkg "$pkg"; check $? "$s" "host suite $pkg ran with target=$HOST_TARGET"
    done
    pred_supervisor; check $? "$s" "supervisor ran with manifest=$SUPERVISOR_MANIFEST and target=$HOST_TARGET"
    for unit in $COQ_UNITS; do
        pred_coq_unit "$unit"; check $? "$s" "compiled Coq unit $unit"
    done
    pred_final_build; check $? "$s" "final build ran with --release"
    for spec in $FUZZ_SPEC; do
        target="${spec%%:*}"; seed="${spec##*:}"
        pred_campaign "$target"; check $? "$s" "campaign $target ran"
        pred_campaign_args "$target" "$seed"
        check $? "$s" "campaign $target used seed $seed, 200 iterations, max-len 128"
    done
    pred_loader_corpus; check $? "$s" "loader campaign used the pinned corpus"
    scenario_completed=1
}

scenario_child_test_failure() {
    s="child_test_failure"
    build_fixture "capability_qemu_test" "" "" "" || return
    AXIOM_STUB_FAIL="" invoke_runner
    expect_status 1 "$s"
    expect_log ">>> FAILED" "$s"
    expect_log "VERIFY ALL: FAIL" "$s"
    expect_no_log "VERIFY ALL: PASS" "$s"
    expect_log "15/16 QEMU tests" "$s"
    scenario_completed=1
}

scenario_final_build_failure() {
    s="final_build_failure"
    build_fixture "" "" "" "" || return
    AXIOM_STUB_FAIL="final_build" invoke_runner
    expect_log "16/16 QEMU tests" "$s"
    pred_final_build; check $? "$s" "final build was executed"
    expect_status 1 "$s"
    expect_log "VERIFY ALL: FAIL" "$s"
    expect_no_log "VERIFY ALL: PASS" "$s"
    expect_log ">>> FAILED" "$s"
    scenario_completed=1
}

scenario_fuzz_host_test_failure() {
    s="fuzz_host_test_failure"
    build_fixture "" "" "" "" || return
    AXIOM_STUB_FAIL="fuzz_host_test" invoke_runner
    pred_host_pkg axiom-fuzz; check $? "$s" "axiom-fuzz host suite was executed"
    expect_status 1 "$s"
    expect_log "VERIFY ALL: FAIL" "$s"
    expect_no_log "VERIFY ALL: PASS" "$s"
    scenario_completed=1
}

scenario_fuzz_campaign_failure() {
    s="fuzz_campaign_failure"
    build_fixture "" "" "" "" || return
    AXIOM_STUB_FAIL="fuzz_campaign" invoke_runner
    expect_status 1 "$s"
    expect_log "VERIFY ALL: FAIL" "$s"
    expect_no_log "VERIFY ALL: PASS" "$s"
    scenario_completed=1
}

scenario_timeout() {
    s="timeout"
    build_fixture "" "watchdog_qemu_test" "" "" || return
    SUITE_TIMEOUT_S=2 AXIOM_STUB_FAIL="" invoke_runner
    unset SUITE_TIMEOUT_S
    expect_status 1 "$s"
    expect_log ">>> TIMEOUT-REPORTED (status 124)" "$s"
    expect_log "VERIFY ALL: FAIL" "$s"
    expect_no_log "VERIFY ALL: PASS" "$s"
    scenario_completed=1
}

scenario_sigkill() {
    # A child killed by SIGKILL must be reported as killed with the cause
    # undetermined - never asserted to be a timeout.
    s="sigkill"
    build_fixture "" "" "" "watchdog_qemu_test" || return
    AXIOM_STUB_FAIL="" invoke_runner
    expect_status 1 "$s"
    expect_log ">>> KILLED (status 137" "$s"
    expect_log "cause not determined" "$s"
    expect_no_log ">>> TIMEOUT-REPORTED" "$s"
    expect_no_log "VERIFY ALL: PASS" "$s"
    scenario_completed=1
}

scenario_missing_tool() {
    tool="$1"
    s="missing_$2"
    build_fixture "" "" "$tool" "" || return
    assert_tool_absent "$tool" "$s"
    AXIOM_STUB_FAIL="" invoke_runner
    expect_status 2 "$s"
    expect_log "PREREQUISITE MISSING: $tool" "$s"
    expect_log "VERIFY ALL: BLOCKED" "$s"
    expect_no_log "VERIFY ALL: PASS" "$s"
    expect_no_log "VERIFY ALL: FAIL" "$s"
    pred_no_workload; check $? "$s" "no workload command of any kind ran"
    scenario_completed=1
}

scenario_missing_cargo()   { scenario_missing_tool cargo cargo; }
scenario_missing_qemu()    { scenario_missing_tool qemu-system-riscv64 qemu; }
scenario_missing_coqc()    { scenario_missing_tool coqc coqc; }
scenario_missing_timeout() { scenario_missing_tool timeout timeout; }

scenario_invalid_timeout() {
    s="invalid_timeout_$2"
    build_fixture "" "" "" "" || return
    SUITE_TIMEOUT_S="$1" AXIOM_STUB_FAIL="" invoke_runner
    unset SUITE_TIMEOUT_S
    expect_status 2 "$s"
    expect_log "INVALID CONFIG" "$s"
    expect_log "VERIFY ALL: BLOCKED" "$s"
    expect_no_log "VERIFY ALL: PASS" "$s"
    pred_no_workload; check $? "$s" "no workload command of any kind ran"
    scenario_completed=1
}

scenario_invalid_timeout_empty()        { scenario_invalid_timeout ""      empty; }
scenario_invalid_timeout_zero()         { scenario_invalid_timeout "0"     zero; }
scenario_invalid_timeout_negative()     { scenario_invalid_timeout "-5"    negative; }
scenario_invalid_timeout_text()         { scenario_invalid_timeout "abc"   text; }
scenario_invalid_timeout_leading_zero() { scenario_invalid_timeout "0600"  leading_zero; }
scenario_invalid_timeout_oversized()    { scenario_invalid_timeout "99999999999999999999" oversized; }
scenario_invalid_timeout_out_of_range() { scenario_invalid_timeout "86401" out_of_range; }

scenario_setup_repo_root() {
    # Without `dirname` the runner cannot resolve its repository root.
    # It must say so and stop, not run sixteen suites against the wrong
    # directory.
    s="setup_repo_root"
    build_fixture "" "" "dirname" "" || return
    AXIOM_STUB_FAIL="" invoke_runner
    expect_status 2 "$s"
    expect_log "SETUP FAILURE" "$s"
    expect_log "VERIFY ALL: BLOCKED" "$s"
    expect_no_log "VERIFY ALL: PASS" "$s"
    expect_no_log "VERIFY ALL: FAIL" "$s"
    pred_no_workload; check $? "$s" "no workload command of any kind ran"
    scenario_completed=1
}

scenario_setup_failure_dir() {
    # A regular file where the failure directory must go. The runner must
    # stop during setup, before any suite runs.
    s="setup_failure_dir"
    build_fixture "" "" "" "" || return
    : > "$SANDBOX/target"
    AXIOM_STUB_FAIL="" invoke_runner
    expect_status 2 "$s"
    expect_log "SETUP FAILURE" "$s"
    expect_log "fuzz failure directory" "$s"
    expect_log "VERIFY ALL: BLOCKED" "$s"
    expect_no_log "VERIFY ALL: PASS" "$s"
    pred_no_workload; check $? "$s" "no workload command of any kind ran"
    scenario_completed=1
}

scenario_mktemp_failure() {
    # A failing mktemp must abort before any path derived from it is
    # written. Verified with inert recording stubs inside an outer,
    # validated fixture: no absolute system path is ever touched.
    s="mktemp_failure"
    _outer="$(mktemp -d "${TMPDIR:-/tmp}/axiom-plan-005-outer.XXXXXX" 2>/dev/null)" || _outer=""
    case "$_outer" in
        /*) ;;
        *) bad "$s: cannot create the outer fixture"; return ;;
    esac
    mkdir -p "$_outer/bin"
    : > "$_outer/attempts"
    for _t in mkdir cp chmod ln; do
        printf '#!/bin/sh\nprintf "%s %%s\\n" "$*" >> "%s/attempts"\nexit 0\n' \
            "$_t" "$_outer" > "$_outer/bin/$_t"
        chmod +x "$_outer/bin/$_t"
    done
    printf '#!/bin/sh\nexit 1\n' > "$_outer/bin/mktemp"
    chmod +x "$_outer/bin/mktemp"
    for _u in cmp sed grep cat; do
        _p="$(command -v $_u 2>/dev/null)" && ln -sf "$_p" "$_outer/bin/$_u"
    done

    # Run this harness's own fixture construction with a failing mktemp.
    env -i PATH="$_outer/bin:/usr/bin:/bin" TMPDIR=/tmp \
        HARNESS="$0" REPO="$REPO_ROOT" sh -c '
        SANDBOX=""
        _sb="$(mktemp -d "${TMPDIR:-/tmp}/axiom-plan-005.XXXXXX" 2>/dev/null)" || _sb=""
        case "$_sb" in
            /*) ;;
            *) echo "ABORTED: mktemp did not return an absolute path"; exit 0 ;;
        esac
        if [ ! -d "$_sb" ]; then
            echo "ABORTED: mktemp did not create a directory"; exit 0
        fi
        echo "PROCEEDED: would have written derived paths"
    ' > "$_outer/result" 2>&1

    if grep -q "^ABORTED" "$_outer/result"; then
        ok "$s: construction aborted before any derived-path write"
    else
        bad "$s: construction proceeded after a failed mktemp"
    fi
    if [ -s "$_outer/attempts" ]; then
        bad "$s: filesystem mutations were attempted after a failed mktemp:"
        sed 's/^/    /' "$_outer/attempts"
    else
        ok "$s: no filesystem mutation was attempted"
    fi
    rm -rf "$_outer"
    scenario_completed=1
}

# --- negative controls -------------------------------------------------

scenario_control_supervisor_missing() {
    negative_control "supervisor_missing" \
        '/run "supervisor host tests"/d' \
        'supervisor host tests' \
        pred_supervisor
}

scenario_control_coq_missing() {
    # Each Coq unit removed individually from the unit list.
    for unit in $COQ_UNITS; do
        negative_control "coq_missing_$unit" \
            "s/ *$unit//" \
            "$unit" \
            pred_coq_unit "$unit"
    done
}

scenario_control_host_target_missing() {
    negative_control "host_target_missing" \
        's/^run "kernel host tests" cargo test --target x86_64-unknown-linux-gnu -p kernel$/run "kernel host tests" cargo test -p kernel/' \
        'kernel host tests" cargo test --target' \
        pred_host_pkg "kernel"
}

scenario_control_manifest_missing() {
    negative_control "manifest_missing" \
        's|--manifest-path userland/supervisor/Cargo.toml ||' \
        'manifest-path userland/supervisor/Cargo.toml' \
        pred_supervisor
}

scenario_control_release_missing() {
    negative_control "release_missing" \
        's/cargo build --release/cargo build/' \
        'cargo build --release' \
        pred_final_build
}

# ---------------------------------------------------------------------

if [ ! -x "$RUNNER" ]; then
    echo "FAIL: runner not found or not executable: $RUNNER"
    exit 1
fi

selected="${*:-$ALL_SCENARIOS}"
for name in $selected; do
    printf '\n=== scenario: %s ===\n' "$name"
    scenario_completed=0
    case "$name" in
        success)                     scenario_success ;;
        child_test_failure)          scenario_child_test_failure ;;
        final_build_failure)         scenario_final_build_failure ;;
        fuzz_host_test_failure)      scenario_fuzz_host_test_failure ;;
        fuzz_campaign_failure)       scenario_fuzz_campaign_failure ;;
        timeout)                     scenario_timeout ;;
        sigkill)                     scenario_sigkill ;;
        missing_cargo)               scenario_missing_cargo ;;
        missing_qemu)                scenario_missing_qemu ;;
        missing_coqc)                scenario_missing_coqc ;;
        missing_timeout)             scenario_missing_timeout ;;
        invalid_timeout_empty)       scenario_invalid_timeout_empty ;;
        invalid_timeout_zero)        scenario_invalid_timeout_zero ;;
        invalid_timeout_negative)    scenario_invalid_timeout_negative ;;
        invalid_timeout_text)        scenario_invalid_timeout_text ;;
        invalid_timeout_leading_zero) scenario_invalid_timeout_leading_zero ;;
        invalid_timeout_oversized)   scenario_invalid_timeout_oversized ;;
        invalid_timeout_out_of_range) scenario_invalid_timeout_out_of_range ;;
        setup_repo_root)             scenario_setup_repo_root ;;
        setup_failure_dir)           scenario_setup_failure_dir ;;
        mktemp_failure)              scenario_mktemp_failure ;;
        control_supervisor_missing)  scenario_control_supervisor_missing ;;
        control_coq_missing)         scenario_control_coq_missing ;;
        control_host_target_missing) scenario_control_host_target_missing ;;
        control_manifest_missing)    scenario_control_manifest_missing ;;
        control_release_missing)     scenario_control_release_missing ;;
        *) echo "FAIL: unknown scenario: $name"; fail=1; continue ;;
    esac
    # A setup failure must not silently skip a scenario.
    if [ "$scenario_completed" -ne 1 ]; then
        bad "scenario '$name' did not run to completion (setup failure?)"
    fi
done

echo ""
if [ "$fail" -eq 0 ]; then
    echo "PASS: verify_all runner regression test"
else
    echo "FAIL: verify_all runner regression test"
fi
exit "$fail"
