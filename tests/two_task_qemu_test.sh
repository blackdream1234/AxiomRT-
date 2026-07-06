#!/bin/sh
# AxiomRT two-task cooperative dispatch QEMU test (AXIOM-SCHEDRT-007).
# Requirement reference: docs/13_DISPATCH.md, docs/14_TEST_STRATEGY.md.
#
# Builds the multitask demo, boots it, and asserts that both tasks start,
# execution alternates between them (both SCHED selected lines appear),
# and both tasks exit and the demo completes. Restores the default build
# afterwards.
#
# Usage: ./tests/two_task_qemu_test.sh
# Exit: 0 = pass, 1 = fail.

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
KERNEL_ELF="$REPO_ROOT/target/riscv64gc-unknown-none-elf/release/kernel"
BOOT_TIMEOUT_S=15
cd "$REPO_ROOT"

log="$(mktemp /tmp/axiomrt_twotask.XXXXXX.log)"
cargo build --release --features demo_multitask >/dev/null 2>&1

set +e
timeout "$BOOT_TIMEOUT_S" qemu-system-riscv64 \
    -machine virt -smp 1 -m 128M -nographic -bios default \
    -kernel "$KERNEL_ELF" < /dev/null > "$log" 2>&1
qs=$?
set -e

# Restore default build for other tests.
cargo build --release >/dev/null 2>&1

if [ "$qs" -ne 124 ] && [ "$qs" -ne 0 ]; then
    echo "FAIL: QEMU exit $qs (log: $log)"
    exit 1
fi

fail=0
for pat in \
    "TASK_STARTED task=task_a" \
    "TASK_STARTED task=task_b" \
    "SCHED selected=task_b" \
    "SCHED selected=task_a" \
    "TASK_EXITED task=task_a" \
    "TASK_EXITED task=task_b" \
    "phase=multitask-demo-complete"; do
    if grep -q "$pat" "$log"; then
        echo "ok: $pat"
    else
        echo "FAIL: missing \"$pat\""
        fail=1
    fi
done

if [ "$fail" -ne 0 ]; then
    echo "FAIL: two-task dispatch test (log: $log)"
    exit 1
fi
rm -f "$log"
echo "PASS: two-task dispatch test"
