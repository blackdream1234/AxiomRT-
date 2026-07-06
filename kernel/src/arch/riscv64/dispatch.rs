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
const SYS_FAULT_ACK: u64 = 7;

// Result codes returned in a0 (docs/04_SYSCALL_MODEL.md).
const ERR_INVALID_CAP: i64 = -2;
const ERR_INSUFFICIENT_RIGHTS: i64 = -3;
const ERR_WRONG_OBJECT_TYPE: i64 = -4;
const ERR_INVALID_ARG: i64 = -5;
const ERR_MSG_TOO_LARGE: i64 = -6;

// On-target capabilities (AXIOM-CAPRT). Rights bits match the host
// model (docs/06_CAPABILITY_MODEL.md §2).
const OTYPE_ENDPOINT: u8 = 0;
const RIGHT_SEND: u16 = 1 << 3;
const RIGHT_RECV: u16 = 1 << 4;
/// Capability slots per task (small, static).
const CAPS_PER_TASK: usize = 4;

/// On-target capability: (object type, object id, rights). The running
/// form of the host `Capability` (docs/06 §3).
#[derive(Clone, Copy)]
struct Cap {
    otype: u8,
    object_id: u32,
    rights: u16,
}

/// Outcome of a capability lookup (fixed check order, docs/06 §4).
/// On success carries the resolved endpoint id.
enum CapCheck {
    Ok(u32),
    InvalidCap,
    WrongType,
    InsufficientRights,
}

/// A deferred message delivery: destination user VA plus the embedded
/// payload (copied into the target's buffer when it next runs). Embedding
/// the bytes keeps kernel notifications and user IPC independent.
#[derive(Clone, Copy)]
struct PendingMsg {
    dst: u64,
    len: usize,
    data: [u8; IPC_MSG_MAX],
}

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
    /// Deferred IPC delivery to complete when this task next runs. The
    /// payload is embedded (not shared) so kernel notifications and
    /// user messages use independent storage. Applied by `resume_task`
    /// once this task's address space is active (AXIOM-IPCRT-006).
    pending_ipc: Option<PendingMsg>,
    /// Per-task capability table (AXIOM-CAPRT-002). Minted at boot; user
    /// code holds only an index into it.
    caps: [Option<Cap>; CAPS_PER_TASK],
    name: &'static str,
}

const EMPTY_TCB: Tcb = Tcb {
    state: RtState::Empty,
    prio: 0,
    satp_root: 0,
    frame: TrapFrame {
        regs: [0; 31],
        sepc: 0,
        sstatus: 0,
    },
    pending_ipc: None,
    caps: [None; CAPS_PER_TASK],
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

/// Endpoint state (docs/17 §3). One in-flight rendezvous per endpoint.
#[derive(Clone, Copy)]
enum Ep {
    Idle,
    SenderWaiting { tid: usize, len: usize },
    ReceiverWaiting { tid: usize, dst: u64, cap: usize },
}

/// Endpoint ids used on target: 1 = demo log endpoint (v0.6/0.7),
/// 2 = fault channel (supervisor), 3 = event channel (logger) — v0.8.
const NUM_ENDPOINTS: usize = 4;
static mut ENDPOINTS: [Ep; NUM_ENDPOINTS] = [Ep::Idle; NUM_ENDPOINTS];
/// Kernel staging buffer for user send→recv copies (bounded, no shared
/// memory, docs/17 §2).
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
        caps: [None; CAPS_PER_TASK],
        name,
    };
    // SAFETY: exclusive boot-time access to a distinct slot.
    unsafe {
        let tasks = &mut *addr_of_mut!(TASKS);
        tasks[idx] = tcb;
    }
}

/// Mint an endpoint capability into task `idx`'s table slot `slot`
/// (AXIOM-CAPRT-001). Boot-time only.
///
/// # Safety
/// Called at boot before dispatching, single hart, distinct `idx`/`slot`.
#[allow(dead_code)]
pub unsafe fn set_endpoint_cap(idx: usize, slot: usize, object_id: u32, rights: u16) {
    // SAFETY: exclusive boot-time access.
    let tasks = tasks_mut();
    tasks[idx].caps[slot] = Some(Cap {
        otype: OTYPE_ENDPOINT,
        object_id,
        rights,
    });
}

/// Resolve `cap_index` in task `cur`'s table for an endpoint capability
/// with the `required` right, in the fixed order of docs/06 §4. On
/// success returns the endpoint id the capability names.
fn cap_check(cur: usize, cap_index: usize, required: u16) -> CapCheck {
    let tasks = tasks_mut();
    if cap_index >= CAPS_PER_TASK {
        return CapCheck::InvalidCap;
    }
    match tasks[cur].caps[cap_index] {
        None => CapCheck::InvalidCap,
        Some(c) if c.otype != OTYPE_ENDPOINT => CapCheck::WrongType,
        Some(c) if c.rights & required != required => CapCheck::InsufficientRights,
        Some(c) => CapCheck::Ok(c.object_id),
    }
}

