#!/bin/sh
# AxiomRT full verification sweep (through v1.8).
# Runs every QEMU serial-assertion test, the host test suites, bounded
# deterministic fuzz smoke campaigns, and the Coq model compilations,
# then restores the default build. Intended for evaluators and CI.
#
# Requirement reference: docs/36_ROBUSTNESS_AND_FUZZING.md sections 6.2
# and 6.2.1 (AXIOM-PLAN-003 runner integrity, AXIOM-PLAN-005 acceptance
# corrections). The runner's own regression test is
# tests/verify_all_runner_test.sh, run separately.
#
# Usage: ./scripts/verify_all.sh
# Exit:  0 = everything passed
#        1 = an executed verification failed
#        2 = setup, configuration or prerequisite rejection
#            (nothing was executed)

set -u

# ---------------------------------------------------------------------
# Setup. Every step that later work depends on is checked, so a broken
# environment is reported as such instead of being misattributed to a
# suite (AXIOM-PLAN-005). Nothing below runs any workload.
# ---------------------------------------------------------------------

setup_failure() {
    echo ">>> SETUP FAILURE: $1"
    echo ""
    echo "VERIFY ALL: BLOCKED"
    exit 2
}

config_failure() {
    echo ">>> INVALID CONFIG: $1"
    echo ""
    echo "VERIFY ALL: BLOCKED"
    exit 2
}

SCRIPT_DIR="$(dirname "$0" 2>/dev/null)" ||
    setup_failure "cannot determine the script directory from '$0'"
[ -n "$SCRIPT_DIR" ] ||
    setup_failure "the script directory resolved empty from '$0' (is 'dirname' available?)"

REPO_ROOT="$(cd "$SCRIPT_DIR/.." 2>/dev/null && pwd)" ||
    setup_failure "cannot resolve the repository root from '$SCRIPT_DIR/..'"
[ -n "$REPO_ROOT" ] ||
    setup_failure "the repository root resolved empty from '$SCRIPT_DIR/..'"

cd "$REPO_ROOT" 2>/dev/null ||
    setup_failure "cannot change directory to '$REPO_ROOT'"

# Confirm this really is the repository: a silently wrong root must not
# be allowed to proceed and report sixteen missing suites.
[ -f scripts/verify_all.sh ] && [ -d tests ] ||
    setup_failure "'$REPO_ROOT' does not look like the AxiomRT repository (scripts/verify_all.sh and tests/ expected)"

# ---------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------

# Operational per-child timeout. This bounds scheduling, it is NOT a
# timing guarantee about AxiomRT (docs/36 section 9).
SUITE_TIMEOUT_MIN=1
SUITE_TIMEOUT_MAX=86400
SUITE_TIMEOUT_S="${SUITE_TIMEOUT_S-600}"
TIMEOUT_KILL_AFTER_S=10

# Bounded fuzz smoke parameters. Deep campaigns stay out of the default
# sweep and remain with AXIOM-ROBUST-016.
FUZZ_SMOKE_ITERATIONS=200
FUZZ_SMOKE_MAX_LEN=128
FUZZ_FAILURE_DIR="target/axiom-plan-003/fuzz-failures"
LOADER_CORPUS="tools/axiom-fuzz/corpus/loader"
COQ_UNITS="MemoryIsolation.v CapabilityAccess.v SchedulerPriority.v"

# Timeout grammar: a canonical decimal integer in 1..86400. Grammar and
# length are checked before any numeric comparison, so an oversized
# value can never reach shell arithmetic.
case "$SUITE_TIMEOUT_S" in
    '')
        config_failure "SUITE_TIMEOUT_S must not be empty (expected an integer ${SUITE_TIMEOUT_MIN}..${SUITE_TIMEOUT_MAX}, or unset for 600)"
        ;;
    *[!0-9]*)
        config_failure "SUITE_TIMEOUT_S must contain digits only (got '$SUITE_TIMEOUT_S'; expected ${SUITE_TIMEOUT_MIN}..${SUITE_TIMEOUT_MAX})"
        ;;
esac
if [ "${#SUITE_TIMEOUT_S}" -gt 5 ]; then
    config_failure "SUITE_TIMEOUT_S is too long (got ${#SUITE_TIMEOUT_S} digits; expected ${SUITE_TIMEOUT_MIN}..${SUITE_TIMEOUT_MAX})"
fi
case "$SUITE_TIMEOUT_S" in
    0*)
        config_failure "SUITE_TIMEOUT_S must be written without leading zeros (got '$SUITE_TIMEOUT_S'; expected ${SUITE_TIMEOUT_MIN}..${SUITE_TIMEOUT_MAX})"
        ;;
esac
if [ "$SUITE_TIMEOUT_S" -lt "$SUITE_TIMEOUT_MIN" ] ||
   [ "$SUITE_TIMEOUT_S" -gt "$SUITE_TIMEOUT_MAX" ]; then
    config_failure "SUITE_TIMEOUT_S is out of range (got '$SUITE_TIMEOUT_S'; expected ${SUITE_TIMEOUT_MIN}..${SUITE_TIMEOUT_MAX})"
fi

# ---------------------------------------------------------------------
# Prerequisites: a missing tool is BLOCKED, never PASS and never a
# silent skip. Checked before anything is executed.
# ---------------------------------------------------------------------

