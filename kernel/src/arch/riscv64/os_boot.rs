//! Real OS boot flow: init_service, console_service, shell_service
//! (AXIOM-INIT-001..005, AXIOM-SHELL-001..009).
//!
//! Requirement reference: docs/25_OS_BOOT_FLOW.md, docs/26_SHELL.md.
//!
//! Everything below the boot function runs in **U-mode** inside the
//! linker-gathered user region (docs/25 §2). Constrained Rust rules:
//! every function and constant is explicitly sectioned, no string
//! literals (they would land in kernel .rodata), no core formatting,
//! no panicking operations, no iterator loops; buffers are raw-pointer
//! accessed and message lengths are compile-time constants, so no
//! bounds-check, memset/memcpy, or core-method call into kernel .text
//! is emitted. The OS build is run in **release** (as every script
//! does): fat LTO guarantees the remaining trivial intrinsics inline.
//! Any violation escapes the mapped region and page-faults — contained,
//! visible, and a bug.

use core::mem::MaybeUninit;
use core::ptr::{addr_of, addr_of_mut, read_volatile, write_volatile};

use crate::dispatch::{
    self, cap_console, cap_control, cap_endpoint, cap_info, Cap, ServiceDef, CAP_RIGHT_CONTROL,
    CAP_RIGHT_RECV, CAP_RIGHT_SEND,
};
use crate::paging_hw;
use crate::timer;
use crate::uart;

// Syscall numbers (docs/04, docs/25 §4).
const SYS_YIELD: u64 = 1;
const SYS_EXIT: u64 = 2;
const SYS_SEND: u64 = 3;
const SYS_RECV: u64 = 4;
const SYS_TASK_START: u64 = 8;
const SYS_CON_WRITE: u64 = 9;
const SYS_CON_READ: u64 = 10;
const SYS_INFO: u64 = 11;
const SYS_TASK_KILL: u64 = 12;
const SYS_TASK_RESTART: u64 = 13;
const SYS_SHUTDOWN: u64 = 14;

// Endpoints (docs/25 §5): 1 = console→shell line channel, 2 = fault
// channel, 3 = event channel (as v0.8).
const EP_LINE: u32 = 1;
const EP_FAULT: u32 = 2;
const EP_EVENT: u32 = 3;

/// Service-table index of the faulty demo task (`run demo`).
const SVC_FAULTY: u64 = 4;

// ---------------------------------------------------------------------
// Boot (S-mode)
// ---------------------------------------------------------------------

#[repr(C, align(4096))]
struct Stack([u8; 4096]);
static mut OS_STACKS: [Stack; 6] = [
    Stack([0; 4096]),
    Stack([0; 4096]),
    Stack([0; 4096]),
    Stack([0; 4096]),
    Stack([0; 4096]),
    Stack([0; 4096]),
];

#[repr(C, align(16))]
struct TrapStack([u8; 8 * 1024]);
static mut OS_TRAP_STACK: TrapStack = TrapStack([0; 8 * 1024]);

const NO_CAPS: [Option<Cap>; 4] = [None, None, None, None];

/// Service table (docs/25 §3). Entry addresses, stacks, and capability
/// grants are runtime values, patched once by `os_boot` before
/// dispatching; the rest is fixed here.
static mut TABLE: [ServiceDef; 5] = [
    ServiceDef {
        name: "supervisor_service",
        entry: 0,
        stack_phys: 0,
        prio: 6,
        slot: 1,
        caps: NO_CAPS,
    },
    ServiceDef {
        name: "logger_service",
        entry: 0,
        stack_phys: 0,
        prio: 5,
        slot: 2,
        caps: NO_CAPS,
    },
    ServiceDef {
        name: "console_service",
        entry: 0,
        stack_phys: 0,
        prio: 1,
        slot: 3,
        caps: NO_CAPS,
    },
    ServiceDef {
        name: "shell_service",
        entry: 0,
        stack_phys: 0,
        prio: 3,
        slot: 4,
        caps: NO_CAPS,
    },
    ServiceDef {
        name: "faulty_task",
        entry: 0,
        stack_phys: 0,
        prio: 4,
        slot: 5,
        caps: NO_CAPS,
    },
];

