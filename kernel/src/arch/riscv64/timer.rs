//! Supervisor timer and preemption tick (AXIOM-TIMER-002..007).
//!
//! Requirement reference: docs/15_TIMER_PREEMPTION.md.
//!
//! Programs the RISC-V S-mode timer via the SBI TIME extension and
//! turns each tick into a preemption point. Always compiled on target
//! (the trap layer routes timer interrupts here) but dormant until
//! `init` enables `sie.STIE` and `arm_next` schedules the first tick —
//! the default build never enables it. riscv64-only.

use core::sync::atomic::{AtomicU64, Ordering};

use crate::dispatch;
use crate::trap::TrapFrame;
use crate::uart;

/// Cycles between ticks (~10 ms at the QEMU virt 10 MHz time base).
const TIMER_INTERVAL: u64 = 100_000;

/// SBI TIME extension (EID "TIME"), function 0 = set_timer.
const SBI_EXT_TIME: u64 = 0x5449_4D45;
const SBI_FID_SET_TIMER: u64 = 0;

/// Monotonic tick counter (docs/15 §5).
static TICKS: AtomicU64 = AtomicU64::new(0);

/// Read the `time` CSR (cycle-accurate wall time on QEMU virt).
fn read_time() -> u64 {
    let t: u64;
    // SAFETY: reading the unprivileged `time` CSR is side-effect free.
    unsafe { core::arch::asm!("rdtime {t}", t = out(reg) t) };
    t
}

/// Program the next S-mode timer interrupt at absolute time `t`.
fn set_timer(t: u64) {
    // SAFETY: SBI ecall to OpenSBI (M-mode) with the TIME extension set_
    // timer arguments (a7=EID, a6=FID, a0=stime). OpenSBI programs the
    // timer and returns; only a0/a1 are clobbered by the SBI ABI.
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") SBI_EXT_TIME,
            in("a6") SBI_FID_SET_TIMER,
            in("a0") t,
            lateout("a0") _,
            lateout("a1") _,
        );
    }
}

/// Enable supervisor timer interrupts (AXIOM-TIMER-003). Sets sie.STIE.
/// Called only by the preemption demo (feature demo_preempt); the
/// default build never enables the timer.
#[allow(dead_code)]
pub fn init() {
    const SIE_STIE: u64 = 1 << 5;
    // SAFETY: setting sie.STIE is a privileged CSR write enabling timer
    // interrupt delivery (docs/15 §3); no memory is affected.
    unsafe {
        core::arch::asm!("csrs sie, {b}", b = in(reg) SIE_STIE);
    }
}

/// Arm the next timer tick relative to now (AXIOM-TIMER-002).
pub fn arm_next() {
    set_timer(read_time().wrapping_add(TIMER_INTERVAL));
}

/// Handle a supervisor timer interrupt: count the tick, re-arm, and
/// offer a preemption point (AXIOM-TIMER-004/005/006/007).
pub fn on_timer_interrupt(frame: &mut TrapFrame) {
    let n = TICKS.fetch_add(1, Ordering::SeqCst) + 1;
    if n <= 3 {
        uart::put_str("TIMER tick=");
        put_dec(n);
        uart::put_str("\n");
    }
    arm_next();
    // Watchdog runs first (AXIOM-WDOG-004): a stuck task is contained
    // even if preemption alone would not displace it. If it faulted and
    // switched, skip ordinary preemption this tick.
    if !dispatch::watchdog_tick(frame) {
        dispatch::preempt(frame);
    }
}

fn put_dec(mut v: u64) {
    if v == 0 {
        uart::put_byte(b'0');
        return;
    }
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    while v > 0 {
        i -= 1;
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
    }
    for &b in &buf[i..] {
        uart::put_byte(b);
    }
}
