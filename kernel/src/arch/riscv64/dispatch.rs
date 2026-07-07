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
// Real OS syscalls (docs/25_OS_BOOT_FLOW.md §4).
const SYS_TASK_START: u64 = 8;
const SYS_CON_WRITE: u64 = 9;
const SYS_CON_READ: u64 = 10;
const SYS_INFO: u64 = 11;
const SYS_TASK_KILL: u64 = 12;
const SYS_TASK_RESTART: u64 = 13;
const SYS_SHUTDOWN: u64 = 14;
// Device syscalls (docs/31 §7/§8; AXIOM-DRV-003/004).
const SYS_DEVICE_INFO: u64 = 15;
const SYS_MMIO_READ: u64 = 16;
const SYS_MMIO_WRITE: u64 = 17;
const SYS_DMA_READ: u64 = 18;
const SYS_DMA_WRITE: u64 = 19;
const SYS_IRQ_RAISE: u64 = 20;

// Result codes returned in a0 (docs/04_SYSCALL_MODEL.md).
const ERR_INVALID_CAP: i64 = -2;
const ERR_INSUFFICIENT_RIGHTS: i64 = -3;
const ERR_WRONG_OBJECT_TYPE: i64 = -4;
const ERR_INVALID_ARG: i64 = -5;
const ERR_MSG_TOO_LARGE: i64 = -6;
const ERR_NO_SLOT: i64 = -7;

// On-target capabilities (AXIOM-CAPRT). Rights bits match the host
// model (docs/06_CAPABILITY_MODEL.md §2).
const OTYPE_ENDPOINT: u8 = 0;
/// Console byte mechanism (docs/25 §4): Send = write, Recv = read.
const OTYPE_CONSOLE: u8 = 1;
/// Task-control authority (start/kill/restart/shutdown).
const OTYPE_CONTROL: u8 = 2;
/// Read-only introspection (sys_info).
const OTYPE_INFO: u8 = 3;
/// Device object access (docs/31 §10): rights are the DEV_RIGHT_* bits.
const OTYPE_DEVICE: u8 = 4;
const RIGHT_SEND: u16 = 1 << 3;
const RIGHT_RECV: u16 = 1 << 4;
const RIGHT_CONTROL: u16 = 1 << 7;
/// Capability slots per task (small, static).
pub const CAPS_PER_TASK: usize = 8;

/// On-target capability: (object type, object id, rights). The running
/// form of the host `Capability` (docs/06 §3).
#[derive(Clone, Copy)]
pub struct Cap {
    otype: u8,
    object_id: u32,
    rights: u16,
}

/// Boot-time capability constructors for the service table
/// (docs/25 §5). Deny-by-default: a service has exactly what its table
/// entry mints, nothing else.
#[allow(dead_code)] // os_boot-only API
pub const fn cap_endpoint(object_id: u32, rights: u16) -> Cap {
    Cap {
        otype: OTYPE_ENDPOINT,
        object_id,
        rights,
    }
}
#[allow(dead_code)] // os_boot-only API
pub const fn cap_console(rights: u16) -> Cap {
    Cap {
        otype: OTYPE_CONSOLE,
        object_id: 0,
        rights,
    }
}
#[allow(dead_code)] // os_boot-only API
pub const fn cap_control() -> Cap {
    Cap {
        otype: OTYPE_CONTROL,
        object_id: 0,
        rights: RIGHT_CONTROL,
    }
}
#[allow(dead_code)] // os_boot-only API
pub const fn cap_info() -> Cap {
    Cap {
        otype: OTYPE_INFO,
        object_id: 0,
        rights: RIGHT_RECV,
    }
}
#[allow(dead_code)] // os_boot-only API
pub const CAP_RIGHT_SEND: u16 = RIGHT_SEND;
#[allow(dead_code)] // os_boot-only API
pub const CAP_RIGHT_RECV: u16 = RIGHT_RECV;
#[allow(dead_code)] // os_boot-only API
pub const CAP_RIGHT_CONTROL: u16 = RIGHT_CONTROL;
// Filesystem rights (docs/28 §7): declarative bits on the fs endpoint
// capability; the kernel checks transport rights only and never
// parses the fs protocol.
#[allow(dead_code)] // os_boot-only API
pub const CAP_RIGHT_FS_READ: u16 = 1 << 5;
#[allow(dead_code)] // os_boot-only API
pub const CAP_RIGHT_FS_LIST: u16 = 1 << 6;
// Storage rights (docs/29 §5): declarative bits on the storage
// endpoint capability; the kernel never parses storage requests.
#[allow(dead_code)] // os_boot-only API
pub const CAP_RIGHT_STORAGE_INFO: u16 = 1 << 8;
#[allow(dead_code)] // os_boot-only API
pub const CAP_RIGHT_STORAGE_READ: u16 = 1 << 9;
// Device rights (docs/31 §10): meaningful on OTYPE_DEVICE capabilities
// only. Deny-by-default; bit values mirror the host model
// (kernel/src/device). No task holds all of them (least privilege).
#[allow(dead_code)] // os_boot-only API
pub const DEV_RIGHT_INFO: u16 = 1 << 0;
#[allow(dead_code)] // os_boot-only API
pub const DEV_RIGHT_MMIO_READ: u16 = 1 << 1;
#[allow(dead_code)] // os_boot-only API
pub const DEV_RIGHT_MMIO_WRITE: u16 = 1 << 2;
#[allow(dead_code)] // os_boot-only API
pub const DEV_RIGHT_DMA_READ: u16 = 1 << 3;
#[allow(dead_code)] // os_boot-only API
pub const DEV_RIGHT_DMA_WRITE: u16 = 1 << 4;
#[allow(dead_code)] // os_boot-only API
pub const DEV_RIGHT_IRQ_RECEIVE: u16 = 1 << 5;
#[allow(dead_code)] // os_boot-only API
pub const DEV_RIGHT_DRIVER_CONTROL: u16 = 1 << 6;

/// Boot-time device capability constructor (docs/31 §10).
#[allow(dead_code)] // os_boot-only API
pub const fn cap_device(object_id: u32, rights: u16) -> Cap {
    Cap {
        otype: OTYPE_DEVICE,
        object_id,
        rights,
    }
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
    /// Initial user context, kept so sys_task_restart can rebuild the
    /// frame (docs/25 §4).
    entry_va: u64,
    stack_top_va: u64,
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
    entry_va: 0,
    stack_top_va: 0,
};

