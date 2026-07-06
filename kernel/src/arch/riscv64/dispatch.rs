//! On-target task dispatcher and synchronous IPC
//! (AXIOM-SCHEDRT, AXIOM-TIMER, AXIOM-WDOG, AXIOM-IPCRT).
//!
//! Requirement reference: docs/13_DISPATCH.md, docs/15_TIMER_PREEMPTION.md,
//! docs/16_WATCHDOG.md, docs/17_IPC_ONTARGET.md, docs/09_SCHEDULER_MODEL.md.
//!
//! A minimal dispatcher for U-mode tasks. Each task has a control block
//! holding its address space root, saved trap frame, priority, and any
//! pending IPC delivery. Scheduling selects the highest-priority Ready
//! task (priority with round-robin tie-break); the timer preempts and
//! runs the watchdog; `sys_send`/`sys_recv` implement synchronous,
//! bounded, copy-based IPC between address spaces. All resume paths go
//! through `resume_task`, which also completes any deferred IPC delivery
//! now that the target address space is active. riscv64-only.

use core::ptr::{addr_of, addr_of_mut, read_volatile, write_volatile};
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

use kernel::memory::PhysAddr;

use crate::paging_hw;
use crate::trap::TrapFrame;
use crate::uart;

// Syscall numbers (docs/04_SYSCALL_MODEL.md).
const SYS_YIELD: u64 = 1;
const SYS_EXIT: u64 = 2;
const SYS_SEND: u64 = 3;
const SYS_RECV: u64 = 4;

// Result codes returned in a0 (docs/04_SYSCALL_MODEL.md).
const ERR_INVALID_ARG: i64 = -5;
const ERR_MSG_TOO_LARGE: i64 = -6;

/// On-target run state of a task control block. The scheduler never
/// selects Killed/Faulted/Blocked (docs/09 §4).
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

/// A task control block.
#[derive(Clone, Copy)]
struct Tcb {
    state: RtState,
    /// Fixed priority; higher value = more urgent (docs/09 §2).
    prio: u8,
    /// Physical root of this task's Sv39 address space.
    satp_root: u64,
    /// Saved full register context (AXIOM-SCHEDRT-002).
    frame: TrapFrame,
    /// Deferred IPC delivery to complete when this task next runs:
    /// (user destination VA, byte length). Applied by `resume_task`
    /// once this task's address space is active (AXIOM-IPCRT-006).
    pending_ipc: Option<(u64, usize)>,
    name: &'static str,
}

const EMPTY_TCB: Tcb = Tcb {
    state: RtState::Empty,
    prio: 0,
    satp_root: 0,
    frame: TrapFrame { regs: [0; 31], sepc: 0, sstatus: 0 },
    pending_ipc: None,
    name: "",
};

/// Maximum on-target tasks (v0.3).
pub const MAX_TASKS: usize = 4;

static mut TASKS: [Tcb; MAX_TASKS] = [EMPTY_TCB; MAX_TASKS];
static CURRENT: AtomicUsize = AtomicUsize::new(0);
static ACTIVE: AtomicBool = AtomicBool::new(false);

/// Watchdog miss counter for the running task (AXIOM-WDOG-003).
static WATCHDOG_MISS: AtomicU64 = AtomicU64::new(0);
const WATCHDOG_WINDOW: u64 = 4;

// ---- On-target IPC state (AXIOM-IPCRT) ----------------------------------

/// Bounded message size (docs/17 §2; matches the host IPC model).
const IPC_MSG_MAX: usize = 64;
/// User data window the demo message buffers live in (the user stack
/// page mapped by paging_hw at USER_STACK_VA). A user IPC buffer must
/// lie fully inside it (AXIOM-IPCRT-002/003).
const USER_DATA_VA: u64 = 0x20_0000;
const USER_DATA_END: u64 = USER_DATA_VA + 0x1000;

/// Single demo endpoint state (docs/17 §3).
#[derive(Clone, Copy)]
enum Ep {
    Idle,
    SenderWaiting { tid: usize, len: usize },
    ReceiverWaiting { tid: usize, dst: u64, cap: usize },
}

static mut ENDPOINT: Ep = Ep::Idle;
/// Kernel staging buffer: one bounded in-flight message, no shared
/// memory (docs/17 §2).
static mut KMSG: [u8; IPC_MSG_MAX] = [0; IPC_MSG_MAX];

extern "C" {
    fn __enter_user(entry: u64, user_sp: u64, trap_stack_top: u64) -> !;
}