fn stack_phys(i: usize) -> u64 {
    // SAFETY: addr_of! of a static array element; address-of only.
    unsafe { addr_of!(OS_STACKS[i]) as u64 }
}

/// Boot the real OS flow (docs/25 §1): register only init_service, hand
/// it the task-control capability, install the service table, start the
/// timer, dispatch.
pub fn os_boot() -> ! {
    // SAFETY: single hart, boot-time exclusive access, before start().
    let table: &'static mut [ServiceDef; 5] = unsafe { &mut *addr_of_mut!(TABLE) };
    table[0].entry = supervisor_body as *const () as u64;
    table[0].stack_phys = stack_phys(1);
    table[0].caps[0] = Some(cap_endpoint(EP_FAULT, CAP_RIGHT_RECV | CAP_RIGHT_CONTROL));
    table[1].entry = logger_body as *const () as u64;
    table[1].stack_phys = stack_phys(2);
    table[1].caps[0] = Some(cap_endpoint(EP_EVENT, CAP_RIGHT_RECV));
    table[2].entry = console_body as *const () as u64;
    table[2].stack_phys = stack_phys(3);
    table[2].caps[0] = Some(cap_endpoint(EP_LINE, CAP_RIGHT_SEND));
    table[2].caps[1] = Some(cap_console(CAP_RIGHT_RECV | CAP_RIGHT_SEND));
    table[3].entry = shell_body as *const () as u64;
    table[3].stack_phys = stack_phys(4);
    table[3].caps[0] = Some(cap_endpoint(EP_LINE, CAP_RIGHT_RECV));
    table[3].caps[1] = Some(cap_console(CAP_RIGHT_SEND));
    table[3].caps[2] = Some(cap_info());
    table[3].caps[3] = Some(cap_control());
    table[4].entry = faulty_body as *const () as u64;
    table[4].stack_phys = stack_phys(5);
    // faulty_task: no capabilities at all (its IPC attempt is denied).

    // SAFETY: boot-time, single hart, called once.
    unsafe { dispatch::set_service_table(table) };

    // init_service: slot 0, highest priority, task-control capability.
    let uas =
        paging_hw::build_service_address_space(0, init_body as *const () as u64, stack_phys(0));
    // SAFETY: boot-time registration into the empty slot 0.
    unsafe {
        dispatch::register_task(
            0,
            "init_service",
            7,
            uas.root,
            uas.entry_va,
            uas.stack_top_va,
        );
        dispatch::set_boot_cap(0, cap_control());
    }
    uart::put_str("TASK_STARTED task=init_service\n");

    timer::init();
    timer::arm_next();

    // SAFETY: slot 0 registered with a valid address space; trap stack
    // valid for the trap handler.
    unsafe {
        let trap_stack_top = addr_of!(OS_TRAP_STACK) as u64 + 8 * 1024;
        dispatch::start(trap_stack_top)
    }
}

// ---------------------------------------------------------------------
// U-mode support (user region)
// ---------------------------------------------------------------------

/// Three-argument syscall.
#[link_section = ".user.text"]
#[inline(never)]
fn sys3(num: u64, a0: u64, a1: u64, a2: u64) -> i64 {
    let ret: i64;
    // SAFETY: U-mode ecall; the kernel validates every argument.
    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") a0 => ret,
            in("a1") a1,
            in("a2") a2,
            in("a7") num,
            options(nostack)
        );
    }
    ret
}

/// Write `len` bytes at `p` to the console, chunked to the syscall cap.
#[link_section = ".user.text"]
#[inline(never)]
fn uwrite_ptr(p: *const u8, len: usize) {
    let mut off = 0usize;
    while off < len {
        let mut chunk = len - off;
        if chunk > 256 {
            chunk = 256;
        }
        // SAFETY: in-bounds offset of the caller's buffer.
        let r = sys3(SYS_CON_WRITE, unsafe { p.add(off) } as u64, chunk as u64, 0);
        if r <= 0 {
            return;
        }
        off += chunk;
    }
}

