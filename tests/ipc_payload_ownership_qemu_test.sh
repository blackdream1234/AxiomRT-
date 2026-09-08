#!/bin/sh
# Live IPC payload-ownership regression (AXIOM-FOUND-001).
# Requirement reference: docs/17_IPC_ONTARGET.md §2/§3/§5,
# docs/08_IPC_MODEL.md §3, docs/36_ROBUSTNESS_AND_FUZZING.md §5.3.
#
# Boots the demo_ipc_payload kernel and asserts that a message parked by
# a blocked sender belongs to its own endpoint. This drives the REAL
# dispatch path (ecall -> trap_handler -> dispatch::on_syscall); the
# host kernel::ipc model is a different implementation and proves
# nothing about it.
#
# Checked, each with exact length and exact bytes:
#   seq1  cross-endpoint: a send on another endpoint must not replace a
#         parked message  (IPC1RXP / IPC1RYP)
#   seq2  busy rejection: a send rejected as busy must not destroy the
#         parked message on that endpoint (IPC2BSY / IPC2RZP)
#   seq3  zero length accepted, oversized rejected (IPC3ZLP / IPC3OVP)
#   seq4  short receive rejected with the sender still parked, then an
#         adequately sized retry returns the original (IPC4SHP/IPC4RTP)
#   seq5  endpoint reuse after the parked sender is killed (IPC5RUP)
#
# Usage: ./tests/ipc_payload_ownership_qemu_test.sh
# Exit: 0 = pass, 1 = fail.

set -u

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
KERNEL_ELF="$REPO_ROOT/target/riscv64gc-unknown-none-elf/release/kernel"
BOOT_TIMEOUT_S=25
cd "$REPO_ROOT"

LOG="$(mktemp /tmp/axiomrt_ipcown.XXXXXX.log)"

echo "building demo_ipc_payload kernel"
cargo build --release --features demo_ipc_payload -p kernel >/dev/null 2>&1 || {
    echo "FAIL: kernel build failed"
    rm -f "$LOG"
    exit 1
}

echo "booting QEMU (IPC payload ownership)"
timeout "$BOOT_TIMEOUT_S" qemu-system-riscv64 \
    -machine virt -smp 1 -m 128M -nographic -bios default \
    -kernel "$KERNEL_ELF" < /dev/null > "$LOG" 2>&1
QEMU_EXIT=$?

echo "restoring default build"
cargo build --release >/dev/null 2>&1

fail=0
expect() {
    if grep -Fq "$1" "$LOG"; then
        echo "ok: found \"$1\""
    else
        echo "MISSING: \"$1\""
        fail=1
    fi
}
forbid() {
    if grep -Fq "$1" "$LOG"; then
        echo "PRESENT (must not be): \"$1\""
        fail=1
    else
        echo "ok: absent \"$1\""
    fi
}

# The run must complete; a build failure, an unrelated trap or a timeout
# is not a valid observation of this contract.
expect "IPCDONE"

# seq1 - cross-endpoint payload ownership.
expect "IPC1RXP"
expect "IPC1RYP"
forbid "IPC1RXF"
forbid "IPC1RYF"

# seq2 - a busy-rejected send must not disturb the parked payload.
expect "IPC2BSY"
expect "IPC2RZP"
forbid "IPC2BSF"
forbid "IPC2RZF"

# seq3 - zero length and oversized rejection.
expect "IPC3ZLP"
expect "IPC3OVP"
forbid "IPC3ZLF"
forbid "IPC3OVF"

# seq4 - short receive keeps the sender parked; retry returns the original.
expect "IPC4SHP"
expect "IPC4RTP"
forbid "IPC4SHF"
forbid "IPC4RTF"

# seq5 - endpoint reuse after the parked sender is killed.
expect "TASK_KILLED task=ipcown_sender_g"
expect "IPC5RUP"
forbid "IPC5RUF"

# The kernel must survive the whole run.
if grep -q "PANIC kernel=axiomrt" "$LOG"; then
    echo "PRESENT (must not be): kernel panic"
    fail=1
else
    echo "ok: no kernel panic"
fi

if [ "$QEMU_EXIT" -ne 124 ] && [ "$QEMU_EXIT" -ne 0 ]; then
    echo "MISSING: clean QEMU run (exit $QEMU_EXIT)"
    fail=1
fi

if [ "$fail" -eq 0 ]; then
    echo "PASS: IPC payload ownership test"
    rm -f "$LOG"
else
    echo "FAIL: IPC payload ownership test (log: $LOG)"
    sed -n '1,200p' "$LOG"
fi
exit "$fail"