/// Maximum on-target tasks (10 since the app phase, docs/27 §2).
pub const MAX_TASKS: usize = 12;

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

/// Endpoint ids used on target: 0 = app channel (docs/27), 1 = shell
/// line channel / demo log, 2 = fault channel, 3 = event channel,
/// 4 = filesystem channel (docs/28), 5 = storage channel (docs/29),
/// 6 = driver-manager channel, 7 = driver command channel, 8 = driver
/// IRQ events (docs/31).
const NUM_ENDPOINTS: usize = 9;
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
        entry_va,
        stack_top_va: sp_va,
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

/// Mint a non-endpoint capability into task `idx`'s slot 0 (boot-time;
/// used for init_service's task-control authority, docs/25 §5).
///
/// # Safety
/// Called at boot before dispatching, single hart.
#[allow(dead_code)]
pub unsafe fn set_boot_cap(idx: usize, cap: Cap) {
    tasks_mut()[idx].caps[0] = Some(cap);
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
fn deny_cap(cur: usize, check: CapCheck) -> i64 {
    ring_push(EV_CAP_DENIED, cur);
    uart::put_str("CAP_DENIED task=");
    uart::put_str(tasks_mut()[cur].name);
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

fn put_hex(mut v: u64) {
    uart::put_str("0x");
    if v == 0 {
        uart::put_byte(b'0');
        return;
    }
    let mut buf = [0u8; 16];
    let mut i = buf.len();
    while v > 0 {
        i -= 1;
        let d = (v & 0xf) as u8;
        buf[i] = if d < 10 { b'0' + d } else { b'a' + (d - 10) };
        v >>= 4;
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
        DELIVERED.fetch_add(1, Ordering::SeqCst);
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
        SYS_TASK_START => {
            sys_task_start(frame);
            true
        }
        SYS_CON_WRITE => {
            sys_con_write(frame);
            true
        }
        SYS_CON_READ => {
            sys_con_read(frame);
            true
        }
        SYS_INFO => {
            sys_info(frame);
            true
        }
        SYS_TASK_KILL => {
            sys_task_kill(frame);
            true
        }
        SYS_TASK_RESTART => {
            sys_task_restart(frame);
            true
        }
        SYS_SHUTDOWN => {
            sys_shutdown(frame);
            true
        }
        SYS_DEVICE_INFO => {
            sys_device_info(frame);
            true
        }
        SYS_MMIO_READ => {
            sys_mmio_read(frame);
            true
        }
        SYS_MMIO_WRITE => {
            sys_mmio_write(frame);
            true
        }
        SYS_DMA_READ => {
            sys_dma_read(frame);
            true
        }
        SYS_DMA_WRITE => {
            sys_dma_write(frame);
            true
        }
        SYS_IRQ_RAISE => {
            sys_irq_raise(frame);
            true
        }
        _ => false,
    }
}

/// Cooperative yield/exit switch (AXIOM-SCHEDRT-003/005).
fn switch(frame: &mut TrapFrame, exiting: bool) {
    let cur = CURRENT.load(Ordering::SeqCst);
    let tasks = tasks_mut();

    // The OS boot flow's console service yields between input polls; a
    // yield that re-selects the same task is pure noise there, so it
    // stays silent. Demo builds keep the full trace their tests assert.
    let quiet = cfg!(feature = "os_boot") && !exiting;

    if exiting {
        emit("SYSCALL name=sys_exit task=", tasks[cur].name);
        tasks[cur].state = RtState::Killed;
        emit("TASK_EXITED task=", tasks[cur].name);
        ring_push(EV_EXITED, cur);
    } else {
        tasks[cur].frame = *frame;
        tasks[cur].state = RtState::Ready;
    }

    match select_highest(cur) {
        Some(next) => {
            if !(quiet && next == cur) {
                if !exiting {
                    emit("SYSCALL name=sys_yield task=", tasks[cur].name);
                }
                emit("SCHED selected=", tasks[next].name);
            }
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
    TICKS.fetch_add(1, Ordering::SeqCst);
    let misses = WATCHDOG_MISS.fetch_add(1, Ordering::SeqCst) + 1;
    if misses <= WATCHDOG_WINDOW {
        return false;
    }
    let cur = CURRENT.load(Ordering::SeqCst);
    let cur_name = tasks_mut()[cur].name;
    ring_push(EV_FAULT, cur);
    emit("FAULT type=WatchdogTimeout task=", cur_name);
    uart::put_str("CONTAIN scope=user reason=watchdog_timeout action=faulted kernel=alive\n");
    tasks_mut()[cur].state = RtState::Faulted;
    irq_drop_for_task(cur);
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

/// Contain a synchronous user fault (page fault, illegal instruction)
/// for the running task while the dispatcher is active: the task is
/// Faulted, the supervisor and logger are notified over their fault/
/// event channels, and the next ready task runs (docs/26 §3). Returns
/// false when the dispatcher is not active (single-task demos use the
/// v0.2 continuation path instead).
pub fn contain_fault(frame: &mut TrapFrame, reason: &str) -> bool {
    if !ACTIVE.load(Ordering::SeqCst) {
        return false;
    }
    let cur = CURRENT.load(Ordering::SeqCst);
    let cur_name = tasks_mut()[cur].name;
    ring_push(EV_FAULT, cur);
    emit("FAULT type=PageFault task=", cur_name);
    uart::put_str("CONTAIN scope=user reason=");
    uart::put_str(reason);
    uart::put_str(" action=faulted kernel=alive\n");
    tasks_mut()[cur].state = RtState::Faulted;
    irq_drop_for_task(cur);
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
            frame.set_a0(deny_cap(cur, other));
            return;
        }
    };

    if len > IPC_MSG_MAX {
        frame.set_a0(ERR_MSG_TOO_LARGE);
        emit("IPC_DENIED op=send reason=msg_too_large task=", cur_name);
        return;
    }
    if !(len <= IPC_MSG_MAX && in_readable_window(buf, len)) {
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
            frame.set_a0(deny_cap(cur, other));
            return;
        }
    };

    // Recv-side IRQ delivery (docs/31 §9): a pending device event on
    // this endpoint goes to its registered receiver immediately. The
    // receiver's satp is active, so the one-byte copy happens now.
    if let Some(dev) = irq_pending_for(ep_id, cur) {
        if cap < 1 || !valid_user_buf(dst, 1) {
            frame.set_a0(ERR_INVALID_ARG);
            emit("IPC_DENIED op=recv reason=bad_buffer task=", cur_name);
            return;
        }
        irq_route_set(
            dev,
            IrqRoute {
                receiver: cur,
                pending: false,
            },
        );
        copy_bytes_to_user(dst, &[IRQ_EVENT_CODE]);
        emit_irq_delivered(cur_name, DEVICES[dev].name);
        frame.set_a0(1);
        return;
    }

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

// ---- Device objects (AXIOM-DRV-002; docs/31 §6) --------------------------

/// One kernel device object: identity plus the MMIO window, IRQ route,
/// and DMA buffer a driver may be granted mediated access to. Static,
/// frozen at build time; carries no device protocol policy (docs/31
/// §11). The host-testable twin is `kernel::device`.
#[allow(dead_code)] // region/route fields consumed by AXIOM-DRV-003..005
struct DeviceDef {
    name: &'static str,
    kind: &'static str,
    mmio_name: &'static str,
    /// Physical base of the MMIO register window (docs/31 §7). User
    /// code never sees it — only offsets inside the window.
    mmio_base: u64,
    mmio_size: u64,
    /// Endpoint the device's (synthetic) IRQ line is routed to
    /// (docs/31 §9).
    irq_endpoint: u32,
    irq_name: &'static str,
    dma_name: &'static str,
    dma_size: u64,
}

/// The kernel device table (docs/31 §6). v1.5: one device, `block0`,
/// on the first virtio-mmio transport window of QEMU virt (docs/30 §1).
/// The window is real; the device behind it is not driven in v1.5.
static DEVICES: [DeviceDef; 1] = [DeviceDef {
    name: "block0",
    kind: "block_skeleton",
    mmio_name: "virtio_mmio0",
    mmio_base: 0x1000_1000,
    mmio_size: 0x200,
    irq_endpoint: 8,
    irq_name: "driver_irq",
    dma_name: "block0_dma",
    dma_size: 4096,
}];

/// Resolve `cap_index` in task `cur`'s table for a **device** capability
/// with the `required` rights, in the fixed order of docs/06 §4:
/// bounds → occupancy → object type → device id known → rights. On
/// success returns the device id the capability names. This lookup runs
/// before any device operation touches anything (docs/31 §10).
fn device_cap_check(cur: usize, cap_index: usize, required: u16) -> CapCheck {
    let tasks = tasks_mut();
    if cap_index >= CAPS_PER_TASK {
        return CapCheck::InvalidCap;
    }
    match tasks[cur].caps[cap_index] {
        None => CapCheck::InvalidCap,
        Some(c) if c.otype != OTYPE_DEVICE => CapCheck::WrongType,
        Some(c) if c.object_id as usize >= DEVICES.len() => CapCheck::InvalidCap,
        Some(c) if c.rights & required != required => CapCheck::InsufficientRights,
        Some(c) => CapCheck::Ok(c.object_id),
    }
}

/// Announce the static device table and its IRQ routes (boot-time,
/// docs/31 §6/§9). Mechanism only: registration mints no capability.
#[allow(dead_code)] // os_boot-only API
pub fn register_devices() {
    let mut i = 0usize;
    while i < DEVICES.len() {
        let d = &DEVICES[i];
        uart::put_str("DEVICE registered=");
        uart::put_str(d.name);
        uart::put_str(" kind=");
        uart::put_str(d.kind);
        uart::put_str("\n");
        uart::put_str("IRQ registered source=");
        uart::put_str(d.name);
        uart::put_str(" endpoint=");
        uart::put_str(d.irq_name);
        uart::put_str("\n");
        i += 1;
    }
}

// ---- Device syscalls (AXIOM-DRV-003; docs/31 §7) --------------------------

/// Emit one device-operation denial line (docs/31 §7/§8) and record the
/// event. `kind` names the denied mechanism (MMIO_DENIED / DMA_DENIED /
/// DEVICE_DENIED). Nothing was touched when this is called.
fn deny_device_op(cur: usize, kind: &str, reason: &str) {
    ring_push(EV_CAP_DENIED, cur);
    uart::put_str(kind);
    uart::put_str(" task=");
    uart::put_str(tasks_mut()[cur].name);
    uart::put_str(" reason=");
    uart::put_str(reason);
    uart::put_str("\n");
}

/// Map a failed device capability check to (error code, denial reason).
fn device_check_err(check: CapCheck) -> (i64, &'static str) {
    match check {
        CapCheck::InvalidCap => (ERR_INVALID_CAP, "no_valid_capability"),
        CapCheck::WrongType => (ERR_WRONG_OBJECT_TYPE, "no_valid_capability"),
        CapCheck::InsufficientRights => (ERR_INSUFFICIENT_RIGHTS, "insufficient_rights"),
        // Ok is never passed to the error path; mapped defensively.
        CapCheck::Ok(_) => (ERR_INVALID_ARG, "unexpected"),
    }
}

/// Apply and log the device grants a starting task was minted
/// (docs/31 §7/§8/§9): authority is explicit and evidence-visible,
/// never ambient. Registers the task as its device's IRQ receiver when
/// the grant carries `irq_receive`. Called once per task, on the first
/// (minting) start.
fn announce_device_grants(slot: usize) {
    let tasks = tasks_mut();
    let mut i = 0usize;
    while i < CAPS_PER_TASK {
        if let Some(c) = tasks[slot].caps[i] {
            if c.otype == OTYPE_DEVICE && (c.object_id as usize) < DEVICES.len() {
                let d = &DEVICES[c.object_id as usize];
                if c.rights & DEV_RIGHT_IRQ_RECEIVE != 0 {
                    let route = irq_route(c.object_id as usize);
                    irq_route_set(
                        c.object_id as usize,
                        IrqRoute {
                            receiver: slot,
                            pending: route.pending,
                        },
                    );
                }
                if c.rights & (DEV_RIGHT_MMIO_READ | DEV_RIGHT_MMIO_WRITE) != 0 {
                    uart::put_str("MMIO grant task=");
                    uart::put_str(tasks[slot].name);
                    uart::put_str(" device=");
                    uart::put_str(d.name);
                    uart::put_str(" region=");
                    uart::put_str(d.mmio_name);
                    uart::put_str("\n");
                }
                if c.rights & (DEV_RIGHT_DMA_READ | DEV_RIGHT_DMA_WRITE) != 0 {
                    uart::put_str("DMA grant task=");
                    uart::put_str(tasks[slot].name);
                    uart::put_str(" buffer=");
                    uart::put_str(d.dma_name);
                    uart::put_str(" size=");
                    put_dec(d.dma_size);
                    uart::put_str("\n");
                }
            }
        }
        i += 1;
    }
}

// ---- IRQ event delivery (AXIOM-DRV-005; docs/31 §9) ----------------------

/// The bounded one-byte device event delivered to a driver.
const IRQ_EVENT_CODE: u8 = 0x10;
/// sys_irq_raise result: event held pending (receiver alive, not
/// waiting). 0 = delivered now; ERR_NO_SLOT = dropped, receiver dead —
/// which is how driver_manager observes driver death (docs/31 §9).
const IRQ_PENDING: i64 = 1;

/// One synthetic IRQ route: the registered receiver task (MAX_TASKS =
/// none yet) plus the coalesced pending bit. Delivery state lives here
/// and in the route's endpoint waiter slot only — it can never touch
/// unrelated endpoint state (docs/31 §9).
#[derive(Clone, Copy)]
struct IrqRoute {
    receiver: usize,
    pending: bool,
}

/// One route per device table entry.
static mut IRQ_ROUTES: [IrqRoute; 1] = [IrqRoute {
    receiver: MAX_TASKS,
    pending: false,
}];

fn irq_route(dev: usize) -> IrqRoute {
    // SAFETY: single-hart, non-reentrant dispatcher state.
    unsafe { (*addr_of!(IRQ_ROUTES))[dev] }
}
fn irq_route_set(dev: usize, r: IrqRoute) {
    // SAFETY: single-hart, non-reentrant dispatcher state.
    unsafe { (*addr_of_mut!(IRQ_ROUTES))[dev] = r };
}

fn emit_irq_delivered(to: &str, source: &str) {
    uart::put_str("IRQ delivered to=");
    uart::put_str(to);
    uart::put_str(" source=");
    uart::put_str(source);
    uart::put_str("\n");
}

/// Raise the synthetic IRQ of device `dev` (docs/31 §9): deliver now if
/// the authorized driver is blocked receiving on the route's endpoint,
/// hold it pending (coalesced) if the driver is alive but busy, drop it
/// loudly if the driver is dead. Only the registered receiver is ever
/// delivered to.
fn irq_raise(dev: usize) -> i64 {
    let d = &DEVICES[dev];
    let route = irq_route(dev);
    let tasks = tasks_mut();
    let dead = route.receiver >= MAX_TASKS
        || matches!(
            tasks[route.receiver].state,
            RtState::Faulted | RtState::Killed | RtState::Empty
        );
    if dead {
        irq_route_set(
            dev,
            IrqRoute {
                receiver: route.receiver,
                pending: false,
            },
        );
        uart::put_str("IRQ_DROPPED reason=driver_not_ready\n");
        return ERR_NO_SLOT;
    }
    if let Ep::ReceiverWaiting { tid, dst, cap } = ep_get(d.irq_endpoint) {
        if tid == route.receiver && cap >= 1 && valid_user_buf(dst, 1) {
            let mut pm = PendingMsg {
                dst,
                len: 1,
                data: [0; IPC_MSG_MAX],
            };
            pm.data[0] = IRQ_EVENT_CODE;
            tasks[tid].pending_ipc = Some(pm);
            tasks[tid].state = RtState::Ready;
            ep_set(d.irq_endpoint, Ep::Idle);
            emit_irq_delivered(tasks[tid].name, d.name);
            return 0;
        }
        // A waiter that is not the registered receiver (or handed an
        // invalid buffer) never gets a device event; its wait state is
        // left untouched and the event is dropped safely.
        uart::put_str("IRQ_DROPPED reason=unauthorized_receiver\n");
        return ERR_NO_SLOT;
    }
    irq_route_set(
        dev,
        IrqRoute {
            receiver: route.receiver,
            pending: true,
        },
    );
    IRQ_PENDING
}

/// If endpoint `ep_id` is an IRQ route with a pending event for task
/// `cur`, return the device index (recv-side delivery, docs/31 §9).
fn irq_pending_for(ep_id: u32, cur: usize) -> Option<usize> {
    let mut i = 0usize;
    while i < DEVICES.len() {
        if DEVICES[i].irq_endpoint == ep_id {
            let route = irq_route(i);
            if route.pending && route.receiver == cur {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

/// Drop policy on driver death (docs/31 §9): a Faulted/Killed driver
/// cannot service its device, so its pending event is dropped loudly —
/// never queued across a restart. Called from the fault/kill paths.
fn irq_drop_for_task(slot: usize) {
    let mut i = 0usize;
    while i < DEVICES.len() {
        let route = irq_route(i);
        if route.receiver == slot && route.pending {
            irq_route_set(
                i,
                IrqRoute {
                    receiver: route.receiver,
                    pending: false,
                },
            );
            uart::put_str("IRQ_DROPPED reason=driver_not_ready\n");
        }
        i += 1;
    }
}

/// sys_irq_raise: a0 = device cap index. Requires `driver_control` —
/// held only by driver_manager in v1.5 (docs/31 §10). Synthetic
/// injection standing in for a real PLIC interrupt; doubles as the
/// manager's liveness probe via its result code.
fn sys_irq_raise(frame: &mut TrapFrame) {
    let cur = CURRENT.load(Ordering::SeqCst);
    let cap_index = frame.regs[9] as usize; // a0
    let dev_id = match device_cap_check(cur, cap_index, DEV_RIGHT_DRIVER_CONTROL) {
        CapCheck::Ok(id) => id,
        other => {
            let (code, reason) = device_check_err(other);
            deny_device_op(cur, "IRQ_DENIED", reason);
            frame.set_a0(code);
            return;
        }
    };
    frame.set_a0(irq_raise(dev_id as usize));
}

// ---- DMA-visible buffer grant (AXIOM-DRV-004; docs/31 §8) ----------------

#[repr(C, align(4096))]
struct DmaPage([u8; 4096]);

/// The modeled DMA-visible bounce buffer for block0 (docs/31 §8): one
/// kernel-owned page, physically contiguous by construction and
/// identity-mapped, so its kernel VA *is* its physical address — the
/// property a future virtio driver needs. **No device masters it in
/// v1.5**; access is exclusively through the capability-gated DMA
/// syscalls, so a driver can never reach kernel memory outside the
/// granted buffer and a user task can never nominate arbitrary memory
/// as DMA.
static mut BLOCK0_DMA: DmaPage = DmaPage([0; 4096]);

/// Physical (= kernel virtual) base of a device's granted DMA buffer.
/// v1.5: every registered device maps to the single bounce page.
fn dma_base(_d: &DeviceDef) -> u64 {
    addr_of!(BLOCK0_DMA) as u64
}

/// Shared body of the DMA syscalls: capability lookup (fixed order),
/// then bounds/alignment against the granted DmaRegion, then the one
/// access inside the kernel-owned bounce page.
fn dma_access(frame: &mut TrapFrame, required: u16, write: bool) {
    let cur = CURRENT.load(Ordering::SeqCst);
    let cap_index = frame.regs[9] as usize; // a0
    let offset = frame.regs[10]; // a1
    let width = frame.regs[11]; // a2
    let value = frame.regs[12]; // a3 (write only)
    let dev_id = match device_cap_check(cur, cap_index, required) {
        CapCheck::Ok(id) => id,
        other => {
            let (code, reason) = device_check_err(other);
            deny_device_op(cur, "DMA_DENIED", reason);
            frame.set_a0(code);
            return;
        }
    };
    let d = &DEVICES[dev_id as usize];
    if !kernel::device::access_in_bounds(d.dma_size, offset, width) {
        deny_device_op(cur, "DMA_DENIED", "out_of_range");
        frame.set_a0(ERR_INVALID_ARG);
        return;
    }
    let addr = dma_base(d) + offset;
    if write {
        // SAFETY: addr lies inside the kernel-owned bounce page;
        // width/alignment validated above; single hart.
        unsafe {
            match width {
                1 => write_volatile(addr as *mut u8, value as u8),
                2 => write_volatile(addr as *mut u16, value as u16),
                _ => write_volatile(addr as *mut u32, value as u32),
            }
        }
        frame.set_a0(0);
    } else {
        // SAFETY: as above, read-only.
        let v: u64 = unsafe {
            match width {
                1 => read_volatile(addr as *const u8) as u64,
                2 => read_volatile(addr as *const u16) as u64,
                _ => read_volatile(addr as *const u32) as u64,
            }
        };
        frame.set_a0(v as i64);
    }
}

/// sys_dma_read: a0 = device cap index, a1 = offset, a2 = width.
/// Requires `dma_read` (docs/31 §8).
fn sys_dma_read(frame: &mut TrapFrame) {
    dma_access(frame, DEV_RIGHT_DMA_READ, false);
}

/// sys_dma_write: a0 = device cap index, a1 = offset, a2 = width,
/// a3 = value. Requires `dma_write` (docs/31 §8).
fn sys_dma_write(frame: &mut TrapFrame) {
    dma_access(frame, DEV_RIGHT_DMA_WRITE, true);
}

/// sys_device_info: a0 = device cap index, a1 = buffer VA, a2 = max.
/// Requires the `device_info` right. Writes a one-line bounded
/// description of the device object (docs/31 §6).
fn sys_device_info(frame: &mut TrapFrame) {
    let cur = CURRENT.load(Ordering::SeqCst);
    let cap_index = frame.regs[9] as usize; // a0
    let va = frame.regs[10]; // a1
    let max = frame.regs[11] as usize; // a2
    let dev_id = match device_cap_check(cur, cap_index, DEV_RIGHT_INFO) {
        CapCheck::Ok(id) => id,
        other => {
            let (code, reason) = device_check_err(other);
            deny_device_op(cur, "DEVICE_DENIED", reason);
            frame.set_a0(code);
            return;
        }
    };
    if !in_stack_window(va, max.min(INFO_MAX)) {
        frame.set_a0(ERR_INVALID_ARG);
        return;
    }
    let d = &DEVICES[dev_id as usize];
    let mut out = InfoBuf::new();
    out.push_str("device id=");
    out.push_dec(dev_id as u64);
    out.push_str(" name=");
    out.push_str(d.name);
    out.push_str(" kind=");
    out.push_str(d.kind);
    out.push_str(" mmio=");
    out.push_str(d.mmio_name);
    out.push_str(" mmio_size=");
    out.push_dec(d.mmio_size);
    out.push_str(" irq=");
    out.push_str(d.irq_name);
    out.push_str(" dma=");
    out.push_str(d.dma_name);
    out.push_str(" dma_size=");
    out.push_dec(d.dma_size);
    out.push_str("\n");
    let n = out.len.min(max);
    copy_bytes_to_user(va, &out.buf[..n]);
    frame.set_a0(n as i64);
}

/// sys_mmio_read: a0 = device cap index, a1 = offset, a2 = width.
/// Requires `mmio_read`. The kernel resolves the capability, checks
/// bounds/alignment against the granted region, and performs the one
/// volatile access itself — the driver never sees a physical address
/// (docs/31 §7). Returns the value read.
fn sys_mmio_read(frame: &mut TrapFrame) {
    let cur = CURRENT.load(Ordering::SeqCst);
    let cap_index = frame.regs[9] as usize; // a0
    let offset = frame.regs[10]; // a1
    let width = frame.regs[11]; // a2
    let dev_id = match device_cap_check(cur, cap_index, DEV_RIGHT_MMIO_READ) {
        CapCheck::Ok(id) => id,
        other => {
            let (code, reason) = device_check_err(other);
            deny_device_op(cur, "MMIO_DENIED", reason);
            frame.set_a0(code);
            return;
        }
    };
    let d = &DEVICES[dev_id as usize];
    if !kernel::device::access_in_bounds(d.mmio_size, offset, width) {
        deny_device_op(cur, "MMIO_DENIED", "out_of_range");
        frame.set_a0(ERR_INVALID_ARG);
        return;
    }
    let addr = d.mmio_base + offset;
    // SAFETY: addr lies inside the kernel-mapped device window of a
    // registered device object; width/alignment validated above.
    let value: u64 = unsafe {
        match width {
            1 => read_volatile(addr as *const u8) as u64,
            2 => read_volatile(addr as *const u16) as u64,
            _ => read_volatile(addr as *const u32) as u64,
        }
    };
    uart::put_str("MMIO read task=");
    uart::put_str(tasks_mut()[cur].name);
    uart::put_str(" device=");
    uart::put_str(d.name);
    uart::put_str(" offset=");
    put_dec(offset);
    uart::put_str(" value=");
    put_hex(value);
    uart::put_str("\n");
    frame.set_a0(value as i64);
}

/// sys_mmio_write: a0 = device cap index, a1 = offset, a2 = width,
/// a3 = value. Requires `mmio_write` — which no v1.5 task holds
/// (docs/31 §10): the mechanism is complete, the grant is withheld.
fn sys_mmio_write(frame: &mut TrapFrame) {
    let cur = CURRENT.load(Ordering::SeqCst);
    let cap_index = frame.regs[9] as usize; // a0
    let offset = frame.regs[10]; // a1
    let width = frame.regs[11]; // a2
    let value = frame.regs[12]; // a3
    let dev_id = match device_cap_check(cur, cap_index, DEV_RIGHT_MMIO_WRITE) {
        CapCheck::Ok(id) => id,
        other => {
            let (code, reason) = device_check_err(other);
            deny_device_op(cur, "MMIO_DENIED", reason);
            frame.set_a0(code);
            return;
        }
    };
    let d = &DEVICES[dev_id as usize];
    if !kernel::device::access_in_bounds(d.mmio_size, offset, width) {
        deny_device_op(cur, "MMIO_DENIED", "out_of_range");
        frame.set_a0(ERR_INVALID_ARG);
        return;
    }
    let addr = d.mmio_base + offset;
    // SAFETY: addr lies inside the kernel-mapped device window of a
    // registered device object; width/alignment validated above.
    unsafe {
        match width {
            1 => write_volatile(addr as *mut u8, value as u8),
            2 => write_volatile(addr as *mut u16, value as u16),
            _ => write_volatile(addr as *mut u32, value as u32),
        }
    }
    uart::put_str("MMIO write task=");
    uart::put_str(tasks_mut()[cur].name);
    uart::put_str(" device=");
    uart::put_str(d.name);
    uart::put_str(" offset=");
    put_dec(offset);
    uart::put_str(" value=");
    put_hex(value);
    uart::put_str("\n");
    frame.set_a0(0);
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
    ring_push(EV_RECOVERY, cur);
    frame.set_a0(0); // OK
}

// ---- Real OS layer: service table, introspection, console, control ------
// (AXIOM-INIT-002..005, AXIOM-SHELL-002..009; docs/25_OS_BOOT_FLOW.md,
// docs/26_SHELL.md.)

/// Uptime in timer ticks (incremented by watchdog_tick on every tick).
static TICKS: AtomicU64 = AtomicU64::new(0);
/// Deferred IPC deliveries completed (evidence counter for sys_info).
static DELIVERED: AtomicU64 = AtomicU64::new(0);

// Kernel event ring (docs/25 §4, `events`/`faults` shell commands).
const EV_STARTED: u8 = 0;
const EV_EXITED: u8 = 1;
const EV_FAULT: u8 = 2;
const EV_CAP_DENIED: u8 = 3;
const EV_RECOVERY: u8 = 4;
const EV_KILLED: u8 = 5;
const EV_RESTARTED: u8 = 6;
const RING_LEN: usize = 32;
static mut RING: [(u8, u8); RING_LEN] = [(0, 0); RING_LEN];
static RING_HEAD: AtomicUsize = AtomicUsize::new(0);

fn ring_push(kind: u8, slot: usize) {
    let head = RING_HEAD.fetch_add(1, Ordering::SeqCst);
    // SAFETY: single-hart, non-reentrant dispatcher state.
    unsafe { (*addr_of_mut!(RING))[head % RING_LEN] = (kind, slot as u8) };
}

fn ev_name(kind: u8) -> &'static str {
    match kind {
        EV_STARTED => "task_started",
        EV_EXITED => "task_exited",
        EV_FAULT => "fault",
        EV_CAP_DENIED => "cap_denied",
        EV_RECOVERY => "recovery_applied",
        EV_KILLED => "task_killed",
        EV_RESTARTED => "task_restarted",
        _ => "unknown",
    }
}

fn state_name(s: RtState) -> &'static str {
    match s {
        RtState::Empty => "empty",
        RtState::Ready => "ready",
        RtState::Running => "running",
        RtState::Blocked => "blocked",
        RtState::Faulted => "faulted",
        RtState::Killed => "killed",
    }
}

/// One entry of the boot-frozen service table (docs/25 §3). The kernel
/// holds the mechanism; init_service owns the start order (policy).
pub struct ServiceDef {
    pub name: &'static str,
    /// Link-time kernel VA of the sectioned entry function.
    pub entry: u64,
    /// Physical base of the service's private stack page.
    pub stack_phys: u64,
    pub prio: u8,
    /// TCB slot == address-space index.
    pub slot: usize,
    pub caps: [Option<Cap>; CAPS_PER_TASK],
}

static mut SERVICE_TABLE: Option<&'static [ServiceDef]> = None;

/// Install the service table (boot-time, before dispatching).
///
/// # Safety
/// Boot-time only, single hart, called once.
#[allow(dead_code)] // os_boot-only API
pub unsafe fn set_service_table(table: &'static [ServiceDef]) {
    // SAFETY: exclusive boot-time access.
    unsafe { *addr_of_mut!(SERVICE_TABLE) = Some(table) };
}

fn service_table() -> Option<&'static [ServiceDef]> {
    // SAFETY: written once at boot, read-only afterwards.
    unsafe { *addr_of!(SERVICE_TABLE) }
}

/// True if the caller holds a capability of `otype` with `required`
/// rights (deny-by-default search over its table; docs/25 §4 syscalls
/// carry no capability index — authority is a property of the task).
fn cap_find(cur: usize, otype: u8, required: u16) -> bool {
    tasks_mut()[cur]
        .caps
        .iter()
        .any(|c| matches!(c, Some(c) if c.otype == otype && c.rights & required == required))
}

fn deny_authority(cur: usize, what: &str) -> i64 {
    ring_push(EV_CAP_DENIED, cur);
    uart::put_str("CAP_DENIED task=");
    uart::put_str(tasks_mut()[cur].name);
    uart::put_str(" reason=");
    uart::put_str(what);
    uart::put_str("\n");
    ERR_INVALID_CAP
}

fn in_stack_window(va: u64, len: usize) -> bool {
    va >= USER_DATA_VA
        && va
            .checked_add(len as u64)
            .is_some_and(|end| end <= USER_DATA_END)
}

/// Read-only user source ranges for sys_con_write: the caller's stack
/// page or the mapped user region (sectioned rodata lives there).
fn in_readable_window(va: u64, len: usize) -> bool {
    if in_stack_window(va, len) {
        return true;
    }
    let (start, end) = paging_hw::user_region_va_span();
    va >= start && va.checked_add(len as u64).is_some_and(|e| e <= end)
}

/// Start service `index` from the table: build its address space,
/// register the TCB, mint its capabilities (AXIOM-INIT-002).
fn start_service(index: usize) -> i64 {
    let Some(table) = service_table() else {
        return ERR_INVALID_ARG;
    };
    let Some(def) = table.get(index) else {
        return ERR_INVALID_ARG;
    };
    match tasks_mut()[def.slot].state {
        RtState::Empty => {
            let uas = paging_hw::build_service_address_space(def.slot, def.entry, def.stack_phys);
            // SAFETY: single hart; the slot is Empty, no live task uses it.
            unsafe {
                register_task(
                    def.slot,
                    def.name,
                    def.prio,
                    uas.root,
                    uas.entry_va,
                    uas.stack_top_va,
                );
            }
            tasks_mut()[def.slot].caps = def.caps;
            // Device grants are minted exactly here; log them once
            // (docs/31 §7/§8).
            announce_device_grants(def.slot);
        }
        // A terminated app returns to Available and is re-armed with
        // its initial frame and unchanged capabilities (docs/27 §6).
        RtState::Killed | RtState::Faulted => {
            const SSTATUS_SPP: u64 = 1 << 8;
            const SSTATUS_SPIE: u64 = 1 << 5;
            let tasks = tasks_mut();
            let mut f = TrapFrame::new_user(tasks[def.slot].entry_va, tasks[def.slot].stack_top_va);
            f.sstatus = (read_sstatus() & !SSTATUS_SPP) | SSTATUS_SPIE;
            tasks[def.slot].frame = f;
            tasks[def.slot].pending_ipc = None;
            tasks[def.slot].state = RtState::Ready;
        }
        _ => return ERR_NO_SLOT,
    }
    ring_push(EV_STARTED, def.slot);
    emit("SERVICE started=", def.name);
    def.slot as i64
}

fn sys_task_start(frame: &mut TrapFrame) {
    let cur = CURRENT.load(Ordering::SeqCst);
    if !cap_find(cur, OTYPE_CONTROL, RIGHT_CONTROL) {
        frame.set_a0(deny_authority(cur, "no_control_capability"));
        return;
    }
    let index = frame.regs[9] as usize; // a0
    frame.set_a0(start_service(index));
}

fn sys_con_write(frame: &mut TrapFrame) {
    let cur = CURRENT.load(Ordering::SeqCst);
    if !cap_find(cur, OTYPE_CONSOLE, RIGHT_SEND) {
        frame.set_a0(deny_authority(cur, "no_console_write_capability"));
        return;
    }
    let va = frame.regs[9]; // a0
    let len = frame.regs[10] as usize; // a1
    const CON_WRITE_MAX: usize = 256;
    if len > CON_WRITE_MAX || !in_readable_window(va, len) {
        frame.set_a0(ERR_INVALID_ARG);
        return;
    }
    set_sum();
    for i in 0..len {
        // SAFETY: validated user range, SUM set, byte-wise volatile read.
        let b = unsafe { read_volatile((va + i as u64) as *const u8) };
        if b == b'\n' {
            uart::put_byte(b'\r');
        }
        uart::put_byte(b);
    }
    clear_sum();
    frame.set_a0(len as i64);
}

fn sys_con_read(frame: &mut TrapFrame) {
    let cur = CURRENT.load(Ordering::SeqCst);
    if !cap_find(cur, OTYPE_CONSOLE, RIGHT_RECV) {
        frame.set_a0(deny_authority(cur, "no_console_read_capability"));
        return;
    }
    let va = frame.regs[9]; // a0
    let max = (frame.regs[10] as usize).min(IPC_MSG_MAX); // a1
    if !in_stack_window(va, max) {
        frame.set_a0(ERR_INVALID_ARG);
        return;
    }
    let mut n = 0usize;
    set_sum();
    while n < max {
        let Some(b) = uart::get_byte() else { break };
        // SAFETY: validated user range, SUM set, byte-wise volatile write.
        unsafe { write_volatile((va + n as u64) as *mut u8, b) };
        n += 1;
    }
    clear_sum();
    frame.set_a0(n as i64);
}

// ---- sys_info (read-only introspection, docs/25 §4) ---------------------

const INFO_MAX: usize = 768;
struct InfoBuf {
    buf: [u8; INFO_MAX],
    len: usize,
}

impl InfoBuf {
    fn new() -> Self {
        InfoBuf {
            buf: [0; INFO_MAX],
            len: 0,
        }
    }
    fn push_byte(&mut self, b: u8) {
        if self.len < INFO_MAX {
            self.buf[self.len] = b;
            self.len += 1;
        }
    }
    fn push_str(&mut self, s: &str) {
        for b in s.bytes() {
            self.push_byte(b);
        }
    }
    fn push_dec(&mut self, mut v: u64) {
        let mut tmp = [0u8; 20];
        let mut i = tmp.len();
        if v == 0 {
            self.push_byte(b'0');
            return;
        }
        while v > 0 {
            i -= 1;
            tmp[i] = b'0' + (v % 10) as u8;
            v /= 10;
        }
        for &b in &tmp[i..] {
            self.push_byte(b);
        }
    }
}

fn info_tasks(out: &mut InfoBuf) {
    let tasks = tasks_mut();
    for (i, t) in tasks.iter().enumerate() {
        if t.state == RtState::Empty {
            continue;
        }
        out.push_str("task idx=");
        out.push_dec(i as u64);
        out.push_str(" name=");
        out.push_str(t.name);
        out.push_str(" prio=");
        out.push_dec(t.prio as u64);
        out.push_str(" state=");
        out.push_str(state_name(t.state));
        out.push_str("\n");
    }
}

fn info_ring(out: &mut InfoBuf, only_faults: bool) {
    let head = RING_HEAD.load(Ordering::SeqCst);
    let count = head.min(RING_LEN);
    let tasks = tasks_mut();
    for k in 0..count {
        let idx = (head - count + k) % RING_LEN;
        // SAFETY: single-hart dispatcher state, read-only here.
        let (kind, slot) = unsafe { (*addr_of!(RING))[idx] };
        if only_faults && kind != EV_FAULT && kind != EV_CAP_DENIED {
            continue;
        }
        out.push_str("evt kind=");
        out.push_str(ev_name(kind));
        out.push_str(" task=");
        let name = tasks[slot as usize].name;
        out.push_str(if name.is_empty() { "?" } else { name });
        out.push_str("\n");
    }
}

fn info_ipc(out: &mut InfoBuf) {
    for id in 0..NUM_ENDPOINTS {
        out.push_str("ep id=");
        out.push_dec(id as u64);
        out.push_str(" state=");
        out.push_str(match ep_get(id as u32) {
            Ep::Idle => "idle",
            Ep::SenderWaiting { .. } => "sender_waiting",
            Ep::ReceiverWaiting { .. } => "receiver_waiting",
        });
        out.push_str("\n");
    }
    out.push_str("deferred_deliveries=");
    out.push_dec(DELIVERED.load(Ordering::SeqCst));
    out.push_str("\n");
}

fn info_caps(out: &mut InfoBuf) {
    let tasks = tasks_mut();
    for t in tasks.iter() {
        if t.state == RtState::Empty {
            continue;
        }
        out.push_str("caps task=");
        out.push_str(t.name);
        for c in t.caps.iter().flatten() {
            out.push_str(match c.otype {
                OTYPE_ENDPOINT => " endpoint",
                OTYPE_CONSOLE => " console",
                OTYPE_CONTROL => " control",
                OTYPE_INFO => " info",
                OTYPE_DEVICE => " device",
                _ => " ?",
            });
        }
        out.push_str("\n");
    }
}

fn info_memory(out: &mut InfoBuf) {
    out.push_str("mmu=sv39 kernel_pages=U0 wxorx=true\n");
    out.push_str("kernel_base=0x80200000\n");
    out.push_str("user_region_va=0x10000\n");
    out.push_str("user_stack_va=0x200000 stack_pages=1\n");
}

fn sys_info(frame: &mut TrapFrame) {
    let cur = CURRENT.load(Ordering::SeqCst);
    if !cap_find(cur, OTYPE_INFO, RIGHT_RECV) {
        frame.set_a0(deny_authority(cur, "no_info_capability"));
        return;
    }
    let kind = frame.regs[9]; // a0
    let va = frame.regs[10]; // a1
    let max = frame.regs[11] as usize; // a2
    if !in_stack_window(va, max.min(INFO_MAX)) {
        frame.set_a0(ERR_INVALID_ARG);
        return;
    }
    let mut out = InfoBuf::new();
    match kind {
        0 => info_tasks(&mut out),
        1 => info_ring(&mut out, true),
        2 => info_ipc(&mut out),
        3 => info_caps(&mut out),
        4 => info_memory(&mut out),
        5 => {
            out.push_str("uptime ticks=");
            out.push_dec(TICKS.load(Ordering::SeqCst));
            out.push_str(" tick_interval=100000 timebase_hz=10000000\n");
        }
        6 => info_ring(&mut out, false),
        _ => {
            frame.set_a0(ERR_INVALID_ARG);
            return;
        }
    }
    let n = out.len.min(max);
    copy_bytes_to_user(va, &out.buf[..n]);
    frame.set_a0(n as i64);
}

fn sys_task_kill(frame: &mut TrapFrame) {
    let cur = CURRENT.load(Ordering::SeqCst);
    if !cap_find(cur, OTYPE_CONTROL, RIGHT_CONTROL) {
        frame.set_a0(deny_authority(cur, "no_control_capability"));
        return;
    }
    let slot = frame.regs[9] as usize; // a0
    let tasks = tasks_mut();
    if slot >= MAX_TASKS || tasks[slot].state == RtState::Empty {
        frame.set_a0(ERR_INVALID_ARG);
        return;
    }
    tasks[slot].state = RtState::Killed;
    tasks[slot].pending_ipc = None;
    irq_drop_for_task(slot);
    ring_push(EV_KILLED, slot);
    emit("TASK_KILLED task=", tasks[slot].name);
    if slot == cur {
        // Killing yourself schedules away like sys_exit.
        match select_highest(cur) {
            Some(next) => {
                emit("SCHED selected=", tasks_mut()[next].name);
                resume_task(next, frame);
            }
            None => idle_halt(),
        }
    } else {
        frame.set_a0(0);
    }
}

fn sys_task_restart(frame: &mut TrapFrame) {
    let cur = CURRENT.load(Ordering::SeqCst);
    if !cap_find(cur, OTYPE_CONTROL, RIGHT_CONTROL) {
        frame.set_a0(deny_authority(cur, "no_control_capability"));
        return;
    }
    let slot = frame.regs[9] as usize; // a0
    let tasks = tasks_mut();
    if slot >= MAX_TASKS
        || slot == cur
        || tasks[slot].state == RtState::Empty
        || tasks[slot].entry_va == 0
    {
        frame.set_a0(ERR_INVALID_ARG);
        return;
    }
    const SSTATUS_SPP: u64 = 1 << 8;
    const SSTATUS_SPIE: u64 = 1 << 5;
    let mut f = TrapFrame::new_user(tasks[slot].entry_va, tasks[slot].stack_top_va);
    f.sstatus = (read_sstatus() & !SSTATUS_SPP) | SSTATUS_SPIE;
    tasks[slot].frame = f;
    tasks[slot].pending_ipc = None;
    tasks[slot].state = RtState::Ready;
    ring_push(EV_RESTARTED, slot);
    emit("TASK_RESTARTED task=", tasks[slot].name);
    frame.set_a0(0);
}

fn sys_shutdown(frame: &mut TrapFrame) {
    let cur = CURRENT.load(Ordering::SeqCst);
    if !cap_find(cur, OTYPE_CONTROL, RIGHT_CONTROL) {
        frame.set_a0(deny_authority(cur, "no_control_capability"));
        return;
    }
    uart::put_str("SHUTDOWN controlled=true by=");
    uart::put_str(tasks_mut()[cur].name);
    uart::put_str("\n");
    // SBI System Reset (SRST, EID 0x53525354 FID 0): type 0 = shutdown,
    // reason 0 = no reason.
    // SAFETY: SBI call; does not return on success.
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 0x53525354u64,
            in("a6") 0u64,
            in("a0") 0u64,
            in("a1") 0u64,
            options(noreturn)
        )
    }
}
