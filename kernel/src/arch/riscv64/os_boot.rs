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
    self, cap_console, cap_control, cap_device, cap_endpoint, cap_info, Cap, ServiceDef,
    CAP_RIGHT_CONTROL, CAP_RIGHT_FS_LIST, CAP_RIGHT_FS_READ, CAP_RIGHT_RECV, CAP_RIGHT_SEND,
    CAP_RIGHT_STORAGE_INFO, CAP_RIGHT_STORAGE_READ, DEV_RIGHT_DMA_READ, DEV_RIGHT_DMA_WRITE,
    DEV_RIGHT_DRIVER_CONTROL, DEV_RIGHT_INFO, DEV_RIGHT_IRQ_RECEIVE, DEV_RIGHT_MMIO_READ,
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
const SYS_DEVICE_INFO: u64 = 15;
const SYS_MMIO_READ: u64 = 16;
const SYS_MMIO_WRITE: u64 = 17;
const SYS_DMA_READ: u64 = 18;
const SYS_DMA_WRITE: u64 = 19;
const SYS_IRQ_RAISE: u64 = 20;

// Endpoints (docs/25 §5): 1 = console→shell line channel, 2 = fault
// channel, 3 = event channel (as v0.8).
const EP_LINE: u32 = 1;
const EP_FAULT: u32 = 2;
const EP_EVENT: u32 = 3;

/// Shell <-> app_loader request/reply channel (docs/27 §7).
const EP_APP: u32 = 0;
/// Shell <-> fs_service channel (docs/28 §3).
const EP_FS: u32 = 4;
/// Storage channel (docs/29 §4).
const EP_STOR: u32 = 5;
/// Shell <-> driver_manager channel (docs/31 §4).
const EP_DRV: u32 = 6;
/// driver_manager <-> block_driver_service command channel (docs/31 §5).
const EP_BLK: u32 = 7;
/// Driver IRQ event endpoint (docs/31 §9).
const EP_IRQ: u32 = 8;

/// Service-table index of the faulty demo task (`run demo`).
const SVC_FAULTY: u64 = 4;
/// Service-table indexes of the built-in applications (docs/27 §5).
const APP_HELLO: u64 = 6;
const APP_FAULT: u64 = 7;
const APP_COUNTER: u64 = 8;
/// Service-table index of block_driver_service (docs/31 §5).
const TBL_BLOCK_DRIVER: u64 = 12;
/// TCB slot of block_driver_service (sys_task_restart takes slots).
const SLOT_BLOCK_DRIVER: u64 = 13;

// ---------------------------------------------------------------------
// Boot (S-mode)
// ---------------------------------------------------------------------

