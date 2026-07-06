//! On-target task dispatcher (AXIOM-SCHEDRT-001/002/003/005/006).
//!
//! Requirement reference: docs/13_DISPATCH.md, docs/09_SCHEDULER_MODEL.md.
//!
//! A minimal cooperative dispatcher for U-mode tasks. Each task has a
//! control block holding its address space root and a saved trap frame
//! (its full register context). On `sys_yield`/`sys_exit` the live trap
//! frame is snapshotted into the current task, the next Ready task is
//! selected (round-robin, excluding Killed/Faulted/Blocked — the state
//! machine is the authority, docs/09 §4), its address space is
//! activated, and its saved frame is loaded so the trap return resumes
//! it. riscv64-only.

use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use kernel::memory::PhysAddr;

use crate::paging_hw;
use crate::trap::TrapFrame;
use crate::uart;

// Syscall numbers handled by the dispatcher (docs/04_SYSCALL_MODEL.md).
const SYS_YIELD: u64 = 1;
const SYS_EXIT: u64 = 2;

/// On-target run state of a task control block. Blocked and Faulted are
/// reserved for the IPC (v0.6) and fault (v0.5) stages; the exclusion
/// logic already refuses to select them.
#[derive(Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum RtState {
    Empty,
    Ready,
    Running,
    Blocked,
    Faulted,
    Killed,
}

/// A task control block (AXIOM-SCHEDRT-001; priority added by
/// AXIOM-TIMER-007).
#[derive(Clone, Copy)]
struct Tcb {
    state: RtState,
    /// Fixed priority; higher value = more urgent (docs/09 §2).
    prio: u8,
    /// Physical root of this task's Sv39 address space.
    satp_root: u64,
    /// Saved full register context (AXIOM-SCHEDRT-002).
    frame: TrapFrame,
    name: &'static str,
}

const EMPTY_TCB: Tcb = Tcb {
    state: RtState::Empty,
    prio: 0,
    satp_root: 0,
    frame: TrapFrame { regs: [0; 31], sepc: 0, sstatus: 0 },
    name: "",
};

/// Maximum on-target tasks (v0.3).
pub const MAX_TASKS: usize = 4;

static mut TASKS: [Tcb; MAX_TASKS] = [EMPTY_TCB; MAX_TASKS];
static CURRENT: AtomicUsize = AtomicUsize::new(0);
static ACTIVE: AtomicBool = AtomicBool::new(false);

extern "C" {
    /// S→U transition (arch/riscv64/user_entry.S), used for the very
    /// first task entry.
    fn __enter_user(entry: u64, user_sp: u64, trap_stack_top: u64) -> !;
}

// register_task / start / read_sstatus are the demo entry API, exercised
// only by the multitask demo (feature demo_multitask); on_syscall is the
// always-compiled hook the trap layer calls. allow(dead_code) keeps the
// default build (no feature) warning-clean.
#[allow(dead_code)]
fn read_sstatus() -> u64 {
    let v: u64;
    // SAFETY: side-effect-free privileged CSR read in S-mode.
    unsafe { core::arch::asm!("csrr {v}, sstatus", v = out(reg) v) };
    v
}

/// Register a task in slot `idx` (AXIOM-SCHEDRT-001). The initial saved
/// frame is synthesized to enter U-mode at `entry_va` on `sp_va`,
/// preserving the live sstatus UXL field.
///
/// # Safety
/// Called at boot before dispatching, single hart, distinct `idx`.
#[allow(dead_code)]
pub unsafe fn register_task(
    idx: usize,
    name: &'static str,
    prio: u8,
    root: PhysAddr,
    entry_va: u64,
    sp_va: u64,
) {
    const SSTATUS_SPP: u64 = 1 << 8;
    const SSTATUS_SPIE: u64 = 1 << 5;
    let mut frame = TrapFrame::new_user(entry_va, sp_va);
    // Preserve UXL etc. from the live sstatus; SPP=0 (→U), SPIE=1.
    frame.sstatus = (read_sstatus() & !SSTATUS_SPP) | SSTATUS_SPIE;
    let tcb = Tcb { state: RtState::Ready, prio, satp_root: root.as_u64(), frame, name };
    // SAFETY: exclusive boot-time access to a distinct slot.
    unsafe {
        let tasks = &mut *core::ptr::addr_of_mut!(TASKS);
        tasks[idx] = tcb;
    }
}

fn tasks_mut() -> &'static mut [Tcb; MAX_TASKS] {
    // SAFETY: single hart; the dispatcher is the only accessor and runs
    // in trap/boot context, never re-entrantly (cooperative).
    unsafe { &mut *core::ptr::addr_of_mut!(TASKS) }
}

/// Select the highest-priority Ready task, scanning in round-robin
/// order after `cur` so equal priorities tie-break round-robin
/// (SCHED-P1, docs/09 §4/§2; AXIOM-SCHEDRT-006 + AXIOM-TIMER-007).
/// Killed/Faulted/Blocked are never Ready, so never selected. Returns
/// None if no task is Ready.
fn select_highest(cur: usize) -> Option<usize> {
    let tasks = tasks_mut();
    let mut best: Option<usize> = None;
    for step in 1..=MAX_TASKS {
        let idx = (cur + step) % MAX_TASKS;
        if tasks[idx].state != RtState::Ready {
            continue;
        }
        best = match best {
            None => Some(idx),
            Some(b) if tasks[idx].prio > tasks[b].prio => Some(idx),
            other => other,
        };
    }
    best
}