#[allow(dead_code)]
fn read_sstatus() -> u64 {
    let v: u64;
    // SAFETY: side-effect-free privileged CSR read in S-mode.
    unsafe { core::arch::asm!("csrr {v}, sstatus", v = out(reg) v) };
    v
}

/// Register a task in slot `idx` (AXIOM-SCHEDRT-001).
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
    frame.sstatus = (read_sstatus() & !SSTATUS_SPP) | SSTATUS_SPIE;
    let tcb = Tcb {
        state: RtState::Ready,
        prio,
        satp_root: root.as_u64(),
        frame,
        pending_ipc: None,
        name,
    };
    // SAFETY: exclusive boot-time access to a distinct slot.
    unsafe {
        let tasks = &mut *addr_of_mut!(TASKS);
        tasks[idx] = tcb;
    }
}

fn tasks_mut() -> &'static mut [Tcb; MAX_TASKS] {
    // SAFETY: single hart; the dispatcher is the only accessor and runs
    // in trap/boot context, never re-entrantly.
    unsafe { &mut *addr_of_mut!(TASKS) }
}

fn ep_get() -> Ep {
    // SAFETY: single-hart, non-reentrant dispatcher state.
    unsafe { *addr_of!(ENDPOINT) }
}
fn ep_set(e: Ep) {
    // SAFETY: single-hart, non-reentrant dispatcher state.
    unsafe { *addr_of_mut!(ENDPOINT) = e };
}

/// Select the highest-priority Ready task, round-robin among equals
/// (SCHED-P1). Killed/Faulted/Blocked are never Ready (docs/09 §4).
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

// ---- User-memory copy (SUM-gated) ---------------------------------------

fn set_sum() {
    // SAFETY: setting sstatus.SUM permits S-mode to access U pages for
    // the duration of a controlled copy (docs/17 §4). Cleared right after.
    unsafe { core::arch::asm!("csrs sstatus, {b}", b = in(reg) (1u64 << 18)) };
}
fn clear_sum() {
    // SAFETY: clears sstatus.SUM, restoring the default no-U-access rule.
    unsafe { core::arch::asm!("csrc sstatus, {b}", b = in(reg) (1u64 << 18)) };
}

/// True if `[va, va+len)` is a valid user IPC buffer in the active
/// address space's data window (AXIOM-IPCRT-002/003).
fn valid_user_buf(va: u64, len: usize) -> bool {
    len <= IPC_MSG_MAX
        && va >= USER_DATA_VA
        && va.checked_add(len as u64).is_some_and(|end| end <= USER_DATA_END)
}

/// Copy `len` bytes from the running task's user buffer into KMSG. The
/// caller must have validated the range and the sender's satp is active.
fn copy_from_user(va: u64, len: usize) {
    set_sum();
    let kmsg = unsafe { &mut *addr_of_mut!(KMSG) };
    for i in 0..len {
        // SAFETY: validated user range, SUM set, byte-wise volatile read.
        kmsg[i] = unsafe { read_volatile((va + i as u64) as *const u8) };
    }
    clear_sum();
}

/// Copy `len` bytes from KMSG into the running task's user buffer. The
/// caller must have validated the range and the receiver's satp is active.
fn copy_to_user(va: u64, len: usize) {
    set_sum();
    let kmsg = unsafe { &*addr_of!(KMSG) };
    for i in 0..len {
        // SAFETY: validated user range, SUM set, byte-wise volatile write.
        unsafe { write_volatile((va + i as u64) as *mut u8, kmsg[i]) };
    }
    clear_sum();
}

// ---- Scheduling core ----------------------------------------------------

/// Resume `next`: mark Running, activate its address space, load its
/// saved frame, and complete any deferred IPC delivery (now that its
/// satp is active). Central resume path for every scheduling decision.
fn resume_task(next: usize, frame: &mut TrapFrame) {
    let tasks = tasks_mut();
    tasks[next].state = RtState::Running;
    CURRENT.store(next, Ordering::SeqCst);
    WATCHDOG_MISS.store(0, Ordering::SeqCst);
    let root = PhysAddr::new(tasks[next].satp_root);
    // SAFETY: next's address space maps the kernel (U=0), so the trap
    // handler, trap stack, and this frame stay valid across the switch.
    unsafe { paging_hw::switch_to_user_space(root) };
    *frame = tasks[next].frame;
    if let Some((dst, len)) = tasks[next].pending_ipc.take() {
        copy_to_user(dst, len);
        frame.set_a0(len as i64);
        uart::put_str("IPC delivered bytes=");
        put_dec(len as u64);
        uart::put_str("\n");
    }
}