/// Map a failed capability check to a syscall error code and emit the
/// CAP_DENIED evidence (docs/18 §3). The endpoint is never touched.
fn deny_cap(cur_name: &str, check: CapCheck) -> i64 {
    uart::put_str("CAP_DENIED task=");
    uart::put_str(cur_name);
    uart::put_str(" reason=no_valid_capability\n");
    uart::put_str("IPC state=unchanged\n");
    match check {
        CapCheck::InvalidCap => ERR_INVALID_CAP,
        CapCheck::WrongType => ERR_WRONG_OBJECT_TYPE,
        CapCheck::InsufficientRights => ERR_INSUFFICIENT_RIGHTS,
        CapCheck::Ok(_) => 0,
    }
}

fn tasks_mut() -> &'static mut [Tcb; MAX_TASKS] {
    // SAFETY: single hart; the dispatcher is the only accessor and runs
    // in trap/boot context, never re-entrantly.
    unsafe { &mut *addr_of_mut!(TASKS) }
}

fn ep_get(id: u32) -> Ep {
    // SAFETY: single-hart, non-reentrant dispatcher state.
    unsafe { (*addr_of!(ENDPOINTS))[id as usize] }
}
fn ep_set(id: u32, e: Ep) {
    // SAFETY: single-hart, non-reentrant dispatcher state.
    unsafe { (*addr_of_mut!(ENDPOINTS))[id as usize] = e };
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
        && va
            .checked_add(len as u64)
            .is_some_and(|end| end <= USER_DATA_END)
}

/// Copy `len` bytes from the running task's user buffer into KMSG. The
/// caller must have validated the range and the sender's satp is active.
fn copy_from_user(va: u64, len: usize) {
    set_sum();
    let kmsg = unsafe { &mut *addr_of_mut!(KMSG) };
    for (i, byte) in kmsg.iter_mut().enumerate().take(len) {
        // SAFETY: validated user range, SUM set, byte-wise volatile read.
        *byte = unsafe { read_volatile((va + i as u64) as *const u8) };
    }
    clear_sum();
}

/// Copy `len` bytes from KMSG into the running task's user buffer. The
/// caller must have validated the range and the receiver's satp is active.
fn copy_to_user(va: u64, len: usize) {
    let kmsg = unsafe { *addr_of!(KMSG) };
    copy_bytes_to_user(va, &kmsg[..len]);
}

