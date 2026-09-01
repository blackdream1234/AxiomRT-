#!/bin/sh
# AxiomRT full verification sweep (through v1.7).
# Runs every QEMU serial-assertion test, the host test suites, and the
# Coq model compilations, then restores the default build. Intended for
# evaluators and CI.
#
# Usage: ./scripts/verify_all.sh
# Exit: 0 = everything passed, 1 = any failure.

set -u

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

fail=0
qemu_pass=0
qemu_total=0
run_qemu() {
    qemu_total=$((qemu_total + 1))
    printf '\n=== %s ===\n' "$1"
    if "./tests/$1.sh"; then
        qemu_pass=$((qemu_pass + 1))
    else
        echo ">>> FAILED: ./tests/$1.sh"
        fail=1
    fi
}
run() {
    printf '\n=== %s ===\n' "$1"
    shift
    if "$@"; then :; else
        echo ">>> FAILED: $*"
        fail=1
    fi
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

# Coq model compilation.
run "coq models" sh -c 'cd proofs/coq && \
    coqc MemoryIsolation.v && \
    coqc CapabilityAccess.v && \
    coqc SchedulerPriority.v'

# Restore default build.
cargo build --release >/dev/null 2>&1

printf '\n=========================\n'
printf '\n%s/%s QEMU tests\n' "$qemu_pass" "$qemu_total"
if [ "$fail" -eq 0 ]; then
    echo "VERIFY ALL: PASS"
else
    echo "VERIFY ALL: FAIL"
fi
exit "$fail"
