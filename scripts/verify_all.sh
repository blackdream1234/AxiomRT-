#!/bin/sh
# AxiomRT full verification sweep (through v1.8).
# Runs every QEMU serial-assertion test, the host test suites, bounded
# deterministic fuzz smoke campaigns, and the Coq model compilations,
# then restores the default build. Intended for evaluators and CI.
#
# Requirement reference: docs/36_ROBUSTNESS_AND_FUZZING.md section 6.2
# (AXIOM-PLAN-003 runner integrity). The runner's own regression test is
# tests/verify_all_runner_test.sh, run separately.
#
# Usage: ./scripts/verify_all.sh
# Exit:  0 = everything passed
#        1 = an executed verification failed
#        2 = a prerequisite is missing or the configuration is invalid
#            (nothing was executed)

set -u

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

# ---------------------------------------------------------------------
# Configuration (AXIOM-PLAN-003)
# ---------------------------------------------------------------------

# Operational per-child timeout. This bounds scheduling, it is NOT a
# timing guarantee about AxiomRT (docs/36 section 9).
SUITE_TIMEOUT_S="${SUITE_TIMEOUT_S:-600}"
TIMEOUT_KILL_AFTER_S=10

# Bounded fuzz smoke parameters. Deep campaigns stay out of the default
# sweep and remain with AXIOM-ROBUST-016.
FUZZ_SMOKE_ITERATIONS=200
FUZZ_SMOKE_MAX_LEN=128
FUZZ_FAILURE_DIR="target/axiom-plan-003/fuzz-failures"
LOADER_CORPUS="tools/axiom-fuzz/corpus/loader"

fail=0
qemu_pass=0
qemu_total=0

# Reject an invalid timeout override before any suite runs.
case "$SUITE_TIMEOUT_S" in
    ''|*[!0-9]*)
        echo ">>> INVALID CONFIG: SUITE_TIMEOUT_S must be a positive integer (got '$SUITE_TIMEOUT_S')"
        echo ""
        echo "VERIFY ALL: BLOCKED"
        exit 2
        ;;
esac
if [ "$SUITE_TIMEOUT_S" -le 0 ]; then
    echo ">>> INVALID CONFIG: SUITE_TIMEOUT_S must be a positive integer (got '$SUITE_TIMEOUT_S')"
    echo ""
    echo "VERIFY ALL: BLOCKED"
    exit 2
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

# ---------------------------------------------------------------------
# Execution helpers. Every child runs under the operational timeout and
# every non-zero status reaches the aggregate result.
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
    if [ "$status" -eq 124 ] || [ "$status" -eq 137 ]; then
        echo ">>> TIMEOUT after ${SUITE_TIMEOUT_S}s (status $status): $*"
    else
        echo ">>> FAILED (status $status): $*"
    fi
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

# One bounded deterministic smoke campaign. Seeds and counts are this
# task's operational selection (docs/36 section 6.2), not a historical
# claim. Extra arguments are appended verbatim.
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

# Host test suites.
run "kernel host tests" cargo test --target x86_64-unknown-linux-gnu -p kernel
run "axiomctl host tests" cargo test --target x86_64-unknown-linux-gnu -p axiomctl
run "supervisor host tests" \
    cargo test --manifest-path userland/supervisor/Cargo.toml \
    --target x86_64-unknown-linux-gnu
run "studio host tests" cargo test --target x86_64-unknown-linux-gnu -p studio
run "axiom-fuzz host tests" cargo test --target x86_64-unknown-linux-gnu -p axiom-fuzz

# Bounded deterministic fuzz smoke campaigns (docs/36 section 6.2).
mkdir -p "$FUZZ_FAILURE_DIR"
run_fuzz smoke 20260903
run_fuzz ipc 20260903
run_fuzz capability 20260903
run_fuzz syscall 20260904
run_fuzz storage 20260904
run_fuzz fs 20260904
run_fuzz loader 20260904 --corpus "$LOADER_CORPUS"

# Coq model compilation.
run "coq models" sh -c 'cd proofs/coq && \
    coqc MemoryIsolation.v && \
    coqc CapabilityAccess.v && \
    coqc SchedulerPriority.v'

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
