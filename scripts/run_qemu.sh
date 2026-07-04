#!/bin/sh
# AxiomRT QEMU run script (AXIOM-BOOT-004).
# Requirement reference: docs/09_BUILD_AND_BOOT.md.
#
# Builds the release kernel and boots it on the QEMU virt machine with
# OpenSBI firmware. Exit QEMU with: Ctrl-A then x.
#
# Usage: ./scripts/run_qemu.sh            (interactive)
# Extra QEMU arguments are passed through: ./scripts/run_qemu.sh -serial ...

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
KERNEL_ELF="$REPO_ROOT/target/riscv64gc-unknown-none-elf/release/kernel"

cd "$REPO_ROOT"
cargo build --release

exec qemu-system-riscv64 \
    -machine virt \
    -smp 1 \
    -m 128M \
    -nographic \
    -bios default \
    -kernel "$KERNEL_ELF" \
    "$@"
