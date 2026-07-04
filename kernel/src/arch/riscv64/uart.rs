//! UART serial output for the QEMU virt machine (AXIOM-BOOT-003).
//!
//! Requirement reference: docs/09_BUILD_AND_BOOT.md (boot banner),
//! docs/05_MEMORY_MODEL.md §7 (device memory is kernel-only in v0.1).
//!
//! Device: NS16550A-compatible UART at physical address 0x1000_0000 on the
//! QEMU `virt` machine. v0.1 runs without address translation in the boot
//! phase, so the physical MMIO address is used directly. Output only —
//! no interrupts, no input, no buffering.

/// NS16550A base address on QEMU virt.
const UART_BASE: usize = 0x1000_0000;
/// Transmitter Holding Register (write).
const THR: usize = 0x0;
/// Line Status Register (read).
const LSR: usize = 0x5;
/// LSR bit 5: THR empty, ready to accept a byte.
const LSR_THRE: u8 = 1 << 5;

/// Write one byte to the UART, waiting until the transmitter is ready.
pub fn put_byte(byte: u8) {
    // SAFETY (docs/07_CODEX_RULES.md §6): MMIO requires volatile access to
    // a fixed hardware address. UART_BASE+LSR / UART_BASE+THR are the QEMU
    // virt NS16550A registers, a device region owned by the kernel
    // (docs/05_MEMORY_MODEL.md §7); no Rust object aliases this memory.
    // Reads/writes are single-byte and side-effect-only.
    unsafe {
        while core::ptr::read_volatile((UART_BASE + LSR) as *const u8) & LSR_THRE == 0 {
            core::hint::spin_loop();
        }
        core::ptr::write_volatile((UART_BASE + THR) as *mut u8, byte);
    }
}

/// Write a string to the UART. `\n` is expanded to `\r\n` for terminals.
pub fn put_str(s: &str) {
    for byte in s.bytes() {
        if byte == b'\n' {
            put_byte(b'\r');
        }
        put_byte(byte);
    }
}
