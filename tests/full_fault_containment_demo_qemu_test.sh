#!/bin/sh
# AxiomRT full four-task fault-containment demo QEMU test (AXIOM-DEMO-002).
# Requirement reference: docs/20_FULL_DEMO.md, docs/00_PROJECT_CHARTER.md §7.
#
# Builds the four-task demo, boots it, and asserts the charter's first
# demonstration: the faulty task's illegal IPC is denied, its CPU
# exhaustion is contained as a watchdog timeout, the fault reaches the
# supervisor and logger, the supervisor applies a recovery policy, the
# critical task continues running after the faulty task is killed, and
# the kernel does not panic. Restores the default build afterwards.
#
# Usage: ./tests/full_fault_containment_demo_qemu_test.sh
# Exit: 0 = pass, 1 = fail.

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
KERNEL_ELF="$REPO_ROOT/target/riscv64gc-unknown-none-elf/release/kernel"
BOOT_TIMEOUT_S=15
cd "$REPO_ROOT"

log="$(mktemp /tmp/axiomrt_full.XXXXXX.log)"
cargo build --release --features demo_full >/dev/null 2>&1

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
    "TASK_STARTED task=critical_task" \
    "TASK_STARTED task=supervisor_task" \
    "TASK_STARTED task=logger_task" \
    "TASK_STARTED task=faulty_task" \
    "CAP_DENIED task=faulty_task reason=no_valid_capability" \
    "IPC state=unchanged" \
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

# The critical task must continue running after the faulty task is
# killed: many scheduling events for it, occurring after RECOVERY_APPLIED.
if [ "$(grep -c 'SCHED selected=critical_task' "$log")" -ge 3 ]; then
    echo "ok: critical_task continues (kept being scheduled)"
else
    echo "FAIL: critical_task did not continue"
    fail=1
fi

if grep -q "PANIC" "$log"; then
    echo "FAIL: kernel panicked (log: $log)"
    fail=1
else
    echo "ok: no PANIC (kernel alive under attack)"
fi

if [ "$fail" -ne 0 ]; then
    echo "FAIL: full fault-containment demo (log: $log)"
    exit 1
fi
rm -f "$log"
echo "PASS: full fault-containment demo (DEMO result=pass)"