fn emit(prefix: &str, name: &str) {
    uart::put_str(prefix);
    uart::put_str(name);
    uart::put_str("\n");
}

/// Start dispatching from task 0 (AXIOM-SCHEDRT-003 entry).
///
/// # Safety
/// All registered tasks must have valid address spaces that map the
/// kernel (U=0) and their own code/stack (U=1); `trap_stack_top` must
/// be a valid kernel trap stack.
#[allow(dead_code)]
pub unsafe fn start(trap_stack_top: u64) -> ! {
    ACTIVE.store(true, Ordering::SeqCst);
    let tasks = tasks_mut();
    tasks[0].state = RtState::Running;
    CURRENT.store(0, Ordering::SeqCst);
    let entry = tasks[0].frame.sepc;
    let sp = tasks[0].frame.regs[1];
    let root = PhysAddr::new(tasks[0].satp_root);
    // SAFETY: task 0's address space maps the kernel identity + its own
    // user pages; the switch keeps this code and stack valid.
    unsafe {
        paging_hw::switch_to_user_space(root);
        __enter_user(entry, sp, trap_stack_top)
    }
}

/// Handle a syscall from the dispatcher's perspective. Returns true if
/// the dispatcher consumed it (yield/exit); false to let the normal
/// syscall path run. The trap layer has already advanced `frame.sepc`
/// past the ecall.
pub fn on_syscall(num: u64, frame: &mut TrapFrame) -> bool {
    if !ACTIVE.load(Ordering::SeqCst) {
        return false;
    }
    match num {
        SYS_YIELD => {
            switch(frame, false);
            true
        }
        SYS_EXIT => {
            switch(frame, true);
            true
        }
        _ => false,
    }
}

/// Core context switch (AXIOM-SCHEDRT-003/005). Saves or retires the
/// current task, selects the next Ready task, activates its address
/// space, and loads its saved frame into the live trap frame.
fn switch(frame: &mut TrapFrame, exiting: bool) {
    let cur = CURRENT.load(Ordering::SeqCst);
    let tasks = tasks_mut();

    if exiting {
        emit("SYSCALL name=sys_exit task=", tasks[cur].name);
        tasks[cur].state = RtState::Killed;
        emit("TASK_EXITED task=", tasks[cur].name);
    } else {
        emit("SYSCALL name=sys_yield task=", tasks[cur].name);
        // Snapshot the live context into the current task.
        tasks[cur].frame = *frame;
        tasks[cur].state = RtState::Ready;
    }

    match select_highest(cur) {
        Some(next) => {
            tasks[next].state = RtState::Running;
            CURRENT.store(next, Ordering::SeqCst);
            emit("SCHED selected=", tasks[next].name);
            let root = PhysAddr::new(tasks[next].satp_root);
            // SAFETY: next task's address space maps the kernel (U=0),
            // so the trap handler, trap stack, and this frame remain
            // valid after the switch (docs/12_MMU_SV39.md §5).
            unsafe { paging_hw::switch_to_user_space(root) };
            *frame = tasks[next].frame;
        }
        None => {
            uart::put_str("SCHED idle=all_tasks_done\n");
            uart::put_str("KERNEL alive=true\n");
            uart::put_str("phase=multitask-demo-complete\n");
            loop {
                core::hint::spin_loop();
            }
        }
    }
}

/// Timer preemption point (AXIOM-TIMER-006/007). Preempts the running
/// task only if a strictly higher-priority task is Ready; otherwise the
/// current task continues (the tick is a no-op switch). A lone
/// low-priority infinite loop therefore keeps running but stays
/// preemptible — the kernel never freezes (docs/15 §5).
pub fn preempt(frame: &mut TrapFrame) {
    if !ACTIVE.load(Ordering::SeqCst) {
        return;
    }
    let cur = CURRENT.load(Ordering::SeqCst);
    let tasks = tasks_mut();
    let Some(next) = select_highest(cur) else {
        return; // no Ready task besides the current one
    };
    if next == cur || tasks[next].prio <= tasks[cur].prio {
        return; // nobody out-ranks the running task
    }
    // Preempt: save the running task, switch to the higher one.
    uart::put_str("SCHED preempt=");
    uart::put_str(tasks[cur].name);
    uart::put_str(" selected=");
    uart::put_str(tasks[next].name);
    uart::put_str("\n");
    tasks[cur].frame = *frame;
    tasks[cur].state = RtState::Ready;
    tasks[next].state = RtState::Running;
    CURRENT.store(next, Ordering::SeqCst);
    let root = PhysAddr::new(tasks[next].satp_root);
    // SAFETY: next task's address space maps the kernel (U=0); the trap
    // handler, trap stack, and frame stay valid across the switch.
    unsafe { paging_hw::switch_to_user_space(root) };
    *frame = tasks[next].frame;
}