/// Start dispatching from task 0 (AXIOM-SCHEDRT-003 entry).
///
/// # Safety
/// All registered tasks must have valid address spaces mapping the
/// kernel (U=0) and their own code/stack (U=1); `trap_stack_top` valid.
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

/// Handle a syscall. Returns true if the dispatcher consumed it. The
/// trap layer has already advanced `frame.sepc` past the ecall.
pub fn on_syscall(num: u64, frame: &mut TrapFrame) -> bool {
    if !ACTIVE.load(Ordering::SeqCst) {
        return false;
    }
    // Any syscall is a watchdog check-in (AXIOM-WDOG-002).
    WATCHDOG_MISS.store(0, Ordering::SeqCst);
    match num {
        SYS_YIELD => {
            switch(frame, false);
            true
        }
        SYS_EXIT => {
            switch(frame, true);
            true
        }
        SYS_SEND => {
            ipc_send(frame);
            true
        }
        SYS_RECV => {
            ipc_recv(frame);
            true
        }
        _ => false,
    }
}

/// Cooperative yield/exit switch (AXIOM-SCHEDRT-003/005).
fn switch(frame: &mut TrapFrame, exiting: bool) {
    let cur = CURRENT.load(Ordering::SeqCst);
    let tasks = tasks_mut();

    if exiting {
        emit("SYSCALL name=sys_exit task=", tasks[cur].name);
        tasks[cur].state = RtState::Killed;
        emit("TASK_EXITED task=", tasks[cur].name);
    } else {
        emit("SYSCALL name=sys_yield task=", tasks[cur].name);
        tasks[cur].frame = *frame;
        tasks[cur].state = RtState::Ready;
    }

    match select_highest(cur) {
        Some(next) => {
            emit("SCHED selected=", tasks[next].name);
            resume_task(next, frame);
        }
        None => idle_halt(),
    }
}

fn idle_halt() -> ! {
    uart::put_str("SCHED idle=all_tasks_done\n");
    uart::put_str("KERNEL alive=true\n");
    uart::put_str("phase=multitask-demo-complete\n");
    loop {
        core::hint::spin_loop();
    }
}

/// Block the running task and switch away (IPC send/recv with no peer).
fn block_and_switch(frame: &mut TrapFrame) {
    let cur = CURRENT.load(Ordering::SeqCst);
    let tasks = tasks_mut();
    tasks[cur].frame = *frame;
    tasks[cur].state = RtState::Blocked;
    match select_highest(cur) {
        Some(next) => {
            emit("SCHED selected=", tasks[next].name);
            resume_task(next, frame);
        }
        None => {
            uart::put_str("SCHED idle=all_blocked\n");
            uart::put_str("KERNEL alive=true\n");
            loop {
                core::hint::spin_loop();
            }
        }
    }
}

/// Timer preemption (AXIOM-TIMER-006/007): preempt only if out-ranked.
pub fn preempt(frame: &mut TrapFrame) {
    if !ACTIVE.load(Ordering::SeqCst) {
        return;
    }
    let cur = CURRENT.load(Ordering::SeqCst);
    let Some(next) = select_highest(cur) else {
        return;
    };
    let tasks = tasks_mut();
    if next == cur || tasks[next].prio <= tasks[cur].prio {
        return;
    }
    uart::put_str("SCHED preempt=");
    uart::put_str(tasks[cur].name);
    uart::put_str(" selected=");
    uart::put_str(tasks[next].name);
    uart::put_str("\n");
    tasks[cur].frame = *frame;
    tasks[cur].state = RtState::Ready;
    resume_task(next, frame);
}

/// Watchdog tick (AXIOM-WDOG-004/005/006).
pub fn watchdog_tick(frame: &mut TrapFrame) -> bool {
    if !ACTIVE.load(Ordering::SeqCst) {
        return false;
    }
    let misses = WATCHDOG_MISS.fetch_add(1, Ordering::SeqCst) + 1;
    if misses <= WATCHDOG_WINDOW {
        return false;
    }
    let cur = CURRENT.load(Ordering::SeqCst);
    let tasks = tasks_mut();
    emit("FAULT type=WatchdogTimeout task=", tasks[cur].name);
    uart::put_str("CONTAIN scope=user reason=watchdog_timeout action=faulted kernel=alive\n");
    tasks[cur].state = RtState::Faulted;
    match select_highest(cur) {
        Some(next) => {
            emit("SCHED selected=", tasks[next].name);
            resume_task(next, frame);
        }
        None => {
            uart::put_str("SCHED idle=no_ready_task\n");
            uart::put_str("KERNEL alive=true\n");
            loop {
                core::hint::spin_loop();
            }
        }
    }
    true
}

