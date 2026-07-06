#!/bin/sh
# AxiomRT on-target supervisor + logger QEMU test (AXIOM-SUPRT-008).
# Requirement reference: docs/19_SUPERVISOR_ONTARGET.md,
# docs/14_TEST_STRATEGY.md.
#
# Builds the supervisor demo, boots it, and asserts that a contained
# fault is delivered to the supervisor over IPC, recorded by the logger,
# and acknowledged with a recovery policy, and that the kernel does not
# panic. Restores the default build afterwards.
#
# Usage: ./tests/supervisor_qemu_test.sh
# Exit: 0 = pass, 1 = fail.

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
KERNEL_ELF="$REPO_ROOT/target/riscv64gc-unknown-none-elf/release/kernel"
BOOT_TIMEOUT_S=15
cd "$REPO_ROOT"

log="$(mktemp /tmp/axiomrt_sup.XXXXXX.log)"
cargo build --release --features demo_supervisor >/dev/null 2>&1

set +e
timeout "$BOOT_TIMEOUT_S" qemu-system-riscv64 \
    -machine virt -smp 1 -m 128M -nographic -bios default \
    -kernel "$KERNEL_ELF" < /dev/null > "$log" 2>&1
qs=$?
set -e

cargo build --release >/dev/null 2>&1

if [ "$qs" -ne 124 ] && [ "$qs" -ne 0 ]; then
    echo "FAIL: QEMU exit $qs (log: $log)"
    exit 1
fi

fail=0
for pat in \
    "TASK_STARTED task=supervisor_task" \
    "TASK_STARTED task=logger_task" \
    "FAULT type=WatchdogTimeout task=faulty_task" \
    "IPC delivered fault_event to=supervisor_task" \
    "LOGGER event=TASK_FAULTED task=faulty_task" \
    "RECOVERY_APPLIED policy=Kill"; do
    if grep -q "$pat" "$log"; then
        echo "ok: $pat"
    else
        echo "FAIL: missing \"$pat\""
        fail=1
    fi
done

if grep -q "PANIC" "$log"; then
    echo "FAIL: kernel panicked (log: $log)"
    fail=1
else
    echo "ok: no PANIC"
fi

if [ "$fail" -ne 0 ]; then
    echo "FAIL: supervisor test (log: $log)"
    exit 1
fi
rm -f "$log"
echo "PASS: supervisor test"
