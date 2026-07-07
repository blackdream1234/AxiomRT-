#!/bin/sh
# Real OS boot flow + interactive shell test (AXIOM-INIT/AXIOM-SHELL).
# Requirement reference: docs/25_OS_BOOT_FLOW.md §1/§6, docs/26_SHELL.md §4.
#
# Builds the os_boot kernel, boots QEMU, drives the shell over stdin
# (help, tasks, run demo, uptime, shutdown), and asserts:
#   - init_service starts and starts all four services,
#   - the axiom> prompt appears,
#   - shell commands answer,
#   - `run demo` produces the live containment + recovery chain,
#   - `shutdown` performs a controlled SBI poweroff (QEMU exits 0).
# Restores the default build afterwards.

set -u

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

LOG="$(mktemp)"
trap 'rm -f "$LOG"' EXIT

echo "building os_boot kernel"
cargo build --release --features os_boot -p kernel || exit 1

echo "booting QEMU (scripted shell session)"
(
    sleep 3
    printf 'help\r'
    sleep 1
    printf 'tasks\r'
    sleep 1
    printf 'run demo\r'
    sleep 4
    printf 'uptime\r'
    sleep 1
    printf 'shutdown\r'
    sleep 3
) | timeout 60 qemu-system-riscv64 \
    -machine virt -smp 1 -m 128M -nographic -bios default \
    -kernel target/riscv64gc-unknown-none-elf/release/kernel \
    >"$LOG" 2>&1
QEMU_EXIT=$?

fail=0
expect() {
    if grep -q "$1" "$LOG"; then
        echo "ok: found \"$1\""
    else
        echo "MISSING: \"$1\""
        fail=1
    fi
}

# Boot flow gate (docs/25 §1).
expect "AxiomRT kernel booted"
expect "MMU status=enabled mode=sv39 scope=kernel"
expect "TASK_STARTED task=init_service"
expect "SERVICE started=supervisor_service"
expect "SERVICE started=logger_service"
expect "SERVICE started=console_service"
expect "SERVICE started=shell_service"
expect "axiom> "

# Shell answers (docs/26 §4 gate).
expect "commands: help version tasks faults ipc caps memory uptime events"
expect "task idx=4 name=shell_service prio=3 state=running"
expect "uptime ticks="

# run demo: live containment + recovery on the console.
expect "SERVICE started=faulty_task"
expect "CAP_DENIED task=faulty_task reason=no_valid_capability"
expect "FAULT type=WatchdogTimeout task=faulty_task"
expect "CONTAIN scope=user reason=watchdog_timeout action=faulted kernel=alive"
expect "RECOVERY_APPLIED policy=Kill"

# Controlled shutdown: the kernel powers the machine off itself.
expect "SHUTDOWN controlled=true by=shell_service"
if [ "$QEMU_EXIT" -eq 0 ]; then
    echo "ok: QEMU exited 0 (controlled poweroff, not timeout)"
else
    echo "MISSING: controlled poweroff (QEMU exit $QEMU_EXIT)"
    fail=1
fi

echo "restoring default build"
cargo build --release >/dev/null 2>&1

if [ "$fail" -eq 0 ]; then
    echo "PASS: OS shell test"
else
    echo "FAIL: OS shell test"
    sed -n '1,200p' "$LOG"
fi
exit "$fail"