// ---- IPC syscalls (AXIOM-IPCRT-004..009) --------------------------------

/// sys_send: a1 = user buffer VA, a2 = length. Synchronous, bounded,
/// copy-based. Blocks if no receiver is waiting (docs/17 §5).
fn ipc_send(frame: &mut TrapFrame) {
    let cur = CURRENT.load(Ordering::SeqCst);
    let buf = frame.regs[10]; // a1
    let len = frame.regs[11] as usize; // a2
    let cur_name = tasks_mut()[cur].name;

    if len > IPC_MSG_MAX {
        frame.set_a0(ERR_MSG_TOO_LARGE);
        emit("IPC_DENIED op=send reason=msg_too_large task=", cur_name);
        return;
    }
    if !valid_user_buf(buf, len) {
        frame.set_a0(ERR_INVALID_ARG);
        emit("IPC_DENIED op=send reason=bad_buffer task=", cur_name);
        return;
    }
    // Copy the sender's buffer into the kernel now (sender satp active).
    copy_from_user(buf, len);

    match ep_get() {
        Ep::ReceiverWaiting { tid, dst, cap } => {
            emit("IPC send task=", cur_name);
            uart::put_str("IPC endpoint=log op=send\n");
            let tasks = tasks_mut();
            if len <= cap && valid_user_buf(dst, len) {
                // Stage delivery; the receiver completes the copy when
                // it next runs (its satp) — AXIOM-IPCRT-006.
                tasks[tid].pending_ipc = Some((dst, len));
            } else {
                tasks[tid].frame.set_a0(ERR_MSG_TOO_LARGE);
            }
            tasks[tid].state = RtState::Ready;
            ep_set(Ep::Idle);
            frame.set_a0(len as i64); // send completes
            // sender continues running.
        }
        Ep::Idle => {
            ep_set(Ep::SenderWaiting { tid: cur, len });
            emit("IPC send task=", cur_name);
            uart::put_str("IPC endpoint=log op=send state=blocked\n");
            block_and_switch(frame); // send blocks until a receiver
        }
        Ep::SenderWaiting { .. } => {
            frame.set_a0(ERR_INVALID_ARG); // one sender only (bounded)
            emit("IPC_DENIED op=send reason=busy task=", cur_name);
        }
    }
}

/// sys_recv: a1 = user buffer VA, a2 = capacity. Blocks if no sender is
/// waiting (docs/17 §5).
fn ipc_recv(frame: &mut TrapFrame) {
    let cur = CURRENT.load(Ordering::SeqCst);
    let dst = frame.regs[10]; // a1
    let cap = frame.regs[11] as usize; // a2
    let cur_name = tasks_mut()[cur].name;

    match ep_get() {
        Ep::SenderWaiting { tid, len } => {
            if len > cap || !valid_user_buf(dst, len) {
                frame.set_a0(ERR_INVALID_ARG);
                emit("IPC_DENIED op=recv reason=bad_buffer task=", cur_name);
                return;
            }
            // Complete the copy into the receiver's buffer (recv satp).
            copy_to_user(dst, len);
            frame.set_a0(len as i64);
            let tasks = tasks_mut();
            tasks[tid].state = RtState::Ready; // sender's send completes
            tasks[tid].frame.set_a0(len as i64);
            ep_set(Ep::Idle);
            emit("IPC recv task=", cur_name);
            uart::put_str("IPC delivered bytes=");
            put_dec(len as u64);
            uart::put_str("\n");
        }
        Ep::Idle => {
            if cap > IPC_MSG_MAX || !valid_user_buf(dst, cap.min(IPC_MSG_MAX)) {
                frame.set_a0(ERR_INVALID_ARG);
                emit("IPC_DENIED op=recv reason=bad_buffer task=", cur_name);
                return;
            }
            ep_set(Ep::ReceiverWaiting { tid: cur, dst, cap });
            emit("IPC recv task=", cur_name);
            uart::put_str("IPC endpoint=log op=recv state=blocked\n");
            block_and_switch(frame); // recv blocks until a sender
        }
        Ep::ReceiverWaiting { .. } => {
            frame.set_a0(ERR_INVALID_ARG); // one receiver only (bounded)
            emit("IPC_DENIED op=recv reason=busy task=", cur_name);
        }
    }
}