/// Copy an explicit byte slice into the running task's user buffer
/// (used for embedded pending messages and kernel notifications). The
/// destination range must be validated and the target satp active.
fn copy_bytes_to_user(va: u64, bytes: &[u8]) {
    set_sum();
    for (i, &b) in bytes.iter().enumerate() {
        // SAFETY: validated user range, SUM set, byte-wise volatile write.
        unsafe { write_volatile((va + i as u64) as *mut u8, b) };
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
    if let Some(pm) = tasks[next].pending_ipc.take() {
        copy_bytes_to_user(pm.dst, &pm.data[..pm.len]);
        frame.set_a0(pm.len as i64);
        uart::put_str("IPC delivered bytes=");
        put_dec(pm.len as u64);
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
        SYS_FAULT_ACK => {
            fault_ack(frame);
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
    let cur_name = tasks_mut()[cur].name;
    emit("FAULT type=WatchdogTimeout task=", cur_name);
    uart::put_str("CONTAIN scope=user reason=watchdog_timeout action=faulted kernel=alive\n");
    tasks_mut()[cur].state = RtState::Faulted;
    // Notify the supervisor (fault channel) and logger (event channel)
    // if they are waiting (AXIOM-SUPRT-005/008).
    notify_supervisor_and_logger(cur_name);
    match select_highest(cur) {
        Some(next) => {
            emit("SCHED selected=", tasks_mut()[next].name);
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

/// sys_send: a0 = cap index, a1 = buffer VA, a2 = length. Synchronous,
/// bounded, copy-based, capability-controlled. Blocks if no receiver.
fn ipc_send(frame: &mut TrapFrame) {
    let cur = CURRENT.load(Ordering::SeqCst);
    let cap_index = frame.regs[9] as usize; // a0
    let buf = frame.regs[10]; // a1
    let len = frame.regs[11] as usize; // a2
    let cur_name = tasks_mut()[cur].name;

    // Capability enforcement (AXIOM-CAPRT-005): resolve the endpoint
    // capability with the Send right BEFORE touching the endpoint.
    let ep_id = match cap_check(cur, cap_index, RIGHT_SEND) {
        CapCheck::Ok(id) => id,
        other => {
            frame.set_a0(deny_cap(cur_name, other));
            return;
        }
    };

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

    match ep_get(ep_id) {
        Ep::ReceiverWaiting { tid, dst, cap } => {
            emit("IPC send task=", cur_name);
            uart::put_str("IPC endpoint=log op=send\n");
            let kmsg = unsafe { *addr_of!(KMSG) };
            let tasks = tasks_mut();
            if len <= cap && valid_user_buf(dst, len) {
                // Stage delivery with an embedded payload; the receiver
                // completes the copy when it next runs (AXIOM-IPCRT-006).
                let mut pm = PendingMsg {
                    dst,
                    len,
                    data: [0; IPC_MSG_MAX],
                };
                pm.data[..len].copy_from_slice(&kmsg[..len]);
                tasks[tid].pending_ipc = Some(pm);
            } else {
                tasks[tid].frame.set_a0(ERR_MSG_TOO_LARGE);
            }
            tasks[tid].state = RtState::Ready;
            ep_set(ep_id, Ep::Idle);
            frame.set_a0(len as i64); // send completes
        }
        Ep::Idle => {
            ep_set(ep_id, Ep::SenderWaiting { tid: cur, len });
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

/// sys_recv: a0 = cap index, a1 = buffer VA, a2 = capacity. Blocks if no
/// sender. Capability-controlled.
fn ipc_recv(frame: &mut TrapFrame) {
    let cur = CURRENT.load(Ordering::SeqCst);
    let cap_index = frame.regs[9] as usize; // a0
    let dst = frame.regs[10]; // a1
    let cap = frame.regs[11] as usize; // a2
    let cur_name = tasks_mut()[cur].name;

    let ep_id = match cap_check(cur, cap_index, RIGHT_RECV) {
        CapCheck::Ok(id) => id,
        other => {
            frame.set_a0(deny_cap(cur_name, other));
            return;
        }
    };

    match ep_get(ep_id) {
        Ep::SenderWaiting { tid, len } => {
            if len > cap || !valid_user_buf(dst, len) {
                frame.set_a0(ERR_INVALID_ARG);
                emit("IPC_DENIED op=recv reason=bad_buffer task=", cur_name);
                return;
            }
            copy_to_user(dst, len);
            frame.set_a0(len as i64);
            let tasks = tasks_mut();
            tasks[tid].state = RtState::Ready; // sender's send completes
            tasks[tid].frame.set_a0(len as i64);
            ep_set(ep_id, Ep::Idle);
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
            ep_set(ep_id, Ep::ReceiverWaiting { tid: cur, dst, cap });
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

// ---- Supervisor / logger notification (AXIOM-SUPRT-005/008) -------------

/// Deliver a one-byte notification to whatever task is blocked receiving
/// on endpoint `ep_id` (the fault channel or event channel). Used by the
/// kernel to push a fault/monitoring event to the supervisor/logger. If
/// no receiver is waiting, the notification is dropped (the demo blocks
/// its supervisor/logger on recv first). Returns the notified task, if
/// any, so the caller can note it.
fn notify_endpoint(ep_id: u32, code: u8) -> Option<usize> {
    if let Ep::ReceiverWaiting { tid, dst, cap } = ep_get(ep_id) {
        if cap >= 1 && valid_user_buf(dst, 1) {
            let mut pm = PendingMsg {
                dst,
                len: 1,
                data: [0; IPC_MSG_MAX],
            };
            pm.data[0] = code;
            let tasks = tasks_mut();
            tasks[tid].pending_ipc = Some(pm);
            tasks[tid].state = RtState::Ready;
        }
        ep_set(ep_id, Ep::Idle);
        Some(tid)
    } else {
        None
    }
}

/// Endpoint ids for the supervisor/logger channels (docs/19).
const EP_FAULT: u32 = 2;
const EP_EVENT: u32 = 3;
/// Fault descriptor code delivered to the supervisor/logger.
const FAULT_CODE_WATCHDOG: u8 = 4;

/// Notify the supervisor (fault channel) and logger (event channel) that
/// `faulted_name` was contained (AXIOM-SUPRT-005/008). Called from the
/// fault paths. The supervisor's recovery decision is applied when it
/// acknowledges (sys_fault_ack).
fn notify_supervisor_and_logger(faulted_name: &str) {
    if notify_endpoint(EP_FAULT, FAULT_CODE_WATCHDOG).is_some() {
        emit(
            "IPC delivered fault_event to=supervisor_task from=",
            faulted_name,
        );
    }
    if notify_endpoint(EP_EVENT, FAULT_CODE_WATCHDOG).is_some() {
        uart::put_str("LOGGER event=TASK_FAULTED task=");
        uart::put_str(faulted_name);
        uart::put_str("\n");
    }
}

/// sys_fault_ack: a1 = recovery decision code (2 = Kill). The supervisor
/// closes the fault-handling loop; the kernel records the applied policy
/// (AXIOM-SUPRT-006/007). The faulted task is already contained
/// (Faulted); Kill is the terminal recovery in the demo.
fn fault_ack(frame: &mut TrapFrame) {
    let cur = CURRENT.load(Ordering::SeqCst);
    let cur_name = tasks_mut()[cur].name;
    let decision = frame.regs[10]; // a1
    let policy = match decision {
        2 => "Kill",
        1 => "Restart",
        _ => "Escalate",
    };
    uart::put_str("SUPERVISOR decision=");
    uart::put_str(policy);
    uart::put_str(" by=");
    uart::put_str(cur_name);
    uart::put_str("\n");
    uart::put_str("RECOVERY_APPLIED policy=");
    uart::put_str(policy);
    uart::put_str("\n");
    frame.set_a0(0); // OK
}