missing=0
require_tool() {
    if command -v "$1" >/dev/null 2>&1; then
        :
    else
        echo ">>> PREREQUISITE MISSING: $1 ($2)"
        missing=1
    fi
}
require_tool cargo "builds the kernel and runs host tests, fuzz campaigns and the default build"
require_tool qemu-system-riscv64 "runs every QEMU serial-assertion suite"
require_tool coqc "compiles the Coq models"
require_tool timeout "bounds each child command"
if [ "$missing" -ne 0 ]; then
    echo ""
    echo "VERIFY ALL: BLOCKED"
    exit 2
fi

# The fuzz failure directory is part of setup: create it before any
# workload so a failure here cannot surface after a full suite run.
mkdir -p "$FUZZ_FAILURE_DIR" 2>/dev/null ||
    setup_failure "cannot create the fuzz failure directory '$FUZZ_FAILURE_DIR'"
[ -d "$FUZZ_FAILURE_DIR" ] ||
    setup_failure "the fuzz failure directory '$FUZZ_FAILURE_DIR' does not exist after creation"

fail=0
qemu_pass=0
qemu_total=0

# ---------------------------------------------------------------------
# Execution helpers. Every child runs under the operational timeout and
# every non-zero outcome reaches the aggregate result. Reported status
# is kept distinct from inferred cause (AXIOM-PLAN-005).
# ---------------------------------------------------------------------

run_cmd() {
    label="$1"
    shift
    printf '\n=== %s ===\n' "$label"
    timeout --kill-after="${TIMEOUT_KILL_AFTER_S}s" "${SUITE_TIMEOUT_S}s" "$@"
    status=$?
    if [ "$status" -eq 0 ]; then
        return 0
    fi
    case "$status" in
        124)
            # 124 is the timeout utility's own indication that the limit
            # was reached. It is not an independently established root
            # cause for why the command was slow.
            echo ">>> TIMEOUT-REPORTED (status 124): the timeout utility reported that the command exceeded ${SUITE_TIMEOUT_S}s: $*"
            ;;
        137)
            # 128+SIGKILL. This can come from the kill-after grace, the
            # out-of-memory killer, or an external signal; the number
            # alone does not distinguish them.
            echo ">>> KILLED (status 137, SIGKILL; cause not determined - may be the kill-after grace, the OOM killer or an external signal): $*"
            ;;
        *)
            echo ">>> FAILED (status $status): $*"
            ;;
    esac
    fail=1
    return 1
}

run_qemu() {
    qemu_total=$((qemu_total + 1))
    if run_cmd "$1" "./tests/$1.sh"; then
        qemu_pass=$((qemu_pass + 1))
    fi
}

run() {
    label="$1"
    shift
    run_cmd "$label" "$@"
}

# One bounded deterministic smoke campaign. Seeds and counts are an
# operational selection (docs/36 section 6.2), not a historical claim.
run_fuzz() {
    fuzz_target="$1"
    fuzz_seed="$2"
    shift 2
    run_cmd "fuzz smoke: $fuzz_target" cargo run --quiet -p axiom-fuzz \
        --target x86_64-unknown-linux-gnu -- \
        --fuzz-target "$fuzz_target" --seed "$fuzz_seed" \
        --iterations "$FUZZ_SMOKE_ITERATIONS" --max-len "$FUZZ_SMOKE_MAX_LEN" \
        --failure-dir "$FUZZ_FAILURE_DIR" "$@"
}

# QEMU serial-assertion tests.
for t in boot_smoke_test \
         memory_isolation_qemu_test \
         two_task_qemu_test \
         timer_preemption_qemu_test \
         watchdog_qemu_test \
         ipc_rendezvous_qemu_test \
         capability_qemu_test \
         supervisor_qemu_test \
         full_fault_containment_demo_qemu_test \
         os_shell_qemu_test \
         app_loader_qemu_test \
         readonly_fs_qemu_test \
         storage_service_qemu_test \
         driver_framework_qemu_test \
         restricted_loader_qemu_test \
         network_service_qemu_test; do
    run_qemu "$t"
done

# Host test suites. The supervisor crate is addressed by manifest path,
# not by package name, so it is kept on one line for exact accounting.
run "kernel host tests" cargo test --target x86_64-unknown-linux-gnu -p kernel
run "axiomctl host tests" cargo test --target x86_64-unknown-linux-gnu -p axiomctl
run "supervisor host tests" cargo test --manifest-path userland/supervisor/Cargo.toml --target x86_64-unknown-linux-gnu
run "studio host tests" cargo test --target x86_64-unknown-linux-gnu -p studio
run "axiom-fuzz host tests" cargo test --target x86_64-unknown-linux-gnu -p axiom-fuzz

# Bounded deterministic fuzz smoke campaigns (docs/36 section 6.2).
run_fuzz smoke 20260903
run_fuzz ipc 20260903
run_fuzz capability 20260903
run_fuzz syscall 20260904
run_fuzz storage 20260904
run_fuzz fs 20260904
run_fuzz loader 20260904 --corpus "$LOADER_CORPUS"

# Coq model compilation. Each unit is compiled in turn; the first
# failure stops the step.
run "coq models" sh -c 'cd proofs/coq && for unit in '"$COQ_UNITS"'; do coqc "$unit" || exit 1; done'

# Restore default build. Its status is part of the aggregate result and
# its diagnostics stay visible (AXIOM-PLAN-003): a broken default build
# must never coexist with VERIFY ALL: PASS.
run "restore default build" cargo build --release

printf '\n=========================\n'
printf '\n%s/%s QEMU tests\n' "$qemu_pass" "$qemu_total"
if [ "$fail" -eq 0 ]; then
    echo "VERIFY ALL: PASS"
else
    echo "VERIFY ALL: FAIL"
fi
exit "$fail"