#[repr(C, align(4096))]
struct Stack([u8; 4096]);
static mut OS_STACKS: [Stack; 14] = [
    Stack([0; 4096]),
    Stack([0; 4096]),
    Stack([0; 4096]),
    Stack([0; 4096]),
    Stack([0; 4096]),
    Stack([0; 4096]),
    Stack([0; 4096]),
    Stack([0; 4096]),
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

const NO_CAPS: [Option<Cap>; dispatch::CAPS_PER_TASK] = [None; dispatch::CAPS_PER_TASK];

/// Service table (docs/25 §3). Entry addresses, stacks, and capability
/// grants are runtime values, patched once by `os_boot` before
/// dispatching; the rest is fixed here.
static mut TABLE: [ServiceDef; 13] = [
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
    ServiceDef {
        name: "app_loader_service",
        entry: 0,
        stack_phys: 0,
        prio: 2,
        slot: 6,
        caps: NO_CAPS,
    },
    ServiceDef {
        name: "hello",
        entry: 0,
        stack_phys: 0,
        prio: 2,
        slot: 7,
        caps: NO_CAPS,
    },
    ServiceDef {
        name: "fault_demo",
        entry: 0,
        stack_phys: 0,
        prio: 2,
        slot: 8,
        caps: NO_CAPS,
    },
    ServiceDef {
        name: "counter",
        entry: 0,
        stack_phys: 0,
        prio: 2,
        slot: 9,
        caps: NO_CAPS,
    },
    ServiceDef {
        name: "fs_service",
        entry: 0,
        stack_phys: 0,
        prio: 2,
        slot: 10,
        caps: NO_CAPS,
    },
    ServiceDef {
        name: "storage_service",
        entry: 0,
        stack_phys: 0,
        prio: 2,
        slot: 11,
        caps: NO_CAPS,
    },
    ServiceDef {
        name: "driver_manager",
        entry: 0,
        stack_phys: 0,
        prio: 2,
        slot: 12,
        caps: NO_CAPS,
    },
    ServiceDef {
        name: "block_driver_service",
        entry: 0,
        stack_phys: 0,
        prio: 2,
        slot: 13,
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
    let table: &'static mut [ServiceDef; 13] = unsafe { &mut *addr_of_mut!(TABLE) };
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
    table[3].caps[4] = Some(cap_endpoint(EP_APP, CAP_RIGHT_SEND | CAP_RIGHT_RECV));
    // Only the shell holds filesystem access (docs/28 §7).
    table[3].caps[5] = Some(cap_endpoint(
        EP_FS,
        CAP_RIGHT_SEND | CAP_RIGHT_RECV | CAP_RIGHT_FS_READ | CAP_RIGHT_FS_LIST,
    ));
    // Storage authority: shell only among operators (docs/29 §5).
    table[3].caps[6] = Some(cap_endpoint(
        EP_STOR,
        CAP_RIGHT_SEND | CAP_RIGHT_RECV | CAP_RIGHT_STORAGE_INFO | CAP_RIGHT_STORAGE_READ,
    ));
    // Driver-manager channel (docs/31 §4): the shell's ONLY path to
    // drivers is forwarding text lines to driver_manager — it holds no
    // device capability and can never reach MMIO. The shell's table is
    // now full: 8/8 of CAPS_PER_TASK (line, console, info, control,
    // app, fs, storage, driver-manager) — checked by the driver test.
    table[3].caps[7] = Some(cap_endpoint(EP_DRV, CAP_RIGHT_SEND | CAP_RIGHT_RECV));
    table[4].entry = faulty_body as *const () as u64;
    table[4].stack_phys = stack_phys(5);
    // faulty_task: no capabilities at all (its IPC attempt is denied).
    // App loader: owns app policy; app request channel + task control
    // to request starts (docs/27 §8).
    table[5].entry = app_loader_body as *const () as u64;
    table[5].stack_phys = stack_phys(6);
    table[5].caps[0] = Some(cap_endpoint(EP_APP, CAP_RIGHT_RECV | CAP_RIGHT_SEND));
    table[5].caps[1] = Some(cap_control());
    // Applications (docs/27 §5): manifest capability grants only.
    table[6].entry = hello_body as *const () as u64;
    table[6].stack_phys = stack_phys(7);
    table[6].caps[0] = Some(cap_console(CAP_RIGHT_SEND));
    table[7].entry = fault_demo_body as *const () as u64;
    table[7].stack_phys = stack_phys(8);
    // fault_demo: deliberately zero capabilities.
    table[8].entry = counter_body as *const () as u64;
    table[8].stack_phys = stack_phys(9);
    table[8].caps[0] = Some(cap_console(CAP_RIGHT_SEND));
    // fs_service (docs/28): filesystem channel only; no other authority.
    table[9].entry = fs_body as *const () as u64;
    table[9].stack_phys = stack_phys(10);
    table[9].caps[0] = Some(cap_endpoint(EP_FS, CAP_RIGHT_RECV | CAP_RIGHT_SEND));
    table[9].caps[1] = Some(cap_endpoint(
        EP_STOR,
        CAP_RIGHT_SEND | CAP_RIGHT_RECV | CAP_RIGHT_STORAGE_READ,
    ));
    // storage_service (docs/29): storage channel only.
    table[10].entry = storage_body as *const () as u64;
    table[10].stack_phys = stack_phys(11);
    table[10].caps[0] = Some(cap_endpoint(EP_STOR, CAP_RIGHT_RECV | CAP_RIGHT_SEND));
    // driver_manager (docs/31 §4): shell channel, driver command
    // channel, task control (start/restart drivers), console (driver
    // lifecycle evidence lines), and driver_control on block0 — the
    // synthetic IRQ injection / liveness probe. Deliberately NO mmio,
    // dma, or irq_receive rights (docs/31 §10).
    table[11].entry = driver_manager_body as *const () as u64;
    table[11].stack_phys = stack_phys(12);
    table[11].caps[0] = Some(cap_endpoint(EP_DRV, CAP_RIGHT_RECV | CAP_RIGHT_SEND));
    table[11].caps[1] = Some(cap_endpoint(EP_BLK, CAP_RIGHT_SEND | CAP_RIGHT_RECV));
    table[11].caps[2] = Some(cap_control());
    table[11].caps[3] = Some(cap_console(CAP_RIGHT_SEND));
    table[11].caps[4] = Some(cap_device(0, DEV_RIGHT_DRIVER_CONTROL));
    // block_driver_service (docs/31 §5): command channel, device
    // capability (info + mmio_read + dma r/w + irq_receive — NOT
    // mmio_write, NOT driver_control: its one register-write attempt
    // is denied on purpose), and the IRQ event endpoint.
    table[12].entry = block_driver_body as *const () as u64;
    table[12].stack_phys = stack_phys(13);
    table[12].caps[0] = Some(cap_endpoint(EP_BLK, CAP_RIGHT_RECV | CAP_RIGHT_SEND));
    table[12].caps[1] = Some(cap_device(
        0,
        DEV_RIGHT_INFO
            | DEV_RIGHT_MMIO_READ
            | DEV_RIGHT_DMA_READ
            | DEV_RIGHT_DMA_WRITE
            | DEV_RIGHT_IRQ_RECEIVE,
    ));
    table[12].caps[2] = Some(cap_endpoint(EP_IRQ, CAP_RIGHT_RECV));

    // SAFETY: boot-time, single hart, called once.
    unsafe { dispatch::set_service_table(table) };

    // Device objects and their IRQ routes (docs/31 §6; AXIOM-DRV-002).
    dispatch::register_devices();

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

/// Four-argument syscall (MMIO/DMA writes carry a value in a3).
#[link_section = ".user.text"]
#[inline(never)]
fn sys4(num: u64, a0: u64, a1: u64, a2: u64, a3: u64) -> i64 {
    let ret: i64;
    // SAFETY: U-mode ecall; the kernel validates every argument.
    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") a0 => ret,
            in("a1") a1,
            in("a2") a2,
            in("a3") a3,
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
    // Applications are policy of the app loader; init only starts it
    // (table index 5). Index 4 (faulty demo) stays shell-on-demand.
    sys3(SYS_TASK_START, 5, 0, 0);
    // Filesystem service (table index 9, docs/28).
    sys3(SYS_TASK_START, 9, 0, 0);
    // Storage service (table index 10, docs/29).
    sys3(SYS_TASK_START, 10, 0, 0);
    // Driver manager (table index 11, docs/31 §4); starting drivers is
    // its policy, not init's.
    sys3(SYS_TASK_START, 11, 0, 0);
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
// app_loader_service (U-mode): owns app policy (AXIOM-APP-003)
// ---------------------------------------------------------------------

umsg!(A_LIST, A_LIST_LEN, b"apps: hello fault_demo counter");
umsg!(A_HELLO, A_HELLO_LEN, b"hello");
umsg!(A_FAULT, A_FAULT_LEN, b"fault_demo");
umsg!(A_COUNTER, A_COUNTER_LEN, b"counter");
umsg!(
    AI_HELLO,
    AI_HELLO_LEN,
    b"hello: greeter prio=2 caps=console restart=rerun"
);
umsg!(
    AI_FAULT,
    AI_FAULT_LEN,
    b"fault_demo: containment demo prio=2 caps=none"
);
umsg!(
    AI_COUNTER,
    AI_COUNTER_LEN,
    b"counter: progress demo prio=2 caps=console"
);
umsg!(A_STARTED, A_STARTED_LEN, b"started");
umsg!(A_ERR, A_ERR_LEN, b"error: cannot start");
umsg!(A_UNKNOWN, A_UNKNOWN_LEN, b"unknown app");
umsg!(A_BADCMD, A_BADCMD_LEN, b"unknown app command");

/// One bounded reply to the shell over the app channel.
#[link_section = ".user.text"]
#[inline(never)]
fn app_reply(p: *const u8, len: usize) {
    sys3(SYS_SEND, 0, p as u64, len as u64);
}

/// App loader main loop: recv one raw shell line, apply app policy,
/// reply one line (docs/27 §7/§8). The kernel never parses app names.
#[link_section = ".user.text"]
extern "C" fn app_loader_body() -> ! {
    let mut buf = MaybeUninit::<[u8; 64]>::uninit();
    let bp = addr_of_mut!(buf) as *mut u8;
    loop {
        let r = sys3(SYS_RECV, 0, bp as u64, 64);
        if r <= 0 {
            continue;
        }
        let n = r as usize;
        if eqs(bp, n, addr_of!(C_APPS) as *const u8, C_APPS_LEN) {
            app_reply(addr_of!(A_LIST) as *const u8, A_LIST_LEN);
        } else if starts_with(bp, n, addr_of!(C_APPINFO) as *const u8, C_APPINFO_LEN) {
            let p = unsafe { bp.add(C_APPINFO_LEN) } as *const u8;
            let m = n - C_APPINFO_LEN;
            if eqs(p, m, addr_of!(A_HELLO) as *const u8, A_HELLO_LEN) {
                app_reply(addr_of!(AI_HELLO) as *const u8, AI_HELLO_LEN);
            } else if eqs(p, m, addr_of!(A_FAULT) as *const u8, A_FAULT_LEN) {
                app_reply(addr_of!(AI_FAULT) as *const u8, AI_FAULT_LEN);
            } else if eqs(p, m, addr_of!(A_COUNTER) as *const u8, A_COUNTER_LEN) {
                app_reply(addr_of!(AI_COUNTER) as *const u8, AI_COUNTER_LEN);
            } else {
                app_reply(addr_of!(A_UNKNOWN) as *const u8, A_UNKNOWN_LEN);
            }
        } else if starts_with(bp, n, addr_of!(C_RUNSP) as *const u8, C_RUNSP_LEN) {
            let p = unsafe { bp.add(C_RUNSP_LEN) } as *const u8;
            let m = n - C_RUNSP_LEN;
            let idx: u64 = if eqs(p, m, addr_of!(A_HELLO) as *const u8, A_HELLO_LEN) {
                APP_HELLO
            } else if eqs(p, m, addr_of!(A_FAULT) as *const u8, A_FAULT_LEN) {
                APP_FAULT
            } else if eqs(p, m, addr_of!(A_COUNTER) as *const u8, A_COUNTER_LEN) {
                APP_COUNTER
            } else {
                u64::MAX
            };
            if idx == u64::MAX {
                app_reply(addr_of!(A_UNKNOWN) as *const u8, A_UNKNOWN_LEN);
            } else if sys3(SYS_TASK_START, idx, 0, 0) < 0 {
                app_reply(addr_of!(A_ERR) as *const u8, A_ERR_LEN);
            } else {
                app_reply(addr_of!(A_STARTED) as *const u8, A_STARTED_LEN);
            }
        } else {
            app_reply(addr_of!(A_BADCMD) as *const u8, A_BADCMD_LEN);
        }
    }
}

// ---------------------------------------------------------------------
// Built-in applications (AXIOM-APP-005/006/007, docs/27 §5)
// ---------------------------------------------------------------------

umsg!(M_HELLO_OUT, M_HELLO_OUT_LEN, b"hello from app: hello\n");
umsg!(M_CNT_PRE, M_CNT_PRE_LEN, b"APP counter progress=");
umsg!(M_CNT_DONE, M_CNT_DONE_LEN, b"APP counter done\n");

/// hello: print one line through its console capability, exit cleanly.
#[link_section = ".user.text"]
extern "C" fn hello_body() -> ! {
    uput!(M_HELLO_OUT, M_HELLO_OUT_LEN);
    sys3(SYS_EXIT, 0, 0, 0);
    loop {
        sys3(SYS_YIELD, 0, 0, 0);
    }
}

/// counter: three progress events with yields between, clean exit.
#[link_section = ".user.text"]
extern "C" fn counter_body() -> ! {
    let mut d = MaybeUninit::<[u8; 2]>::uninit();
    let dp = addr_of_mut!(d) as *mut u8;
    let mut i: u8 = 1;
    while i <= 3 {
        uput!(M_CNT_PRE, M_CNT_PRE_LEN);
        // SAFETY: two-byte stack buffer.
        unsafe {
            write_volatile(dp, b'0' + i);
            write_volatile(dp.add(1), b'\n');
        }
        uwrite_ptr(dp, 2);
        sys3(SYS_YIELD, 0, 0, 0);
        i += 1;
    }
    uput!(M_CNT_DONE, M_CNT_DONE_LEN);
    sys3(SYS_EXIT, 0, 0, 0);
    loop {
        sys3(SYS_YIELD, 0, 0, 0);
    }
}

/// fault_demo: unauthorized device probes and console write (all
/// denied: zero capabilities — MMIO_DENIED / DMA_DENIED / CAP_DENIED
/// evidence, docs/31 §10), then CPU exhaustion; contained by the
/// watchdog, killed by the supervisor, while shell and console keep
/// running.
#[link_section = ".user.text"]
extern "C" fn fault_demo_body() -> ! {
    sys3(SYS_MMIO_READ, 0, 0, 4);
    sys3(SYS_DMA_READ, 0, 0, 1);
    sys3(
        SYS_CON_WRITE,
        addr_of!(M_HELLO_OUT) as *const u8 as u64,
        M_HELLO_OUT_LEN as u64,
        0,
    );
    // SAFETY: intentional infinite loop; never returns.
    unsafe { core::arch::asm!("1:", "j 1b", options(noreturn)) }
}

// ---------------------------------------------------------------------
// fs_service (U-mode): read-only filesystem (AXIOM-FS-002..004)
// ---------------------------------------------------------------------

// Protocol opcodes (docs/28 §3).
umsg!(P_LS, P_LS_LEN, b"LS ");
umsg!(P_CAT, P_CAT_LEN, b"CAT ");
// Paths (docs/28 §5).
umsg!(F_ROOT, F_ROOT_LEN, b"/");
umsg!(F_ETC, F_ETC_LEN, b"/etc");
umsg!(F_APPS, F_APPS_LEN, b"/apps");
umsg!(F_DOCS, F_DOCS_LEN, b"/docs");
umsg!(F_VERSION, F_VERSION_LEN, b"/etc/version");
umsg!(F_LIMITS, F_LIMITS_LEN, b"/etc/limitations");
umsg!(F_MHELLO, F_MHELLO_LEN, b"/apps/hello.manifest");
umsg!(F_MCOUNTER, F_MCOUNTER_LEN, b"/apps/counter.manifest");
umsg!(F_MFAULT, F_MFAULT_LEN, b"/apps/fault_demo.manifest");
umsg!(F_ABOUT, F_ABOUT_LEN, b"/docs/about");
umsg!(F_STORV, F_STORV_LEN, b"/storage/version");
umsg!(Q_BLOCK1, Q_BLOCK1_LEN, b"READ block=1");
// Directory listings and file contents (single bounded reply each).
umsg!(R_ROOT, R_ROOT_LEN, b"OK etc apps docs");
umsg!(R_ETC, R_ETC_LEN, b"OK version limitations");
umsg!(
    R_APPS,
    R_APPS_LEN,
    b"OK hello.manifest counter.manifest fault_demo.manifest"
);
umsg!(R_DOCS, R_DOCS_LEN, b"OK about");
umsg!(
    R_VERSION,
    R_VERSION_LEN,
    b"OK AxiomRT v1.4-storage-service RISC-V 64 evaluation stage"
);
umsg!(
    R_LIMITS,
    R_LIMITS_LEN,
    b"OK emulator-only read-only evaluation build no cert claim"
);
umsg!(
    R_MHELLO,
    R_MHELLO_LEN,
    b"OK hello: prio=2 caps=console restart=rerun"
);
umsg!(
    R_MCOUNTER,
    R_MCOUNTER_LEN,
    b"OK counter: prio=2 caps=console restart=rerun"
);
umsg!(
    R_MFAULT,
    R_MFAULT_LEN,
    b"OK fault_demo: prio=2 caps=none restart=rerun"
);
umsg!(
    R_ABOUT,
    R_ABOUT_LEN,
    b"OK AxiomRT microkernel safety runtime see docs/INDEX.md"
);
umsg!(E_NOTFOUND, E_NOTFOUND_LEN, b"ERR not_found");
umsg!(E_BADPATH, E_BADPATH_LEN, b"ERR bad_path");

/// One bounded reply to the shell over the fs channel.
#[link_section = ".user.text"]
#[inline(never)]
fn fs_reply(p: *const u8, len: usize) {
    sys3(SYS_SEND, 0, p as u64, len as u64);
}

/// fs_service main loop (docs/28 §3): parse one bounded request,
/// answer one bounded reply; every malformed input gets an ERR reply.
#[link_section = ".user.text"]
extern "C" fn fs_body() -> ! {
    let mut buf = MaybeUninit::<[u8; 64]>::uninit();
    let bp = addr_of_mut!(buf) as *mut u8;
    loop {
        let r = sys3(SYS_RECV, 0, bp as u64, 64);
        if r <= 0 {
            continue;
        }
        let n = r as usize;
        if starts_with(bp, n, addr_of!(P_LS) as *const u8, P_LS_LEN) {
            let p = unsafe { bp.add(P_LS_LEN) } as *const u8;
            let m = n - P_LS_LEN;
            if eqs(p, m, addr_of!(F_ROOT) as *const u8, F_ROOT_LEN) {
                fs_reply(addr_of!(R_ROOT) as *const u8, R_ROOT_LEN);
            } else if eqs(p, m, addr_of!(F_ETC) as *const u8, F_ETC_LEN) {
                fs_reply(addr_of!(R_ETC) as *const u8, R_ETC_LEN);
            } else if eqs(p, m, addr_of!(F_APPS) as *const u8, F_APPS_LEN) {
                fs_reply(addr_of!(R_APPS) as *const u8, R_APPS_LEN);
            } else if eqs(p, m, addr_of!(F_DOCS) as *const u8, F_DOCS_LEN) {
                fs_reply(addr_of!(R_DOCS) as *const u8, R_DOCS_LEN);
            } else {
                fs_reply(addr_of!(E_NOTFOUND) as *const u8, E_NOTFOUND_LEN);
            }
        } else if starts_with(bp, n, addr_of!(P_CAT) as *const u8, P_CAT_LEN) {
            let p = unsafe { bp.add(P_CAT_LEN) } as *const u8;
            let m = n - P_CAT_LEN;
            if eqs(p, m, addr_of!(F_VERSION) as *const u8, F_VERSION_LEN) {
                fs_reply(addr_of!(R_VERSION) as *const u8, R_VERSION_LEN);
            } else if eqs(p, m, addr_of!(F_LIMITS) as *const u8, F_LIMITS_LEN) {
                fs_reply(addr_of!(R_LIMITS) as *const u8, R_LIMITS_LEN);
            } else if eqs(p, m, addr_of!(F_MHELLO) as *const u8, F_MHELLO_LEN) {
                fs_reply(addr_of!(R_MHELLO) as *const u8, R_MHELLO_LEN);
            } else if eqs(p, m, addr_of!(F_MCOUNTER) as *const u8, F_MCOUNTER_LEN) {
                fs_reply(addr_of!(R_MCOUNTER) as *const u8, R_MCOUNTER_LEN);
            } else if eqs(p, m, addr_of!(F_MFAULT) as *const u8, F_MFAULT_LEN) {
                fs_reply(addr_of!(R_MFAULT) as *const u8, R_MFAULT_LEN);
            } else if eqs(p, m, addr_of!(F_ABOUT) as *const u8, F_ABOUT_LEN) {
                fs_reply(addr_of!(R_ABOUT) as *const u8, R_ABOUT_LEN);
            } else if eqs(p, m, addr_of!(F_STORV) as *const u8, F_STORV_LEN) {
                // Storage-backed path (docs/29 §7): nested synchronous
                // IPC on the fs service's own storage capability.
                let mut sb = MaybeUninit::<[u8; 64]>::uninit();
                let sp = addr_of_mut!(sb) as *mut u8;
                if sys3(SYS_SEND, 1, addr_of!(Q_BLOCK1) as u64, Q_BLOCK1_LEN as u64) < 0 {
                    fs_reply(addr_of!(E_NOTFOUND) as *const u8, E_NOTFOUND_LEN);
                } else {
                    let sr = sys3(SYS_RECV, 1, sp as u64, 64);
                    if sr > 0 {
                        fs_reply(sp as *const u8, sr as usize);
                    } else {
                        fs_reply(addr_of!(E_NOTFOUND) as *const u8, E_NOTFOUND_LEN);
                    }
                }
            } else {
                fs_reply(addr_of!(E_NOTFOUND) as *const u8, E_NOTFOUND_LEN);
            }
        } else {
            fs_reply(addr_of!(E_BADPATH) as *const u8, E_BADPATH_LEN);
        }
    }
}

// ---------------------------------------------------------------------
// storage_service (U-mode): read-only block image (AXIOM-STOR-002..004)
// ---------------------------------------------------------------------

// Protocol (docs/29 §4).
umsg!(S_INFO, S_INFO_LEN, b"INFO");
umsg!(S_READ, S_READ_LEN, b"READ block=");
umsg!(S_RANGE, S_RANGE_LEN, b"READ_RANGE start=");
umsg!(S_COUNT, S_COUNT_LEN, b" count=");
umsg!(
    S_R_INFO,
    S_R_INFO_LEN,
    b"OK block_size=48 blocks=8 readonly=true"
);
umsg!(S_R_DATA, S_R_DATA_LEN, b"OK data=");
umsg!(S_E_BLOCK, S_E_BLOCK_LEN, b"ERR bad_block");
umsg!(S_E_MANY, S_E_MANY_LEN, b"ERR too_many_blocks");
umsg!(S_E_MAL, S_E_MAL_LEN, b"ERR malformed");
// Block image (docs/29 §8): 8 x 48-byte read-only blocks.
umsg!(B0, B0_LEN, b"AXSTOR v1 blocks=8 bs=48 ro=1");
umsg!(B1, B1_LEN, b"AxiomRT v1.4-storage-service evaluation stage");
umsg!(B2, B2_LEN, b"AxiomRT microkernel safety runtime");
umsg!(B3, B3_LEN, b"apps: hello counter fault_demo prio=2");
umsg!(BRES, BRES_LEN, b"reserved");

/// Answer one READ: branch chain with a call per arm — a
/// value-returning match here becomes an LLVM lookup table in kernel
/// .rodata, which U-mode must never reference (docs/25 §2; found live
/// when storage_service page-faulted on 0xfb58 and was contained).
#[link_section = ".user.text"]
#[inline(never)]
fn block_reply(qp: *mut u8, n: u64) {
    if n == 0 {
        storage_send_block(qp, addr_of!(B0) as *const u8, B0_LEN);
    } else if n == 1 {
        storage_send_block(qp, addr_of!(B1) as *const u8, B1_LEN);
    } else if n == 2 {
        storage_send_block(qp, addr_of!(B2) as *const u8, B2_LEN);
    } else if n == 3 {
        storage_send_block(qp, addr_of!(B3) as *const u8, B3_LEN);
    } else if n <= 7 {
        storage_send_block(qp, addr_of!(BRES) as *const u8, BRES_LEN);
    } else {
        sys3(
            SYS_SEND,
            0,
            addr_of!(S_E_BLOCK) as u64,
            S_E_BLOCK_LEN as u64,
        );
    }
}

/// Parse decimal digits from `from`; returns (value, index after the
/// digits) or u64::MAX when no digit is present. Bounded. Manual range
/// check: no core calls in U-mode (file header rules).
#[allow(clippy::manual_range_contains)]
#[link_section = ".user.text"]
#[inline(never)]
fn parse_dec_stop(p: *const u8, from: usize, n: usize) -> (u64, usize) {
    let mut v: u64 = 0;
    let mut i = from;
    let mut any = false;
    while i < n {
        // SAFETY: in-bounds read.
        let b = unsafe { read_volatile(p.add(i)) };
        if b < b'0' || b > b'9' {
            break;
        }
        v = v.wrapping_mul(10).wrapping_add((b - b'0') as u64);
        i += 1;
        any = true;
    }
    if any {
        (v, i)
    } else {
        (u64::MAX, i)
    }
}

/// Send one `OK data=<block>` reply assembled in a stack buffer.
#[link_section = ".user.text"]
#[inline(never)]
fn storage_send_block(qp: *mut u8, bp: *const u8, blen: usize) {
    let hdr = addr_of!(S_R_DATA) as *const u8;
    let mut q = 0usize;
    while q < S_R_DATA_LEN {
        // SAFETY: bounded copy into the 64-byte reply buffer.
        unsafe { write_volatile(qp.add(q), read_volatile(hdr.add(q))) };
        q += 1;
    }
    let mut i = 0usize;
    while i < blen && q < 64 {
        // SAFETY: bounded copy (48-byte block max, 8-byte prefix).
        unsafe { write_volatile(qp.add(q), read_volatile(bp.add(i))) };
        i += 1;
        q += 1;
    }
    sys3(SYS_SEND, 0, qp as u64, q as u64);
}

/// storage_service main loop (docs/29 §3/§4): one bounded request in,
/// one bounded reply out; malformed input always answers ERR.
#[link_section = ".user.text"]
extern "C" fn storage_body() -> ! {
    let mut buf = MaybeUninit::<[u8; 64]>::uninit();
    let bp = addr_of_mut!(buf) as *mut u8;
    let mut rep = MaybeUninit::<[u8; 64]>::uninit();
    let qp = addr_of_mut!(rep) as *mut u8;
    loop {
        let r = sys3(SYS_RECV, 0, bp as u64, 64);
        if r <= 0 {
            continue;
        }
        let n = r as usize;
        if eqs(bp, n, addr_of!(S_INFO) as *const u8, S_INFO_LEN) {
            sys3(SYS_SEND, 0, addr_of!(S_R_INFO) as u64, S_R_INFO_LEN as u64);
        } else if starts_with(bp, n, addr_of!(S_READ) as *const u8, S_READ_LEN) {
            let (blk, end) = parse_dec_stop(bp, S_READ_LEN, n);
            if blk == u64::MAX || end != n {
                sys3(SYS_SEND, 0, addr_of!(S_E_MAL) as u64, S_E_MAL_LEN as u64);
            } else {
                block_reply(qp, blk);
            }
        } else if starts_with(bp, n, addr_of!(S_RANGE) as *const u8, S_RANGE_LEN) {
            // READ_RANGE start=<n> count=<m>: single-block only (docs/29 §4).
            let (start, i) = parse_dec_stop(bp, S_RANGE_LEN, n);
            let after = i + S_COUNT_LEN;
            if start == u64::MAX
                || after > n
                || !eqs(
                    unsafe { bp.add(i) } as *const u8,
                    S_COUNT_LEN,
                    addr_of!(S_COUNT) as *const u8,
                    S_COUNT_LEN,
                )
            {
                sys3(SYS_SEND, 0, addr_of!(S_E_MAL) as u64, S_E_MAL_LEN as u64);
            } else {
                let (count, end) = parse_dec_stop(bp, after, n);
                if count == u64::MAX || end != n {
                    sys3(SYS_SEND, 0, addr_of!(S_E_MAL) as u64, S_E_MAL_LEN as u64);
                } else if count != 1 {
                    sys3(SYS_SEND, 0, addr_of!(S_E_MANY) as u64, S_E_MANY_LEN as u64);
                } else {
                    block_reply(qp, start);
                }
            }
        } else {
            sys3(SYS_SEND, 0, addr_of!(S_E_MAL) as u64, S_E_MAL_LEN as u64);
        }
    }
}

// ---------------------------------------------------------------------
// block_driver_service (U-mode): driver skeleton (AXIOM-DRV-007)
// ---------------------------------------------------------------------

umsg!(
    BD_STATUS,
    BD_STATUS_LEN,
    b"kind=block_skeleton state=running mmio=granted irq=registered"
);
umsg!(BD_ERR, BD_ERR_LEN, b"ERR unknown_driver_command");
umsg!(BDC_STATUS, BDC_STATUS_LEN, b"STATUS");
umsg!(BDC_FAULT, BDC_FAULT_LEN, b"FAULT");

/// block_driver_service main loop (docs/31 §5): a skeleton, not a
/// virtio driver. Its start sequence exercises every granted device
/// mechanism exactly once — device_info, one real MMIO read of the
/// virtio magic register, one *denied* MMIO write (the right is
/// withheld on purpose), a DMA byte written and read back, then a
/// blocking wait for the synthetic boot attention event on its IRQ
/// endpoint. After that it serves bounded STATUS/FAULT commands from
/// driver_manager; FAULT dereferences an unmapped address for the
/// containment test. Restart re-runs the whole sequence.
#[link_section = ".user.text"]
extern "C" fn block_driver_body() -> ! {
    let mut ib = MaybeUninit::<[u8; 192]>::uninit();
    let ip = addr_of_mut!(ib) as *mut u8;
    let mut cb = MaybeUninit::<[u8; 64]>::uninit();
    let cp = addr_of_mut!(cb) as *mut u8;
    sys3(SYS_DEVICE_INFO, 1, ip as u64, 192);
    sys3(SYS_MMIO_READ, 1, 0, 4);
    sys4(SYS_MMIO_WRITE, 1, 0, 4, 0);
    sys4(SYS_DMA_WRITE, 1, 0, 1, 0x41);
    sys3(SYS_DMA_READ, 1, 0, 1);
    sys3(SYS_RECV, 2, cp as u64, 1);
    loop {
        let r = sys3(SYS_RECV, 0, cp as u64, 64);
        if r <= 0 {
            continue;
        }
        let n = r as usize;
        if eqs(cp, n, addr_of!(BDC_STATUS) as *const u8, BDC_STATUS_LEN) {
            sys3(
                SYS_SEND,
                0,
                addr_of!(BD_STATUS) as u64,
                BD_STATUS_LEN as u64,
            );
        } else if eqs(cp, n, addr_of!(BDC_FAULT) as *const u8, BDC_FAULT_LEN) {
            // Deliberate contained fault (docs/31 §5 test mode): the
            // store below hits an unmapped page.
            // SAFETY: intentionally invalid; the kernel contains it.
            unsafe { write_volatile(0x4000_0000 as *mut u8, 1) };
            // If containment ever failed to trigger, exhaust the CPU so
            // the watchdog contains us instead.
            // SAFETY: intentional infinite loop; never returns.
            unsafe { core::arch::asm!("1:", "j 1b", options(noreturn)) }
        } else {
            sys3(SYS_SEND, 0, addr_of!(BD_ERR) as u64, BD_ERR_LEN as u64);
        }
    }
}

// ---------------------------------------------------------------------
// driver_manager (U-mode): driver lifecycle policy (AXIOM-DRV-006)
// ---------------------------------------------------------------------

// Driver lifecycle evidence (console) and shell replies (IPC ≤ 64 B).
umsg!(
    DM_STARTED,
    DM_STARTED_LEN,
    b"DRIVER started=block_driver_service\n"
);
umsg!(
    DM_STARTFAIL,
    DM_STARTFAIL_LEN,
    b"DRIVER start_failed=block_driver_service\n"
);
umsg!(
    DM_RESTARTED,
    DM_RESTARTED_LEN,
    b"DRIVER restarted=block_driver_service\n"
);
umsg!(
    DM_OBSERVED,
    DM_OBSERVED_LEN,
    b"DRIVER_MANAGER observed=fault driver=block_driver_service\n"
);
umsg!(
    DM_LINE_RUN,
    DM_LINE_RUN_LEN,
    b"driver name=block_driver_service state=running kind=block_skeleton\n"
);
umsg!(
    DM_LINE_FLT,
    DM_LINE_FLT_LEN,
    b"driver name=block_driver_service state=faulted kind=block_skeleton\n"
);
umsg!(
    DM_LINE_STOP,
    DM_LINE_STOP_LEN,
    b"driver name=block_driver_service state=stopped kind=block_skeleton\n"
);
umsg!(DMR_RUN, DMR_RUN_LEN, b"block_driver_service running");
umsg!(DMR_FLT, DMR_FLT_LEN, b"block_driver_service faulted");
umsg!(DMR_STOP, DMR_STOP_LEN, b"block_driver_service stopped");
umsg!(
    DMR_INFO_FLT,
    DMR_INFO_FLT_LEN,
    b"kind=block_skeleton state=faulted"
);
umsg!(
    DMR_INFO_STOP,
    DMR_INFO_STOP_LEN,
    b"kind=block_skeleton state=stopped"
);
umsg!(DMR_RESTARTED, DMR_RESTARTED_LEN, b"restarted");
umsg!(
    DMR_RESTART_ERR,
    DMR_RESTART_ERR_LEN,
    b"error: cannot restart"
);
umsg!(DMR_FAULTED, DMR_FAULTED_LEN, b"faulted (contained)");
umsg!(DMR_FAULTREQ, DMR_FAULTREQ_LEN, b"fault requested");
umsg!(DMR_NOTRUN, DMR_NOTRUN_LEN, b"driver not running");
umsg!(DMR_BADCMD, DMR_BADCMD_LEN, b"unknown driver command");
// Driver command protocol (docs/31 §5) and shell lines (docs/31 §4).
umsg!(DM_Q_STATUS, DM_Q_STATUS_LEN, b"STATUS");
umsg!(DM_Q_FAULT, DM_Q_FAULT_LEN, b"FAULT");
umsg!(DMC_LIST, DMC_LIST_LEN, b"drivers");
umsg!(DMC_INFO, DMC_INFO_LEN, b"driver info block");
umsg!(DMC_RESTART, DMC_RESTART_LEN, b"driver restart block");
umsg!(DMC_FAULT, DMC_FAULT_LEN, b"driver fault block");

/// One bounded reply to the shell over the driver-manager channel.
#[link_section = ".user.text"]
#[inline(never)]
fn dm_reply(p: *const u8, len: usize) {
    sys3(SYS_SEND, 0, p as u64, len as u64);
}

/// `drivers`: print the full state line (console) and reply the short
/// one (IPC). Branch chain with a call per arm (docs/25 §2 rules).
#[link_section = ".user.text"]
#[inline(never)]
fn dm_list(st: u8) {
    if st == 1 {
        uput!(DM_LINE_RUN, DM_LINE_RUN_LEN);
        dm_reply(addr_of!(DMR_RUN) as *const u8, DMR_RUN_LEN);
    } else if st == 2 {
        uput!(DM_LINE_FLT, DM_LINE_FLT_LEN);
        dm_reply(addr_of!(DMR_FLT) as *const u8, DMR_FLT_LEN);
    } else {
        uput!(DM_LINE_STOP, DM_LINE_STOP_LEN);
        dm_reply(addr_of!(DMR_STOP) as *const u8, DMR_STOP_LEN);
    }
}

/// `driver info block`: a running driver answers for itself (nested
/// STATUS query over EP_BLK, reply forwarded verbatim); a dead or
/// stopped driver is answered from the manager's tracked state — the
/// manager never IPCs a driver it believes dead (docs/31 §4).
#[link_section = ".user.text"]
#[inline(never)]
fn dm_info(st: u8, sp: *mut u8) {
    if st == 1 {
        if sys3(
            SYS_SEND,
            1,
            addr_of!(DM_Q_STATUS) as u64,
            DM_Q_STATUS_LEN as u64,
        ) < 0
        {
            dm_reply(addr_of!(DMR_INFO_STOP) as *const u8, DMR_INFO_STOP_LEN);
            return;
        }
        let sr = sys3(SYS_RECV, 1, sp as u64, 64);
        if sr > 0 {
            dm_reply(sp as *const u8, sr as usize);
        } else {
            dm_reply(addr_of!(DMR_INFO_STOP) as *const u8, DMR_INFO_STOP_LEN);
        }
    } else if st == 2 {
        dm_reply(addr_of!(DMR_INFO_FLT) as *const u8, DMR_INFO_FLT_LEN);
    } else {
        dm_reply(addr_of!(DMR_INFO_STOP) as *const u8, DMR_INFO_STOP_LEN);
    }
}

/// driver_manager main loop (docs/31 §4): owns driver lifecycle
/// policy. Starts the block driver, tracks its state (0 = stopped,
/// 1 = running, 2 = faulted), answers the shell's driver lines,
/// observes driver death through the synthetic-IRQ liveness probe
/// (docs/31 §9), and requests restarts through its control capability.
/// It never parses device registers or block protocol.
#[link_section = ".user.text"]
extern "C" fn driver_manager_body() -> ! {
    let mut buf = MaybeUninit::<[u8; 64]>::uninit();
    let bp = addr_of_mut!(buf) as *mut u8;
    let mut sbuf = MaybeUninit::<[u8; 64]>::uninit();
    let sp = addr_of_mut!(sbuf) as *mut u8;
    let mut st: u8 = 0;
    if sys3(SYS_TASK_START, TBL_BLOCK_DRIVER, 0, 0) >= 0 {
        st = 1;
        uput!(DM_STARTED, DM_STARTED_LEN);
        // Boot attention event: the synthetic stand-in for the device's
        // first interrupt; the driver consumes it in its start sequence.
        sys3(SYS_IRQ_RAISE, 4, 0, 0);
    } else {
        uput!(DM_STARTFAIL, DM_STARTFAIL_LEN);
    }
    loop {
        let r = sys3(SYS_RECV, 0, bp as u64, 64);
        if r <= 0 {
            continue;
        }
        let n = r as usize;
        if eqs(bp, n, addr_of!(DMC_LIST) as *const u8, DMC_LIST_LEN) {
            dm_list(st);
        } else if eqs(bp, n, addr_of!(DMC_INFO) as *const u8, DMC_INFO_LEN) {
            dm_info(st, sp);
        } else if eqs(bp, n, addr_of!(DMC_RESTART) as *const u8, DMC_RESTART_LEN) {
            if sys3(SYS_TASK_RESTART, SLOT_BLOCK_DRIVER, 0, 0) < 0 {
                dm_reply(addr_of!(DMR_RESTART_ERR) as *const u8, DMR_RESTART_ERR_LEN);
            } else {
                st = 1;
                uput!(DM_RESTARTED, DM_RESTARTED_LEN);
                sys3(SYS_IRQ_RAISE, 4, 0, 0); // re-arm attention event
                dm_reply(addr_of!(DMR_RESTARTED) as *const u8, DMR_RESTARTED_LEN);
            }
        } else if eqs(bp, n, addr_of!(DMC_FAULT) as *const u8, DMC_FAULT_LEN) {
            // Never IPC a driver believed dead (the send would block
            // forever on an endpoint nobody serves); short-circuit
            // guards the FAULT command behind the tracked state.
            if st != 1
                || sys3(
                    SYS_SEND,
                    1,
                    addr_of!(DM_Q_FAULT) as u64,
                    DM_Q_FAULT_LEN as u64,
                ) < 0
            {
                dm_reply(addr_of!(DMR_NOTRUN) as *const u8, DMR_NOTRUN_LEN);
            } else {
                // Bounded liveness probe: a dropped raise (error result)
                // means the driver is dead — the manager's observation
                // of the contained fault (docs/31 §9).
                let mut k: u32 = 0;
                let mut dead = false;
                while k < 16 {
                    sys3(SYS_YIELD, 0, 0, 0);
                    if sys3(SYS_IRQ_RAISE, 4, 0, 0) < 0 {
                        dead = true;
                        break;
                    }
                    k += 1;
                }
                if dead {
                    st = 2;
                    uput!(DM_OBSERVED, DM_OBSERVED_LEN);
                    dm_reply(addr_of!(DMR_FAULTED) as *const u8, DMR_FAULTED_LEN);
                } else {
                    dm_reply(addr_of!(DMR_FAULTREQ) as *const u8, DMR_FAULTREQ_LEN);
                }
            }
        } else {
            dm_reply(addr_of!(DMR_BADCMD) as *const u8, DMR_BADCMD_LEN);
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
    b"commands: help version tasks faults ipc caps memory uptime events\n          run demo | kill <idx> | restart <idx> | clear | shutdown\n          drivers | driver <info|restart|fault> block\n"
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
umsg!(C_DRIVERS, C_DRIVERS_LEN, b"drivers");
umsg!(C_DRIVERSP, C_DRIVERSP_LEN, b"driver ");
umsg!(C_STORI, C_STORI_LEN, b"storage info");
umsg!(C_STORR, C_STORR_LEN, b"storage read ");
umsg!(C_LS, C_LS_LEN, b"ls");
umsg!(C_LSSP, C_LSSP_LEN, b"ls ");
umsg!(C_CATSP, C_CATSP_LEN, b"cat ");
umsg!(C_APPS, C_APPS_LEN, b"apps");
umsg!(C_APPINFO, C_APPINFO_LEN, b"app info ");
umsg!(C_RUNSP, C_RUNSP_LEN, b"run ");
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

/// Forward one raw app command line to the app loader and print its
/// bounded reply (docs/27 §7): the shell holds no app-name knowledge.
#[link_section = ".user.text"]
#[inline(never)]
fn shell_app_forward(bp: *mut u8, n: usize, rp: *mut u8) {
    if sys3(SYS_SEND, 4, bp as u64, n as u64) < 0 {
        uput!(M_ERR, M_ERR_LEN);
        return;
    }
    let r = sys3(SYS_RECV, 4, rp as u64, 64);
    if r > 0 {
        uwrite_ptr(rp, r as usize);
        uput!(M_NL, M_NL_LEN);
    } else {
        uput!(M_ERR, M_ERR_LEN);
    }
}

/// Forward one raw driver command line to driver_manager on the
/// shell's driver capability (slot 7) and print the bounded reply
/// (docs/31 §4): all driver knowledge is manager policy — the shell
/// holds no device capability and can never reach MMIO.
#[link_section = ".user.text"]
#[inline(never)]
fn shell_drv_forward(bp: *mut u8, n: usize, rp: *mut u8) {
    if sys3(SYS_SEND, 7, bp as u64, n as u64) < 0 {
        uput!(M_ERR, M_ERR_LEN);
        return;
    }
    let r = sys3(SYS_RECV, 7, rp as u64, 64);
    if r > 0 {
        uwrite_ptr(rp, r as usize);
        uput!(M_NL, M_NL_LEN);
    } else {
        uput!(M_ERR, M_ERR_LEN);
    }
}

/// Build `LS <path>` / `CAT <path>` in a stack buffer, forward it to
/// fs_service on the shell's fs capability (slot 5), print the reply
/// verbatim (docs/28 §6). `tail_at` = where the path starts in the
/// input line, or n (== `ls` alone -> path `/`).
#[link_section = ".user.text"]
#[inline(never)]
fn shell_fs_cmd(bp: *const u8, n: usize, tail_at: usize, cat: bool, qp: *mut u8, rp: *mut u8) {
    let (pfx, pfx_len): (*const u8, usize) = if cat {
        (addr_of!(P_CAT) as *const u8, P_CAT_LEN)
    } else {
        (addr_of!(P_LS) as *const u8, P_LS_LEN)
    };
    let mut q = 0usize;
    while q < pfx_len {
        // SAFETY: q < pfx_len <= 4, request buffer is 64 bytes.
        unsafe { write_volatile(qp.add(q), read_volatile(pfx.add(q))) };
        q += 1;
    }
    if tail_at >= n {
        // bare `ls` -> `LS /`
        // SAFETY: q < 64.
        unsafe { write_volatile(qp.add(q), b'/') };
        q += 1;
    } else {
        let mut i = tail_at;
        while i < n && q < 63 {
            // SAFETY: bounded copy inside both 64-byte buffers.
            unsafe { write_volatile(qp.add(q), read_volatile(bp.add(i))) };
            i += 1;
            q += 1;
        }
    }
    if sys3(SYS_SEND, 5, qp as u64, q as u64) < 0 {
        uput!(M_ERR, M_ERR_LEN);
        return;
    }
    let r = sys3(SYS_RECV, 5, rp as u64, 64);
    if r > 0 {
        uwrite_ptr(rp, r as usize);
        uput!(M_NL, M_NL_LEN);
    } else {
        uput!(M_ERR, M_ERR_LEN);
    }
}

/// Build `INFO` / `READ block=<n>` and forward it to storage_service
/// on the shell's storage capability (slot 6), printing the reply
/// verbatim (docs/29 §4). `tail_at >= n` means `storage info`.
#[link_section = ".user.text"]
#[inline(never)]
fn shell_stor_cmd(bp: *const u8, n: usize, tail_at: usize, qp: *mut u8, rp: *mut u8) {
    let mut q = 0usize;
    if tail_at >= n {
        let hdr = addr_of!(S_INFO) as *const u8;
        while q < S_INFO_LEN {
            // SAFETY: bounded copy into the 64-byte request buffer.
            unsafe { write_volatile(qp.add(q), read_volatile(hdr.add(q))) };
            q += 1;
        }
    } else {
        let hdr = addr_of!(S_READ) as *const u8;
        while q < S_READ_LEN {
            // SAFETY: bounded copy into the 64-byte request buffer.
            unsafe { write_volatile(qp.add(q), read_volatile(hdr.add(q))) };
            q += 1;
        }
        let mut i = tail_at;
        while i < n && q < 63 {
            // SAFETY: bounded copy inside both 64-byte buffers.
            unsafe { write_volatile(qp.add(q), read_volatile(bp.add(i))) };
            i += 1;
            q += 1;
        }
    }
    if sys3(SYS_SEND, 6, qp as u64, q as u64) < 0 {
        uput!(M_ERR, M_ERR_LEN);
        return;
    }
    let r = sys3(SYS_RECV, 6, rp as u64, 64);
    if r > 0 {
        uwrite_ptr(rp, r as usize);
        uput!(M_NL, M_NL_LEN);
    } else {
        uput!(M_ERR, M_ERR_LEN);
    }
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
    let mut req = MaybeUninit::<[u8; 64]>::uninit();
    let qp = addr_of_mut!(req) as *mut u8;

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
        } else if is_cmd!(bp, n, C_DRIVERS, C_DRIVERS_LEN)
            || starts_with(bp, n, addr_of!(C_DRIVERSP) as *const u8, C_DRIVERSP_LEN)
        {
            // drivers / driver <info|restart|fault> block: every
            // driver line is driver_manager policy (docs/31 §4).
            shell_drv_forward(bp, n, op);
        } else if is_cmd!(bp, n, C_STORI, C_STORI_LEN) {
            shell_stor_cmd(bp, n, n, qp, op); // INFO
        } else if starts_with(bp, n, addr_of!(C_STORR) as *const u8, C_STORR_LEN) {
            shell_stor_cmd(bp, n, C_STORR_LEN, qp, op); // READ block=<n>
        } else if is_cmd!(bp, n, C_LS, C_LS_LEN) {
            shell_fs_cmd(bp, n, n, false, qp, op);
        } else if starts_with(bp, n, addr_of!(C_LSSP) as *const u8, C_LSSP_LEN) {
            shell_fs_cmd(bp, n, C_LSSP_LEN, false, qp, op);
        } else if starts_with(bp, n, addr_of!(C_CATSP) as *const u8, C_CATSP_LEN) {
            shell_fs_cmd(bp, n, C_CATSP_LEN, true, qp, op);
        } else if is_cmd!(bp, n, C_APPS, C_APPS_LEN)
            || starts_with(bp, n, addr_of!(C_APPINFO) as *const u8, C_APPINFO_LEN)
            || starts_with(bp, n, addr_of!(C_RUNSP) as *const u8, C_RUNSP_LEN)
        {
            // "run demo" was matched above; every other apps / app info
            // / run <name> line is loader policy.
            shell_app_forward(bp, n, op);
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
