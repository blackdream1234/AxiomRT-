#!/bin/sh
# Restricted storage-backed loader test (AXIOM-LOAD-014; covers the
# LOAD-010..013 behaviors: load/run hello, load/run counter, fault
# containment, invalid-image rejection).
# Requirement reference: docs/32_RESTRICTED_APP_IMAGE_FORMAT.md,
# docs/33_STORAGE_BACKED_FS.md, AxiomRT v1.6.md §5.

set -u

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

LOG="$(mktemp)"
trap 'rm -f "$LOG"' EXIT

echo "building os_boot kernel"
cargo build --release --features os_boot -p kernel || exit 1

echo "booting QEMU (scripted loader session)"
(
    sleep 4
    printf 'bin\r'
    sleep 1
    printf 'app load hello\r'
    sleep 1
    printf 'app state hello\r'
    sleep 1
    printf 'run loaded hello\r'
    sleep 2
    printf 'app unload hello\r'
    sleep 1
    printf 'app load counter\r'
    sleep 1
    printf 'run loaded counter\r'
    sleep 2
    printf 'app load invalid_bad_magic\r'
    sleep 1
    printf 'app load invalid_bad_checksum\r'
    sleep 1
    printf 'app load invalid_bad_cap\r'
    sleep 1
    printf 'app load fault_demo\r'
    sleep 1
    printf 'run loaded fault_demo\r'
    sleep 3
    printf 'run hello\r'
    sleep 2
    printf 'ls\r'
    sleep 1
    printf 'storage info\r'
    sleep 1
    printf 'drivers\r'
    sleep 1
    printf 'shutdown\r'
    sleep 3
) | timeout 100 qemu-system-riscv64 \
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

# 1. Boot reaches the shell.
expect "axiom> "

# 2. bin lists the restricted app images (docs/33 §3).
expect "hello.app counter.app fault_demo.app invalid_bad_magic.app invalid_bad_cap.app invalid_bad_checksum.app"

# 3./4. load + run hello (LOAD-010), storage-backed record + validation.
expect "APP_IMAGE loaded=hello source=/bin/hello.app"
expect "OK loaded hello"
expect "OK running hello"
expect "hello from app: hello"
expect "TASK_EXITED task=hello"

# 5./6. load + run counter (LOAD-011).
expect "OK loaded counter"
expect "OK running counter"
expect "APP counter progress=1"
expect "APP counter progress=3"
expect "APP counter done"

# 10./11./12. invalid images rejected (LOAD-013), no kernel fault.
expect "APP_IMAGE rejected=invalid_bad_magic reason=bad_image"
expect "ERR bad_image"
expect "APP_IMAGE rejected=invalid_bad_checksum reason=bad_checksum"
expect "ERR bad_checksum"
expect "APP_IMAGE rejected=invalid_bad_cap reason=denied_capability"
expect "ERR denied_capability"

# 7./8./9. load + run fault_demo is contained (LOAD-012); shell alive.
expect "OK loaded fault_demo"
expect "CAP_DENIED task=fault_demo"
expect "FAULT type=WatchdogTimeout task=fault_demo"
expect "CONTAIN scope=user reason=watchdog_timeout action=faulted kernel=alive"
expect "RECOVERY_APPLIED policy=Kill"

# 13./14./15./16. existing behavior unchanged after loader activity.
expect "hello from app: hello"       # legacy run hello
expect "OK etc apps docs bin"        # ls
expect "OK block_size=48 blocks=8 readonly=true"  # storage info
expect "driver name=block_driver_service state=running kind=block_skeleton"  # drivers

# 17. controlled shutdown.
expect "SHUTDOWN controlled=true by=shell_service"
if [ "$QEMU_EXIT" -eq 0 ]; then
    echo "ok: QEMU exited 0 (controlled poweroff, not timeout)"
else
    echo "MISSING: controlled poweroff (QEMU exit $QEMU_EXIT)"
    fail=1
fi
if grep -q "PANIC kernel=axiomrt" "$LOG"; then
    echo "MISSING: kernel must not fault during loader activity"
    fail=1
else
    echo "ok: no kernel panic"
fi

# The loader itself must never fault (a .rodata escape in the loader
# path would show here).
if grep -q "FAULT type=PageFault task=app_loader_service" "$LOG"; then
    echo "MISSING: app_loader must not fault"
    fail=1
else
    echo "ok: app_loader stayed alive"
fi

echo "restoring default build"
cargo build --release >/dev/null 2>&1

if [ "$fail" -eq 0 ]; then
    echo "PASS: restricted loader test"
else
    echo "FAIL: restricted loader test"
    sed -n '1,300p' "$LOG"
fi
exit "$fail"
