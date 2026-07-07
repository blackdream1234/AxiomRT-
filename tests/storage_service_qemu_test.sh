#!/bin/sh
# Storage service test (AXIOM-STOR-008).
# Requirement reference: docs/29_STORAGE_SERVICE.md, AxiomRTos.md §7.

set -u

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

LOG="$(mktemp)"
trap 'rm -f "$LOG"' EXIT

echo "building os_boot kernel"
cargo build --release --features os_boot -p kernel || exit 1

echo "booting QEMU (scripted storage session)"
(
    sleep 3
    printf 'storage info\r'
    sleep 1
    printf 'storage read 0\r'
    sleep 1
    printf 'storage read 99\r'
    sleep 1
    printf 'storage read abc\r'
    sleep 1
    printf 'cat /storage/version\r'
    sleep 1
    printf 'caps\r'
    sleep 1
    printf 'run hello\r'
    sleep 2
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

expect "SERVICE started=storage_service"
expect "axiom> "

# Protocol answers (docs/29 §4).
expect "OK block_size=48 blocks=8 readonly=true"
expect "OK data=AXSTOR v1 blocks=8 bs=48 ro=1"
expect "ERR bad_block"
expect "ERR malformed"

# shell -> fs -> storage -> fs -> shell chain (docs/29 §7).
expect "OK data=AxiomRT v1.4-storage-service evaluation stage"

# No pre-existing capability dropped by the table growth (docs/29 §5):
# the shell still holds endpoint(line) + console + info + control +
# endpoint(app) + endpoint(fs) + endpoint(storage).
expect "caps task=shell_service endpoint console info control endpoint endpoint endpoint"

# Apps still run after storage errors; controlled shutdown.
expect "hello from app: hello"
expect "SHUTDOWN controlled=true by=shell_service"
if [ "$QEMU_EXIT" -eq 0 ]; then
    echo "ok: QEMU exited 0 (controlled poweroff, not timeout)"
else
    echo "MISSING: controlled poweroff (QEMU exit $QEMU_EXIT)"
    fail=1
fi
if grep -q "PANIC kernel=axiomrt" "$LOG"; then
    echo "MISSING: kernel must not fault during storage errors"
    fail=1
else
    echo "ok: no kernel panic"
fi

echo "restoring default build"
cargo build --release >/dev/null 2>&1

if [ "$fail" -eq 0 ]; then
    echo "PASS: storage service test"
else
    echo "FAIL: storage service test"
    sed -n '1,220p' "$LOG"
fi
exit "$fail"
