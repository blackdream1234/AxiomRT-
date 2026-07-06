#!/bin/sh
# AxiomRT timer preemption QEMU test (AXIOM-TIMER-008).
# Requirement reference: docs/15_TIMER_PREEMPTION.md, docs/14_TEST_STRATEGY.md.
#
# Builds the preemption demo, boots it, and asserts that a low-priority
# infinite loop is preempted by the timer in favour of a high-priority
# task, that the critical task runs, and that the kernel does not panic
# (stays alive). Restores the default build afterwards.
#
# Usage: ./tests/timer_preemption_qemu_test.sh
# Exit: 0 = pass, 1 = fail.

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
KERNEL_ELF="$REPO_ROOT/target/riscv64gc-unknown-none-elf/release/kernel"
BOOT_TIMEOUT_S=15
cd "$REPO_ROOT"

log="$(mktemp /tmp/axiomrt_timer.XXXXXX.log)"
cargo build --release --features demo_preempt >/dev/null 2>&1

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
    "TASK_STARTED task=low_loop" \
    "TASK_STARTED task=critical_task" \
    "TIMER tick=1" \
    "SCHED preempt=low_loop selected=critical_task" \
    "SYSCALL name=sys_yield task=critical_task" \
    "TASK_EXITED task=critical_task"; do
    if grep -q "$pat" "$log"; then
        echo "ok: $pat"
    else
        echo "FAIL: missing \"$pat\""
        fail=1
    fi
done

# The kernel must not panic: an infinite loop cannot freeze it.
if grep -q "PANIC" "$log"; then
    echo "FAIL: kernel panicked (log: $log)"
    fail=1
else
    echo "ok: no PANIC (kernel stayed alive under the infinite loop)"
fi

if [ "$fail" -ne 0 ]; then
    echo "FAIL: timer preemption test (log: $log)"
    exit 1
fi
rm -f "$log"
echo "PASS: timer preemption test"
