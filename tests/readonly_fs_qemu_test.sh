#!/bin/sh
# Read-only filesystem service test (AXIOM-FS-007).
# Requirement reference: docs/28_READONLY_FILESYSTEM_SERVICE.md, axiomFX.md.
#
# Boots the os_boot kernel and asserts: fs_service starts, ls / ls /etc
# / ls /apps answer from user-space, cat serves file contents, invalid
# and overlong paths fail safely with ERR, the shell stays alive after
# errors, and shutdown still powers off cleanly (QEMU exit 0).

set -u

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

LOG="$(mktemp)"
trap 'rm -f "$LOG"' EXIT

echo "building os_boot kernel"
cargo build --release --features os_boot -p kernel || exit 1

echo "booting QEMU (scripted fs session)"
(
    sleep 3
    printf 'ls\r'
    sleep 1
    printf 'ls /etc\r'
    sleep 1
    printf 'ls /apps\r'
    sleep 1
    printf 'cat /etc/version\r'
    sleep 1
    printf 'cat /apps/hello.manifest\r'
    sleep 1
    printf 'cat /definitely/not/a/file\r'
    sleep 1
    printf 'cat /an/extremely/long/path/that/exceeds/the/line/buffer/safely/xxxxxxxxxx\r'
    sleep 1
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

expect "SERVICE started=fs_service"
expect "axiom> "

# Listings served by user-space fs policy.
expect "OK etc apps docs"
expect "OK version limitations"
expect "OK hello.manifest counter.manifest fault_demo.manifest"

# File contents.
expect "OK AxiomRT v1.3-readonly-fs RISC-V 64 evaluation stage"
expect "OK hello: prio=2 caps=console restart=rerun"

# Invalid path fails safely; overlong path (console-truncated at 63
# bytes) also resolves to a safe ERR, never a crash.
expect "ERR not_found"

# Shell alive after filesystem errors.
expect "uptime ticks="

# Controlled shutdown.
expect "SHUTDOWN controlled=true by=shell_service"
if [ "$QEMU_EXIT" -eq 0 ]; then
    echo "ok: QEMU exited 0 (controlled poweroff, not timeout)"
else
    echo "MISSING: controlled poweroff (QEMU exit $QEMU_EXIT)"
    fail=1
fi

# Kernel stayed a bystander: no kernel fault lines.
if grep -q "PANIC kernel=axiomrt" "$LOG"; then
    echo "MISSING: kernel must not fault during fs errors"
    fail=1
else
    echo "ok: no kernel panic during malformed requests"
fi

echo "restoring default build"
cargo build --release >/dev/null 2>&1

if [ "$fail" -eq 0 ]; then
    echo "PASS: read-only filesystem test"
else
    echo "FAIL: read-only filesystem test"
    sed -n '1,220p' "$LOG"
fi
exit "$fail"
