# AxiomRT Build and Boot

Document ID: created by AXIOM-BOOT-001 (Phase 2)
Requirement reference: docs/02_KERNEL_BLUEPRINT.md §5 (target platform)

## Toolchain Requirements

* Rust (stable) with the bare-metal target:
  `rustup target add riscv64gc-unknown-none-elf`
* QEMU with RISC-V 64 system emulation: `qemu-system-riscv64`
  (QEMU ships OpenSBI firmware; `-bios default` uses it)

No external Rust dependencies are used (docs/07_CODEX_RULES.md §7).

## Crate Layout (AXIOM-BOOT-001)

```text
Cargo.toml            — workspace root; panic=abort profiles
.cargo/config.toml    — default target riscv64gc-unknown-none-elf
kernel/Cargo.toml     — kernel package (lib + bin), zero dependencies
kernel/src/lib.rs     — kernel library root (no_std)
kernel/src/main.rs    — kernel binary entry (no_std, no_main)
kernel/src/panic.rs   — panic handler: controlled halt
```

Properties enforced in this phase:

* `no_std` — no standard library, no heap, no allocator.
* No scheduler, no user tasks, no OS features (Phase 2 boundary).
* Panic handler performs a controlled halt (docs/06_FAULT_MODEL.md,
  KernelInvariantViolation → KernelPanic).

## Build (check stage, AXIOM-BOOT-001)

Run from the repository root:

```sh
cargo check
```

Expected state after AXIOM-BOOT-001: `cargo check` completes with no errors
for target `riscv64gc-unknown-none-elf`. Linking a bootable image requires
the boot entry and linker script (AXIOM-BOOT-002).
