#!/bin/sh
# Application model + loader test (AXIOM-APP-008).
# Requirement reference: docs/27_APPLICATION_MODEL.md, AXIOMapp.md.
#
# Boots the os_boot kernel, drives the shell over stdin, and asserts:
#   - boot reaches axiom>,
#   - `apps` lists hello, fault_demo, counter (loader policy),
#   - `app info hello` answers from the manifest,
#   - `run hello` starts an isolated app that prints and exits cleanly,
#   - `run counter` emits progress events and exits,
#   - `run fault_demo` is denied + watchdog-contained + supervisor-killed
#     while the shell stays alive (uptime answers afterwards),
#   - `shutdown` performs a controlled poweroff (QEMU exit 0).
# Restores the default build afterwards.

set -u

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

LOG="$(mktemp)"
trap 'rm -f "$LOG"' EXIT

echo "building os_boot kernel"
cargo build --release --features os_boot -p kernel || exit 1

echo "booting QEMU (scripted app session)"
(
    sleep 3
    printf 'apps\r'
    sleep 1
    printf 'app info hello\r'
    sleep 1
    printf 'run hello\r'
    sleep 2
    printf 'run counter\r'
    sleep 2
    printf 'run fault_demo\r'
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

# Boot still reaches the shell (no regression of docs/25 §1).
expect "SERVICE started=app_loader_service"
expect "axiom> "

# apps / app info answered by loader policy.
expect "apps: hello fault_demo counter"
expect "hello: greeter prio=2 caps=console restart=rerun"

# run hello: isolated app prints and exits cleanly.
expect "SERVICE started=hello"
expect "hello from app: hello"
expect "TASK_EXITED task=hello"

# run counter: progress events, clean exit.
expect "APP counter progress=1"
expect "APP counter progress=3"
expect "APP counter done"
expect "TASK_EXITED task=counter"

# run fault_demo: denied (zero caps), contained, supervisor kills it.
expect "SERVICE started=fault_demo"
expect "CAP_DENIED task=fault_demo"
expect "FAULT type=WatchdogTimeout task=fault_demo"
expect "CONTAIN scope=user reason=watchdog_timeout action=faulted kernel=alive"
expect "RECOVERY_APPLIED policy=Kill"

# Shell alive after the app fault.
expect "uptime ticks="

# Controlled shutdown.
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
    echo "PASS: app loader test"
else
    echo "FAIL: app loader test"
    sed -n '1,220p' "$LOG"
fi
exit "$fail"
