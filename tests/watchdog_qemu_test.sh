#!/bin/sh
# AxiomRT watchdog / CPU-exhaustion QEMU test (AXIOM-WDOG-008).
# Requirement reference: docs/16_WATCHDOG.md, docs/14_TEST_STRATEGY.md.
#
# Builds the watchdog demo, boots it, and asserts that an infinite
# compute loop that never checks in is detected as a watchdog timeout,
# contained (moved to Faulted), that the critical task then runs, and
# that the kernel does not panic. Restores the default build afterwards.
#
# Usage: ./tests/watchdog_qemu_test.sh
# Exit: 0 = pass, 1 = fail.

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
KERNEL_ELF="$REPO_ROOT/target/riscv64gc-unknown-none-elf/release/kernel"
BOOT_TIMEOUT_S=15
cd "$REPO_ROOT"

log="$(mktemp /tmp/axiomrt_wdog.XXXXXX.log)"
cargo build --release --features demo_watchdog >/dev/null 2>&1

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
    "TASK_STARTED task=faulty_task" \
    "TASK_STARTED task=critical_task" \
    "FAULT type=WatchdogTimeout task=faulty_task" \
    "CONTAIN scope=user reason=watchdog_timeout action=faulted kernel=alive" \
    "SCHED selected=critical_task"; do
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
    echo "ok: no PANIC (kernel stayed alive; CPU exhaustion contained)"
fi

if [ "$fail" -ne 0 ]; then
    echo "FAIL: watchdog test (log: $log)"
    exit 1
fi
rm -f "$log"
echo "PASS: watchdog test"