/// Byte-equality of `[p, p+n)` and `[q, q+m)` (manual loop — no memcmp).
#[link_section = ".user.text"]
#[inline(never)]
fn eqs(p: *const u8, n: usize, q: *const u8, m: usize) -> bool {
    if n != m {
        return false;
    }
    let mut i = 0usize;
    while i < n {
        // SAFETY: both pointers in-bounds for n bytes.
        if unsafe { read_volatile(p.add(i)) != read_volatile(q.add(i)) } {
            return false;
        }
        i += 1;
    }
    true
}

/// True if `[p, p+n)` starts with `[q, q+m)`.
#[link_section = ".user.text"]
#[inline(never)]
fn starts_with(p: *const u8, n: usize, q: *const u8, m: usize) -> bool {
    if n < m {
        return false;
    }
    eqs(p, m, q, m)
}

/// Parse a decimal number at `[p+from, p+n)`; u64::MAX on bad input.
/// Manual range checks: RangeInclusive::contains is a cross-crate core
/// call that U-mode code must not risk (file header rules).
#[allow(clippy::manual_range_contains)]
#[link_section = ".user.text"]
#[inline(never)]
fn parse_dec(p: *const u8, from: usize, n: usize) -> u64 {
    if from >= n {
        return u64::MAX;
    }
    let mut v: u64 = 0;
    let mut i = from;
    while i < n {
        // SAFETY: in-bounds read.
        let b = unsafe { read_volatile(p.add(i)) };
        if !(b >= b'0' && b <= b'9') {
            return u64::MAX;
        }
        v = v.wrapping_mul(10).wrapping_add((b - b'0') as u64);
        i += 1;
    }
    v
}

/// Message static in the mapped user region plus its compile-time
/// length (an immediate at the use site — no slice methods in U-mode).
macro_rules! umsg {
    ($name:ident, $len:ident, $lit:literal) => {
        #[link_section = ".user.rodata"]
        static $name: [u8; $lit.len()] = *$lit;
        const $len: usize = $lit.len();
    };
}

/// Emit one region message (pointer via addr_of!, length by const).
macro_rules! uput {
    ($name:ident, $len:ident) => {
        uwrite_ptr(addr_of!($name) as *const u8, $len)
    };
}

// ---------------------------------------------------------------------
// init_service (U-mode): boot policy — start order (AXIOM-INIT-002/003)
// ---------------------------------------------------------------------

#[link_section = ".user.text"]
extern "C" fn init_body() -> ! {
    // Order: supervisor, logger, console, shell (docs/25 §1). The
    // faulty demo task is NOT started at boot; the shell starts it on
    // demand (`run demo`).
    let mut i: u64 = 0;
    while i < 4 {
        sys3(SYS_TASK_START, i, 0, 0);
        i += 1;
    }
    sys3(SYS_EXIT, 0, 0, 0);
    loop {
        sys3(SYS_YIELD, 0, 0, 0);
    }
}

// ---------------------------------------------------------------------
// supervisor / logger / faulty (U-mode, register-only loops as v0.9)
// ---------------------------------------------------------------------

/// Supervisor: recv fault event (cap 0), acknowledge Kill, repeat.
#[link_section = ".user.text"]
extern "C" fn supervisor_body() -> ! {
    // SAFETY: syscall control flow only; never returns.
    unsafe {
        core::arch::asm!(
            "1:",
            "li a0, 0",
            "li a1, 0x200040",
            "li a2, 64",
            "li a7, 4",
            "ecall",
            "li a1, 2",
            "li a7, 7",
            "ecall",
            "j 1b",
            options(noreturn)
        )
    }
}

/// Logger: recv monitoring event (cap 0), repeat.
#[link_section = ".user.text"]
extern "C" fn logger_body() -> ! {
    // SAFETY: syscall control flow only; never returns.
    unsafe {
        core::arch::asm!(
            "1:",
            "li a0, 0",
            "li a1, 0x200040",
            "li a2, 64",
            "li a7, 4",
            "ecall",
            "j 1b",
            options(noreturn)
        )
    }
}

/// Faulty demo task: capability-less IPC (denied), then CPU exhaustion
/// (watchdog-contained). Started from the shell (`run demo`).
#[link_section = ".user.text"]
extern "C" fn faulty_body() -> ! {
    // SAFETY: denied syscall then intentional infinite loop.
    unsafe {
        core::arch::asm!(
            "li a0, 0",
            "li a1, 0x200040",
            "li a2, 4",
            "li a7, 3",
            "ecall",
            "1:",
            "j 1b",
            options(noreturn)
        )
    }
}

// ---------------------------------------------------------------------
// console_service (U-mode): owns input; echo + line assembly
// (AXIOM-SHELL-002)
// ---------------------------------------------------------------------

umsg!(M_NL, M_NL_LEN, b"\n");
umsg!(M_BS, M_BS_LEN, b"\x08 \x08");

#[link_section = ".user.text"]
#[allow(clippy::manual_range_contains)] // no core calls in U-mode
extern "C" fn console_body() -> ! {
    let mut line = MaybeUninit::<[u8; 64]>::uninit();
    let lp = addr_of_mut!(line) as *mut u8;
    let mut ch = MaybeUninit::<[u8; 1]>::uninit();
    let cp = addr_of_mut!(ch) as *mut u8;
    let mut len: usize = 0;
    loop {
        let n = sys3(SYS_CON_READ, cp as u64, 1, 0);
        if n <= 0 {
            // Lowest priority: this poll loop is also the idle task.
            sys3(SYS_YIELD, 0, 0, 0);
            continue;
        }
        // SAFETY: cp holds the byte the kernel just wrote.
        let b = unsafe { read_volatile(cp) };
        if b == b'\r' || b == b'\n' {
            uput!(M_NL, M_NL_LEN);
            sys3(SYS_SEND, 0, lp as u64, len as u64);
            len = 0;
        } else if b == 0x7f || b == 0x08 {
            if len > 0 {
                len -= 1;
                uput!(M_BS, M_BS_LEN);
            }
        } else if len < 63 && b >= 0x20 && b < 0x7f {
            // SAFETY: len < 63 keeps the write in the 64-byte buffer.
            unsafe { write_volatile(lp.add(len), b) };
            len += 1;
            uwrite_ptr(cp, 1); // echo
        }
    }
}

// ---------------------------------------------------------------------
// shell_service (U-mode): parse + execute (AXIOM-SHELL-003..009)
// ---------------------------------------------------------------------

umsg!(
    M_BANNER,
    M_BANNER_LEN,
    b"AxiomRT shell ready (evaluation stage, no certification claim)\n"
);
umsg!(M_PROMPT, M_PROMPT_LEN, b"axiom> ");
umsg!(
    M_HELP,
    M_HELP_LEN,
    b"commands: help version tasks faults ipc caps memory uptime events\n          run demo | kill <idx> | restart <idx> | clear | shutdown\n"
);
umsg!(
    M_VERSION,
    M_VERSION_LEN,
    b"AxiomRT v1.1-os RISC-V 64 microkernel (QEMU evaluation build)\n"
);
umsg!(M_UNKNOWN, M_UNKNOWN_LEN, b"unknown command (try: help)\n");
umsg!(M_ERR, M_ERR_LEN, b"error\n");
umsg!(M_OK, M_OK_LEN, b"ok\n");
umsg!(
    M_DEMO,
    M_DEMO_LEN,
    b"demo: starting faulty_task (watch containment + recovery)\n"
);
umsg!(M_CLEAR, M_CLEAR_LEN, b"\x1b[2J\x1b[H");
umsg!(M_BYE, M_BYE_LEN, b"shutting down\n");

umsg!(C_HELP, C_HELP_LEN, b"help");
umsg!(C_VERSION, C_VERSION_LEN, b"version");
umsg!(C_TASKS, C_TASKS_LEN, b"tasks");
umsg!(C_FAULTS, C_FAULTS_LEN, b"faults");
umsg!(C_IPC, C_IPC_LEN, b"ipc");
umsg!(C_CAPS, C_CAPS_LEN, b"caps");
umsg!(C_MEMORY, C_MEMORY_LEN, b"memory");
umsg!(C_UPTIME, C_UPTIME_LEN, b"uptime");
umsg!(C_EVENTS, C_EVENTS_LEN, b"events");
umsg!(C_RUN_DEMO, C_RUN_DEMO_LEN, b"run demo");
umsg!(C_KILL, C_KILL_LEN, b"kill ");
umsg!(C_RESTART, C_RESTART_LEN, b"restart ");
umsg!(C_CLEAR, C_CLEAR_LEN, b"clear");
umsg!(C_SHUTDOWN, C_SHUTDOWN_LEN, b"shutdown");

/// Is the received line exactly this command?
macro_rules! is_cmd {
    ($bp:expr, $n:expr, $name:ident, $len:ident) => {
        eqs($bp, $n, addr_of!($name) as *const u8, $len)
    };
}

#[link_section = ".user.text"]
#[inline(never)]
fn shell_info(kind: u64, op: *mut u8) {
    let n = sys3(SYS_INFO, kind, op as u64, 640);
    if n > 0 {
        uwrite_ptr(op, n as usize);
    } else {
        uput!(M_ERR, M_ERR_LEN);
    }
}

#[link_section = ".user.text"]
extern "C" fn shell_body() -> ! {
    let mut buf = MaybeUninit::<[u8; 64]>::uninit();
    let bp = addr_of_mut!(buf) as *mut u8;
    let mut out = MaybeUninit::<[u8; 640]>::uninit();
    let op = addr_of_mut!(out) as *mut u8;

    uput!(M_BANNER, M_BANNER_LEN);
    loop {
        uput!(M_PROMPT, M_PROMPT_LEN);
        let r = sys3(SYS_RECV, 0, bp as u64, 64);
        if r <= 0 {
            continue;
        }
        let n = r as usize;
        if is_cmd!(bp, n, C_HELP, C_HELP_LEN) {
            uput!(M_HELP, M_HELP_LEN);
        } else if is_cmd!(bp, n, C_VERSION, C_VERSION_LEN) {
            uput!(M_VERSION, M_VERSION_LEN);
        } else if is_cmd!(bp, n, C_TASKS, C_TASKS_LEN) {
            shell_info(0, op);
        } else if is_cmd!(bp, n, C_FAULTS, C_FAULTS_LEN) {
            shell_info(1, op);
        } else if is_cmd!(bp, n, C_IPC, C_IPC_LEN) {
            shell_info(2, op);
        } else if is_cmd!(bp, n, C_CAPS, C_CAPS_LEN) {
            shell_info(3, op);
        } else if is_cmd!(bp, n, C_MEMORY, C_MEMORY_LEN) {
            shell_info(4, op);
        } else if is_cmd!(bp, n, C_UPTIME, C_UPTIME_LEN) {
            shell_info(5, op);
        } else if is_cmd!(bp, n, C_EVENTS, C_EVENTS_LEN) {
            shell_info(6, op);
        } else if is_cmd!(bp, n, C_RUN_DEMO, C_RUN_DEMO_LEN) {
            uput!(M_DEMO, M_DEMO_LEN);
            if sys3(SYS_TASK_START, SVC_FAULTY, 0, 0) < 0 {
                uput!(M_ERR, M_ERR_LEN);
            }
        } else if starts_with(bp, n, addr_of!(C_KILL) as *const u8, C_KILL_LEN) {
            let idx = parse_dec(bp, C_KILL_LEN, n);
            if idx == u64::MAX || sys3(SYS_TASK_KILL, idx, 0, 0) < 0 {
                uput!(M_ERR, M_ERR_LEN);
            } else {
                uput!(M_OK, M_OK_LEN);
            }
        } else if starts_with(bp, n, addr_of!(C_RESTART) as *const u8, C_RESTART_LEN) {
            let idx = parse_dec(bp, C_RESTART_LEN, n);
            if idx == u64::MAX || sys3(SYS_TASK_RESTART, idx, 0, 0) < 0 {
                uput!(M_ERR, M_ERR_LEN);
            } else {
                uput!(M_OK, M_OK_LEN);
            }
        } else if is_cmd!(bp, n, C_CLEAR, C_CLEAR_LEN) {
            uput!(M_CLEAR, M_CLEAR_LEN);
        } else if is_cmd!(bp, n, C_SHUTDOWN, C_SHUTDOWN_LEN) {
            uput!(M_BYE, M_BYE_LEN);
            sys3(SYS_SHUTDOWN, 0, 0, 0);
        } else {
            uput!(M_UNKNOWN, M_UNKNOWN_LEN);
        }
    }
}
