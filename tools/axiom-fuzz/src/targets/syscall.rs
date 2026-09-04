//! Deterministic adversarial target for the AxiomRT syscall boundary.
//!
//! The target models the on-target syscall classification and argument
//! validation contract (docs/04, docs/25, docs/31, docs/19 §4) and routes
//! MMIO/DMA bounds decisions through the real host validator
//! `kernel::device::access_in_bounds` — the exact function the RISC-V
//! dispatcher calls. Everything else is a documented host model of the
//! private riscv64 dispatcher: it proves validation-order, rejection, and
//! no-mutation-on-reject semantics at the model level, not RISC-V trap,
//! SATP, SUM, or MMU behavior (those remain AXIOM-ROBUST-013).
//!
//! Syscall contract encoded here (resolved by AXIOM-ROBUST-005):
//! * numbers 1-4 and 7-20 are implemented;
//! * numbers 5 (sys_reply) and 6 (sys_cap_query) are ABI-recognized
//!   stubs returning ERR_NOT_IMPLEMENTED (-9), never invalid;
//! * every other number is invalid and returns ERR_INVALID_SYSCALL (-1);
//! * sys_fault_ack requires a fault-channel endpoint capability with the
//!   Control right (docs/04; boot policy mints exactly that right) and is
//!   otherwise record-only (docs/19 §4).

use crate::targets::capability::RUNTIME_CAP_SLOTS;
use crate::{CaseResult, FuzzCase, FuzzTarget, ResultClass};
use kernel::device::access_in_bounds;
use kernel::ipc::MSG_MAX_BYTES;
use std::panic::{catch_unwind, AssertUnwindSafe};

pub const NAME: &str = "syscall";
pub const MAX_SYSCALL_OPS_PER_CASE: usize = 16;

// ---- Runtime bounds mirrored from the authoritative sources ------------
// (dispatch.rs, docs/36 §4). IPC size is imported from the host kernel.

pub const MAX_TASKS: usize = 16;
pub const NUM_ENDPOINTS: usize = 12;
pub const NUM_SERVICES: usize = 15;
pub const NUM_DEVICES: usize = 2;
const IPC_MSG_MAX: usize = MSG_MAX_BYTES;
const CON_WRITE_MAX: usize = 256;
const INFO_MAX: usize = 768;
const USER_DATA_VA: u64 = 0x20_0000;
const USER_DATA_END: u64 = USER_DATA_VA + 0x1000;
const KERNEL_BASE_VA: u64 = 0x8020_0000;
const MMIO0_SIZE: u64 = 0x200;
const DMA0_SIZE: u64 = 4096;
const EP_FAULT: u32 = 2;
const EP_EVENT: u32 = 3;
const IRQ_ENDPOINTS: [u32; NUM_DEVICES] = [8, 10];

// ---- Result codes (docs/04; docs/25 §4 overloads -7 as no-slot) --------

pub const OK: i64 = 0;
pub const ERR_INVALID_SYSCALL: i64 = -1;
pub const ERR_INVALID_CAP: i64 = -2;
pub const ERR_INSUFFICIENT_RIGHTS: i64 = -3;
pub const ERR_WRONG_OBJECT_TYPE: i64 = -4;
pub const ERR_INVALID_ARG: i64 = -5;
pub const ERR_MSG_TOO_LARGE: i64 = -6;
pub const ERR_NO_SLOT: i64 = -7;
pub const ERR_NOT_IMPLEMENTED: i64 = -9;
const IRQ_PENDING: i64 = 1;

// ---- Rights bits (dispatch.rs / docs/06 §2, docs/31 §10) ---------------

const RIGHT_SEND: u16 = 1 << 3;
const RIGHT_RECV: u16 = 1 << 4;
const RIGHT_CONTROL: u16 = 1 << 7;
const DEV_INFO: u16 = 1 << 0;
const DEV_MMIO_READ: u16 = 1 << 1;
const DEV_MMIO_WRITE: u16 = 1 << 2;
const DEV_DMA_READ: u16 = 1 << 3;
const DEV_DMA_WRITE: u16 = 1 << 4;
const DEV_IRQ_RECEIVE: u16 = 1 << 5;
const DEV_DRIVER_CONTROL: u16 = 1 << 6;

// ---- Syscall inventory (SYS-INV-001) -----------------------------------

/// Classification of one ABI number (docs/36 three-class model).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyscallClass {
    Implemented,
    RecognizedNotImplemented,
    Invalid,
}

/// Every recognized ABI number. Adding a runtime syscall without updating
/// this table fails the inventory tests below.
pub const SYSCALL_INVENTORY: [(u64, &str, SyscallClass); 20] = [
    (1, "sys_yield", SyscallClass::Implemented),
    (2, "sys_exit", SyscallClass::Implemented),
    (3, "sys_send", SyscallClass::Implemented),
    (4, "sys_recv", SyscallClass::Implemented),
    (5, "sys_reply", SyscallClass::RecognizedNotImplemented),
    (6, "sys_cap_query", SyscallClass::RecognizedNotImplemented),
    (7, "sys_fault_ack", SyscallClass::Implemented),
    (8, "sys_task_start", SyscallClass::Implemented),
    (9, "sys_con_write", SyscallClass::Implemented),
    (10, "sys_con_read", SyscallClass::Implemented),
    (11, "sys_info", SyscallClass::Implemented),
    (12, "sys_task_kill", SyscallClass::Implemented),
    (13, "sys_task_restart", SyscallClass::Implemented),
    (14, "sys_shutdown", SyscallClass::Implemented),
    (15, "sys_device_info", SyscallClass::Implemented),
    (16, "sys_mmio_read", SyscallClass::Implemented),
    (17, "sys_mmio_write", SyscallClass::Implemented),
    (18, "sys_dma_read", SyscallClass::Implemented),
    (19, "sys_dma_write", SyscallClass::Implemented),
    (20, "sys_irq_raise", SyscallClass::Implemented),
];

/// Deterministic total classification of any 64-bit syscall number.
pub fn classify(number: u64) -> SyscallClass {
    for (candidate, _, class) in SYSCALL_INVENTORY {
        if candidate == number {
            return class;
        }
    }
    SyscallClass::Invalid
}

// ---- Fixed boundary bank -----------------------------------------------

pub const FIXED_SCENARIO_NAMES: [&str; 56] = [
    "invalid_syscall_0",
    "implemented_syscall_1",
    "implemented_syscall_4",
    "stub_syscall_5",
    "stub_syscall_6",
    "implemented_syscall_7",
    "implemented_syscall_20",
    "invalid_syscall_21",
    "invalid_syscall_max",
    "cap_slot_0",
    "cap_slot_8",
    "cap_slot_9",
    "cap_slot_max",
    "cap_slot_empty",
    "cap_slot_revoked",
    "cap_slot_wrong_type",
    "cap_slot_wrong_object",
    "cap_slot_wrong_rights",
    "endpoint_0",
    "endpoint_11",
    "endpoint_12",
    "endpoint_max",
    "task_0",
    "task_15",
    "task_16",
    "task_max",
    "service_first",
    "service_last",
    "service_one_beyond",
    "service_max",
    "mmio_first",
    "mmio_last_valid",
    "mmio_boundary",
    "mmio_misaligned",
    "mmio_width_invalid",
    "mmio_zero_size_device",
    "mmio_device_out_of_range",
    "dma_first",
    "dma_last_valid",
    "dma_boundary",
    "dma_width_invalid",
    "dma_overflow",
    "dma_direction_mismatch",
    "pointer_null",
    "pointer_valid",
    "pointer_kernel",
    "pointer_cross_range",
    "pointer_overflow",
    "lifecycle_start_valid",
    "lifecycle_start_running",
    "lifecycle_kill_repeated",
    "lifecycle_restart_self",
    "fault_ack_authorized",
    "fault_ack_unauthorized",
    "fault_ack_unknown_decision",
    "fault_ack_no_pending_fault",
];

const MANDATORY_SCENARIOS: u64 = FIXED_SCENARIO_NAMES.len() as u64;

// ---- Model task/capability representation ------------------------------

/// Runtime capability object kinds (dispatch.rs OTYPE_*).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ObjKind {
    Endpoint,
    Console,
    Control,
    Info,
    Device,
}

/// One tagged runtime-style capability (9-slot per-task array model).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ModelCap {
    kind: ObjKind,
    object_id: u32,
    rights: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Life {
    Empty,
    Ready,
    Blocked,
    Faulted,
    Killed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ModelTask {
    life: Life,
    caps: [Option<ModelCap>; RUNTIME_CAP_SLOTS],
    pending_delivery: bool,
    /// 0 means the slot was never armed from the service table and can
    /// never be restarted (dispatch.rs sys_task_restart entry_va check).
    entry_va: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EpState {
    Idle,
    SenderWaiting { tid: usize, len: usize },
    ReceiverWaiting { tid: usize, cap: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct IrqRoute {
    /// MAX_TASKS means no registered receiver.
    receiver: usize,
    pending: bool,
}

/// State a rejected syscall must never change (SYS-INV-002/003/014).
/// The kernel event ring is documented denial/recovery *evidence* and is
/// tracked separately, exactly like the runtime CAP_DENIED ring pushes.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Protected {
    tasks: Vec<ModelTask>,
    endpoints: Vec<EpState>,
    irq_routes: Vec<IrqRoute>,
    mmio: Vec<u8>,
    dma: Vec<u8>,
    console_bytes: u64,
    info_reads: u64,
    shutdowns: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct OperationCounts {
    accepted: u64,
    safe_rejects: u64,
    busy: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ModelOutcome {
    protected: Protected,
    evidence_events: u64,
    recovery_acks: u64,
    counts: OperationCounts,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StepDisposition {
    Accepted,
    SafeReject,
    Busy,
}

struct SyscallModel {
    tasks: [ModelTask; MAX_TASKS],
    endpoints: [EpState; NUM_ENDPOINTS],
    irq_routes: [IrqRoute; NUM_DEVICES],
    mmio: [u8; MMIO0_SIZE as usize],
    dma: [u8; DMA0_SIZE as usize],
    console_bytes: u64,
    info_reads: u64,
    shutdowns: u64,
    evidence_events: u64,
    recovery_acks: u64,
    counts: OperationCounts,
}

// ---- Operations decoded from fuzz input --------------------------------

/// A capability profile a fuzz case may mint into a task slot. Minting
/// stands for an (adversarial) trusted boot definition, not for any
/// U-mode authority: the live kernel exposes no mint syscall.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MintProfile {
    EndpointSendRecv0,
    EndpointSendRecv11,
    /// One-beyond endpoint id 12: probes the model guard that no
    /// capability may reach endpoint indexing out of range (SYS-INV-005).
    EndpointOutOfRange12,
    EndpointOutOfRangeMax,
    EndpointZeroRights,
    ConsoleSendRecv,
    Control,
    Info,
    DeviceBlockAll,
    DeviceBlockReadOnly,
    DeviceNet,
    /// One-beyond device id 2 (rejected by the device cap check itself).
    DeviceOutOfRange2,
    DeviceOutOfRangeMax,
    FaultChannelControl,
    FaultChannelRecvOnly,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyscallOp {
    /// Invoke syscall `number` with raw argument registers.
    Invoke {
        caller: usize,
        number: u64,
        args: [u64; 4],
    },
    /// Replace a capability slot (adversarial trusted definition).
    MintCap {
        task: usize,
        slot: usize,
        profile: MintProfile,
    },
    /// Clear a capability slot (models revocation of boot authority).
    RevokeCap { task: usize, slot: usize },
    /// Kernel-side fault containment stand-in (watchdog/page fault).
    FaultTask { task: usize },
}

// ---- Fuzz target -------------------------------------------------------

#[derive(Clone, Copy, Debug, Default)]
pub struct SyscallTarget;

impl FuzzTarget for SyscallTarget {
    fn name(&self) -> &'static str {
        NAME
    }

    fn evaluate(&mut self, case: &FuzzCase) -> CaseResult {
        if case.target != NAME {
            return CaseResult::invariant_failure(
                "syscall target received a case for another target",
            );
        }
        evaluate_operations(&decode_operations(case.iteration, &case.input))
    }
}

pub fn evaluate_operations(operations: &[SyscallOp]) -> CaseResult {
    if operations.len() > MAX_SYSCALL_OPS_PER_CASE {
        return CaseResult::invariant_failure(format!(
            "syscall operation count {} exceeds bound {MAX_SYSCALL_OPS_PER_CASE}",
            operations.len()
        ));
    }

    let first = match run_caught(operations) {
        Ok(outcome) => outcome,
        Err(reason) => return CaseResult::invariant_failure(reason),
    };
    let second = match run_caught(operations) {
        Ok(outcome) => outcome,
        Err(reason) => return CaseResult::invariant_failure(reason),
    };
    if first != second {
        return CaseResult::invariant_failure(
            "SYS-INV-015: identical syscall streams produced different final states",
        );
    }

    if first.counts.busy > 0 {
        CaseResult::new(
            ResultClass::BoundedResourceExhaustion,
            "a documented one-party endpoint or busy lifecycle bound was reached",
        )
    } else {
        CaseResult::safe_reject("syscall sequence completed without an invariant failure")
    }
}

fn run_caught(operations: &[SyscallOp]) -> Result<ModelOutcome, String> {
    match catch_unwind(AssertUnwindSafe(|| run_once(operations))) {
        Ok(result) => result,
        Err(_) => Err(
            "SYS-INV-016: user-controlled syscall input triggered a host-reachable panic"
                .to_string(),
        ),
    }
}

fn run_once(operations: &[SyscallOp]) -> Result<ModelOutcome, String> {
    let mut model = SyscallModel::new();
    for operation in operations {
        let disposition = model.apply(operation)?;
        model.record(disposition);
    }
    Ok(model.outcome())
}

// ---- Deterministic decoding --------------------------------------------

pub fn decode_operations(iteration: u64, input: &[u8]) -> Vec<SyscallOp> {
    let mut operations = mandatory_scenario(iteration % MANDATORY_SCENARIOS);
    for chunk in input.chunks(6) {
        if operations.len() == MAX_SYSCALL_OPS_PER_CASE {
            break;
        }
        let opcode = chunk[0];
        let caller = caller_boundary(chunk.get(1).copied().unwrap_or(0));
        let b2 = chunk.get(2).copied().unwrap_or(0);
        let b3 = chunk.get(3).copied().unwrap_or(opcode);
        let b4 = chunk.get(4).copied().unwrap_or(b2);
        let b5 = chunk.get(5).copied().unwrap_or(b3);

        let operation = match opcode % 18 {
            0 => SyscallOp::Invoke {
                caller,
                number: number_boundary(b2),
                args: [
                    scalar_boundary(b3),
                    scalar_boundary(b4),
                    scalar_boundary(b5),
                    scalar_boundary(b2 ^ b5),
                ],
            },
            1 => SyscallOp::Invoke {
                caller,
                number: 3,
                args: [cap_slot_boundary(b2), va_boundary(b3), len_boundary(b4), 0],
            },
            2 => SyscallOp::Invoke {
                caller,
                number: 4,
                args: [cap_slot_boundary(b2), va_boundary(b3), len_boundary(b4), 0],
            },
            3 => SyscallOp::Invoke {
                caller,
                number: 7,
                args: [0, decision_boundary(b2), 0, 0],
            },
            4 => SyscallOp::Invoke {
                caller,
                number: 8,
                args: [service_boundary(b2), 0, 0, 0],
            },
            5 => SyscallOp::Invoke {
                caller,
                number: 9,
                args: [va_boundary(b2), len_boundary(b3), 0, 0],
            },
            6 => SyscallOp::Invoke {
                caller,
                number: 10,
                args: [va_boundary(b2), len_boundary(b3), 0, 0],
            },
            7 => SyscallOp::Invoke {
                caller,
                number: 11,
                args: [
                    u64::from(b2 % 10),
                    va_boundary(b3),
                    len_boundary(b4),
                    task_boundary(b5),
                ],
            },
            8 => SyscallOp::Invoke {
                caller,
                number: 12,
                args: [task_boundary(b2), 0, 0, 0],
            },
            9 => SyscallOp::Invoke {
                caller,
                number: 13,
                args: [task_boundary(b2), 0, 0, 0],
            },
            10 => SyscallOp::Invoke {
                caller,
                number: 15,
                args: [cap_slot_boundary(b2), va_boundary(b3), len_boundary(b4), 0],
            },
            11 => SyscallOp::Invoke {
                caller,
                number: if b5 & 1 == 0 { 16 } else { 17 },
                args: [
                    cap_slot_boundary(b2),
                    offset_boundary(b3),
                    width_boundary(b4),
                    u64::from(b5),
                ],
            },
            12 => SyscallOp::Invoke {
                caller,
                number: if b5 & 1 == 0 { 18 } else { 19 },
                args: [
                    cap_slot_boundary(b2),
                    offset_boundary(b3),
                    width_boundary(b4),
                    u64::from(b5),
                ],
            },
            13 => SyscallOp::Invoke {
                caller,
                number: 20,
                args: [cap_slot_boundary(b2), 0, 0, 0],
            },
            14 => SyscallOp::MintCap {
                task: caller,
                slot: (b2 as usize) % RUNTIME_CAP_SLOTS,
                profile: mint_profile(b3),
            },
            15 => SyscallOp::RevokeCap {
                task: caller,
                slot: (b2 as usize) % RUNTIME_CAP_SLOTS,
            },
            16 => SyscallOp::FaultTask { task: caller },
            _ => SyscallOp::Invoke {
                caller,
                number: u64::from(b2 % 2) + 5,
                args: [
                    scalar_boundary(b3),
                    scalar_boundary(b4),
                    scalar_boundary(b5),
                    0,
                ],
            },
        };
        operations.push(operation);
    }
    operations
}

/// Callers are drawn from the boot-profile task set below.
fn caller_boundary(selector: u8) -> usize {
    const CALLERS: [usize; 8] = [0, 1, 3, 4, 5, 6, 7, 15];
    CALLERS[(selector as usize) % CALLERS.len()]
}

fn number_boundary(selector: u8) -> u64 {
    match selector % 16 {
        0 => 0,
        1 => 1,
        2 => 2,
        3 => 4,
        4 => 5,
        5 => 6,
        6 => 7,
        7 => 8,
        8 => 14,
        9 => 20,
        10 => 21,
        11 => 32,
        12 => u64::from(u8::MAX),
        13 => u64::from(u16::MAX),
        14 => u64::from(u32::MAX),
        _ => u64::MAX,
    }
}

fn scalar_boundary(selector: u8) -> u64 {
    match selector % 10 {
        0 => 0,
        1 => 1,
        2 => 8,
        3 => 127,
        4 => 128,
        5 => 129,
        6 => u64::from(u8::MAX),
        7 => u64::from(u16::MAX),
        8 => u64::from(u32::MAX),
        _ => u64::MAX,
    }
}

fn cap_slot_boundary(selector: u8) -> u64 {
    match selector % 8 {
        0 => 0,
        1 => 1,
        2 => 2,
        3 => (RUNTIME_CAP_SLOTS - 1) as u64,
        4 => RUNTIME_CAP_SLOTS as u64,
        5 => (RUNTIME_CAP_SLOTS + 1) as u64,
        6 => u64::from(u8::MAX),
        _ => u64::MAX,
    }
}

fn va_boundary(selector: u8) -> u64 {
    match selector % 10 {
        0 => 0,
        1 => 1,
        2 => USER_DATA_VA,
        3 => USER_DATA_VA + 64,
        4 => USER_DATA_END - IPC_MSG_MAX as u64,
        5 => USER_DATA_END - 1,
        6 => USER_DATA_END,
        7 => KERNEL_BASE_VA,
        8 => 0x1_0000,
        _ => u64::MAX,
    }
}

fn len_boundary(selector: u8) -> u64 {
    match selector % 14 {
        0 => 0,
        1 => 1,
        2 => (IPC_MSG_MAX - 1) as u64,
        3 => IPC_MSG_MAX as u64,
        4 => (IPC_MSG_MAX + 1) as u64,
        5 => (CON_WRITE_MAX - 1) as u64,
        6 => CON_WRITE_MAX as u64,
        7 => (CON_WRITE_MAX + 1) as u64,
        8 => (INFO_MAX - 1) as u64,
        9 => INFO_MAX as u64,
        10 => (INFO_MAX + 1) as u64,
        11 => u64::from(u16::MAX),
        12 => u64::from(u32::MAX),
        _ => u64::MAX,
    }
}

fn task_boundary(selector: u8) -> u64 {
    match selector % 8 {
        0 => 0,
        1 => 1,
        2 => 7,
        3 => 9,
        4 => (MAX_TASKS - 1) as u64,
        5 => MAX_TASKS as u64,
        6 => u64::from(u8::MAX),
        _ => u64::MAX,
    }
}

fn service_boundary(selector: u8) -> u64 {
    match selector % 6 {
        0 => 0,
        1 => 1,
        2 => (NUM_SERVICES - 1) as u64,
        3 => NUM_SERVICES as u64,
        4 => u64::from(u8::MAX),
        _ => u64::MAX,
    }
}

fn offset_boundary(selector: u8) -> u64 {
    match selector % 15 {
        0 => 0,
        1 => 1,
        2 => 2,
        3 => 4,
        4 => MMIO0_SIZE - 4,
        5 => MMIO0_SIZE - 2,
        6 => MMIO0_SIZE - 1,
        7 => MMIO0_SIZE,
        8 => MMIO0_SIZE + 1,
        9 => DMA0_SIZE - 4,
        10 => DMA0_SIZE - 1,
        11 => DMA0_SIZE,
        12 => DMA0_SIZE + 1,
        13 => u64::MAX - 3,
        _ => u64::MAX,
    }
}

fn width_boundary(selector: u8) -> u64 {
    match selector % 8 {
        0 => 0,
        1 => 1,
        2 => 2,
        3 => 4,
        4 => 8,
        5 => 16,
        6 => u64::from(u8::MAX),
        _ => u64::MAX,
    }
}

fn decision_boundary(selector: u8) -> u64 {
    match selector % 7 {
        0 => 0,
        1 => 1,
        2 => 2,
        3 => 3,
        4 => 7,
        5 => u64::from(u8::MAX),
        _ => u64::MAX,
    }
}

fn mint_profile(selector: u8) -> MintProfile {
    match selector % 15 {
        0 => MintProfile::EndpointSendRecv0,
        1 => MintProfile::EndpointSendRecv11,
        2 => MintProfile::EndpointOutOfRange12,
        3 => MintProfile::EndpointOutOfRangeMax,
        4 => MintProfile::EndpointZeroRights,
        5 => MintProfile::ConsoleSendRecv,
        6 => MintProfile::Control,
        7 => MintProfile::Info,
        8 => MintProfile::DeviceBlockAll,
        9 => MintProfile::DeviceBlockReadOnly,
        10 => MintProfile::DeviceNet,
        11 => MintProfile::DeviceOutOfRange2,
        12 => MintProfile::DeviceOutOfRangeMax,
        13 => MintProfile::FaultChannelControl,
        _ => MintProfile::FaultChannelRecvOnly,
    }
}

// ---- Mandatory scenarios -----------------------------------------------

const VALID_VA: u64 = USER_DATA_VA + 64;

fn mandatory_scenario(index: u64) -> Vec<SyscallOp> {
    let invoke = |caller, number, args| SyscallOp::Invoke {
        caller,
        number,
        args,
    };
    // Task profiles (see SyscallModel::new): 0 init(control+info),
    // 1 supervisor(fault channel recv+control), 3 shell(console+info+
    // control+ep1 recv), 4 app(ep0 send), 5 driver_mgr(dev0 control+info),
    // 6 block_driver(dev0 mmio/dma + ep8 recv), 7 peer(ep0 recv),
    // 15 edge(ep11 send+recv).
    match index {
        0 => vec![invoke(4, 0, [0, 0, 0, 0])],
        1 => vec![invoke(4, 1, [0, 0, 0, 0])],
        2 => vec![invoke(7, 4, [0, VALID_VA, 64, 0])],
        3 => vec![invoke(4, 5, [u64::MAX, u64::MAX, u64::MAX, u64::MAX])],
        4 => vec![invoke(4, 6, [0, 1, 2, 3])],
        5 => vec![invoke(1, 7, [0, 2, 0, 0])],
        6 => vec![invoke(5, 20, [0, 0, 0, 0])],
        7 => vec![invoke(4, 21, [0, 0, 0, 0])],
        8 => vec![invoke(
            4,
            u64::MAX,
            [u64::MAX, u64::MAX, u64::MAX, u64::MAX],
        )],
        // cap_slot_0: valid send through slot 0.
        9 => vec![invoke(4, 3, [0, VALID_VA, 8, 0])],
        // cap_slot_8: mint into the last valid runtime slot, then use it.
        10 => vec![
            SyscallOp::MintCap {
                task: 4,
                slot: RUNTIME_CAP_SLOTS - 1,
                profile: MintProfile::EndpointSendRecv0,
            },
            invoke(4, 3, [(RUNTIME_CAP_SLOTS - 1) as u64, VALID_VA, 8, 0]),
        ],
        11 => vec![invoke(4, 3, [RUNTIME_CAP_SLOTS as u64, VALID_VA, 8, 0])],
        12 => vec![invoke(4, 3, [u64::MAX, VALID_VA, 8, 0])],
        13 => vec![invoke(4, 3, [1, VALID_VA, 8, 0])],
        14 => vec![
            SyscallOp::RevokeCap { task: 4, slot: 0 },
            invoke(4, 3, [0, VALID_VA, 8, 0]),
        ],
        // cap_slot_wrong_type: console cap used as an endpoint.
        15 => vec![
            SyscallOp::MintCap {
                task: 4,
                slot: 2,
                profile: MintProfile::ConsoleSendRecv,
            },
            invoke(4, 3, [2, VALID_VA, 8, 0]),
        ],
        // cap_slot_wrong_object: device cap naming the wrong device.
        16 => vec![
            SyscallOp::MintCap {
                task: 6,
                slot: 2,
                profile: MintProfile::DeviceNet,
            },
            invoke(6, 16, [2, 0, 4, 0]),
        ],
        // cap_slot_wrong_rights: recv-only fault channel cannot ack.
        17 => vec![
            SyscallOp::MintCap {
                task: 4,
                slot: 3,
                profile: MintProfile::FaultChannelRecvOnly,
            },
            invoke(4, 7, [0, 2, 0, 0]),
        ],
        18 => vec![invoke(4, 3, [0, VALID_VA, 8, 0])],
        19 => vec![invoke(15, 3, [0, VALID_VA, 8, 0])],
        20 => vec![
            SyscallOp::MintCap {
                task: 4,
                slot: 2,
                profile: MintProfile::EndpointOutOfRange12,
            },
            invoke(4, 3, [2, VALID_VA, 8, 0]),
        ],
        21 => vec![
            SyscallOp::MintCap {
                task: 4,
                slot: 2,
                profile: MintProfile::EndpointOutOfRangeMax,
            },
            invoke(4, 3, [2, VALID_VA, 8, 0]),
        ],
        22 => vec![invoke(3, 12, [0, 0, 0, 0])],
        23 => vec![invoke(3, 12, [(MAX_TASKS - 1) as u64, 0, 0, 0])],
        24 => vec![invoke(3, 12, [MAX_TASKS as u64, 0, 0, 0])],
        25 => vec![invoke(3, 13, [u64::MAX, 0, 0, 0])],
        26 => vec![invoke(3, 8, [0, 0, 0, 0])],
        27 => vec![invoke(3, 8, [(NUM_SERVICES - 1) as u64, 0, 0, 0])],
        28 => vec![invoke(3, 8, [NUM_SERVICES as u64, 0, 0, 0])],
        29 => vec![invoke(3, 8, [u64::MAX, 0, 0, 0])],
        30 => vec![invoke(6, 16, [0, 0, 4, 0])],
        31 => vec![
            invoke(6, 16, [0, MMIO0_SIZE - 4, 4, 0]),
            invoke(6, 16, [0, MMIO0_SIZE - 1, 1, 0]),
        ],
        32 => vec![
            invoke(6, 16, [0, MMIO0_SIZE - 2, 4, 0]),
            invoke(6, 16, [0, MMIO0_SIZE, 1, 0]),
            invoke(6, 16, [0, u64::MAX, 1, 0]),
        ],
        33 => vec![
            invoke(6, 16, [0, 1, 2, 0]),
            invoke(6, 16, [0, 2, 4, 0]),
            invoke(6, 17, [0, MMIO0_SIZE - 1, 2, 0xab]),
        ],
        34 => vec![
            invoke(6, 16, [0, 0, 0, 0]),
            invoke(6, 16, [0, 0, 8, 0]),
            invoke(6, 16, [0, 0, u64::MAX, 0]),
        ],
        // mmio_zero_size_device: net0 has no MMIO window at all.
        35 => vec![
            SyscallOp::MintCap {
                task: 6,
                slot: 2,
                profile: MintProfile::DeviceNet,
            },
            invoke(6, 16, [2, 0, 1, 0]),
        ],
        36 => vec![
            SyscallOp::MintCap {
                task: 6,
                slot: 2,
                profile: MintProfile::DeviceOutOfRange2,
            },
            invoke(6, 16, [2, 0, 4, 0]),
            SyscallOp::MintCap {
                task: 6,
                slot: 3,
                profile: MintProfile::DeviceOutOfRangeMax,
            },
            invoke(6, 16, [3, 0, 4, 0]),
        ],
        37 => vec![invoke(6, 18, [0, 0, 1, 0])],
        38 => vec![
            invoke(6, 18, [0, DMA0_SIZE - 4, 4, 0]),
            invoke(6, 19, [0, DMA0_SIZE - 1, 1, 0x5a]),
        ],
        39 => vec![
            invoke(6, 18, [0, DMA0_SIZE - 2, 4, 0]),
            invoke(6, 18, [0, DMA0_SIZE, 1, 0]),
        ],
        40 => vec![
            invoke(6, 19, [0, 0, 0, 1]),
            invoke(6, 19, [0, 0, 8, 1]),
            invoke(6, 19, [0, 0, u64::MAX, 1]),
        ],
        41 => vec![
            invoke(6, 18, [0, u64::MAX - 3, 4, 0]),
            invoke(6, 19, [0, u64::MAX, 1, 1]),
        ],
        // dma_direction_mismatch: read-only device grant used for writes.
        42 => vec![
            SyscallOp::MintCap {
                task: 5,
                slot: 2,
                profile: MintProfile::DeviceBlockReadOnly,
            },
            invoke(5, 19, [2, 0, 1, 1]),
            invoke(5, 17, [2, 0, 4, 1]),
        ],
        43 => vec![invoke(4, 3, [0, 0, 8, 0])],
        44 => vec![invoke(
            4,
            3,
            [0, USER_DATA_END - IPC_MSG_MAX as u64, IPC_MSG_MAX as u64, 0],
        )],
        45 => vec![invoke(4, 3, [0, KERNEL_BASE_VA, 8, 0])],
        46 => vec![invoke(4, 3, [0, USER_DATA_END - 1, 2, 0])],
        47 => vec![invoke(4, 3, [0, u64::MAX, IPC_MSG_MAX as u64, 0])],
        // lifecycle_start_valid: service 9 -> empty slot 10.
        48 => vec![invoke(3, 8, [9, 0, 0, 0])],
        49 => vec![invoke(3, 8, [0, 0, 0, 0]), invoke(3, 8, [0, 0, 0, 0])],
        50 => vec![
            invoke(3, 12, [7, 0, 0, 0]),
            invoke(3, 12, [7, 0, 0, 0]),
            invoke(3, 13, [7, 0, 0, 0]),
            invoke(3, 13, [7, 0, 0, 0]),
        ],
        51 => vec![invoke(3, 13, [3, 0, 0, 0])],
        52 => vec![SyscallOp::FaultTask { task: 4 }, invoke(1, 7, [0, 2, 0, 0])],
        53 => vec![invoke(4, 7, [0, 2, 0, 0]), invoke(6, 7, [0, 1, 0, 0])],
        54 => vec![
            invoke(1, 7, [0, 7, 0, 0]),
            invoke(1, 7, [0, u64::MAX, 0, 0]),
        ],
        _ => vec![invoke(1, 7, [0, 2, 0, 0])],
    }
}

// ---- Model implementation ----------------------------------------------

const NO_CAPS: [Option<ModelCap>; RUNTIME_CAP_SLOTS] = [None; RUNTIME_CAP_SLOTS];

fn endpoint_cap(object_id: u32, rights: u16) -> ModelCap {
    ModelCap {
        kind: ObjKind::Endpoint,
        object_id,
        rights,
    }
}

fn device_cap(object_id: u32, rights: u16) -> ModelCap {
    ModelCap {
        kind: ObjKind::Device,
        object_id,
        rights,
    }
}

fn profile_cap(profile: MintProfile) -> ModelCap {
    match profile {
        MintProfile::EndpointSendRecv0 => endpoint_cap(0, RIGHT_SEND | RIGHT_RECV),
        MintProfile::EndpointSendRecv11 => {
            endpoint_cap((NUM_ENDPOINTS - 1) as u32, RIGHT_SEND | RIGHT_RECV)
        }
        MintProfile::EndpointOutOfRange12 => {
            endpoint_cap(NUM_ENDPOINTS as u32, RIGHT_SEND | RIGHT_RECV)
        }
        MintProfile::EndpointOutOfRangeMax => endpoint_cap(u32::MAX, RIGHT_SEND | RIGHT_RECV),
        MintProfile::EndpointZeroRights => endpoint_cap(0, 0),
        MintProfile::ConsoleSendRecv => ModelCap {
            kind: ObjKind::Console,
            object_id: 0,
            rights: RIGHT_SEND | RIGHT_RECV,
        },
        MintProfile::Control => ModelCap {
            kind: ObjKind::Control,
            object_id: 0,
            rights: RIGHT_CONTROL,
        },
        MintProfile::Info => ModelCap {
            kind: ObjKind::Info,
            object_id: 0,
            rights: RIGHT_RECV,
        },
        MintProfile::DeviceBlockAll => device_cap(
            0,
            DEV_INFO
                | DEV_MMIO_READ
                | DEV_MMIO_WRITE
                | DEV_DMA_READ
                | DEV_DMA_WRITE
                | DEV_IRQ_RECEIVE,
        ),
        MintProfile::DeviceBlockReadOnly => device_cap(0, DEV_INFO | DEV_MMIO_READ | DEV_DMA_READ),
        MintProfile::DeviceNet => device_cap(1, DEV_INFO | DEV_MMIO_READ | DEV_DMA_READ),
        MintProfile::DeviceOutOfRange2 => device_cap(NUM_DEVICES as u32, DEV_MMIO_READ),
        MintProfile::DeviceOutOfRangeMax => device_cap(u32::MAX, DEV_MMIO_READ),
        MintProfile::FaultChannelControl => endpoint_cap(EP_FAULT, RIGHT_RECV | RIGHT_CONTROL),
        MintProfile::FaultChannelRecvOnly => endpoint_cap(EP_FAULT, RIGHT_RECV),
    }
}

/// Boot capability profile per slot, a documented representative mirror
/// of the os_boot service table (docs/25 §5): explicit, deny-by-default.
fn boot_caps(slot: usize) -> [Option<ModelCap>; RUNTIME_CAP_SLOTS] {
    let mut caps = NO_CAPS;
    match slot {
        0 => {
            caps[0] = Some(profile_cap(MintProfile::Control));
            caps[1] = Some(profile_cap(MintProfile::Info));
        }
        1 => caps[0] = Some(endpoint_cap(EP_FAULT, RIGHT_RECV | RIGHT_CONTROL)),
        2 => caps[0] = Some(endpoint_cap(EP_EVENT, RIGHT_RECV)),
        3 => {
            caps[0] = Some(profile_cap(MintProfile::ConsoleSendRecv));
            caps[1] = Some(profile_cap(MintProfile::Info));
            caps[2] = Some(profile_cap(MintProfile::Control));
            caps[3] = Some(endpoint_cap(1, RIGHT_RECV));
        }
        4 => caps[0] = Some(endpoint_cap(0, RIGHT_SEND)),
        5 => caps[0] = Some(device_cap(0, DEV_INFO | DEV_DRIVER_CONTROL)),
        6 => {
            caps[0] = Some(profile_cap(MintProfile::DeviceBlockAll));
            caps[1] = Some(endpoint_cap(IRQ_ENDPOINTS[0], RIGHT_RECV));
        }
        7 => caps[0] = Some(endpoint_cap(0, RIGHT_RECV)),
        15 => {
            caps[0] = Some(endpoint_cap(
                (NUM_ENDPOINTS - 1) as u32,
                RIGHT_SEND | RIGHT_RECV,
            ))
        }
        _ => {}
    }
    caps
}

fn boot_task(slot: usize) -> ModelTask {
    let life = match slot {
        0..=7 | 15 => Life::Ready,
        8 => Life::Killed,
        9 => Life::Faulted,
        _ => Life::Empty,
    };
    let entry_va = match slot {
        // Slot 8 is a killed task that was never table-armed: it cannot
        // be restarted (entry_va == 0), only killed-state observed.
        8 => 0,
        _ if life != Life::Empty => 0x1_0000 + slot as u64,
        _ => 0,
    };
    ModelTask {
        life,
        caps: if life == Life::Empty {
            NO_CAPS
        } else {
            boot_caps(slot)
        },
        pending_delivery: false,
        entry_va,
    }
}

impl SyscallModel {
    fn new() -> Self {
        let mut model = Self {
            tasks: std::array::from_fn(boot_task),
            endpoints: [EpState::Idle; NUM_ENDPOINTS],
            irq_routes: [IrqRoute {
                receiver: MAX_TASKS,
                pending: false,
            }; NUM_DEVICES],
            mmio: [0; MMIO0_SIZE as usize],
            dma: [0; DMA0_SIZE as usize],
            console_bytes: 0,
            info_reads: 0,
            shutdowns: 0,
            evidence_events: 0,
            recovery_acks: 0,
            counts: OperationCounts::default(),
        };
        model.register_irq_receivers(6);
        model
    }

    /// Register IRQ routes for a task's boot-minted `irq_receive` device
    /// grants (dispatch.rs announce_device_grants).
    fn register_irq_receivers(&mut self, slot: usize) {
        for cap in self.tasks[slot].caps.into_iter().flatten() {
            if cap.kind == ObjKind::Device
                && (cap.object_id as usize) < NUM_DEVICES
                && cap.rights & DEV_IRQ_RECEIVE != 0
            {
                self.irq_routes[cap.object_id as usize].receiver = slot;
            }
        }
    }

    fn protected(&self) -> Protected {
        Protected {
            tasks: self.tasks.to_vec(),
            endpoints: self.endpoints.to_vec(),
            irq_routes: self.irq_routes.to_vec(),
            mmio: self.mmio.to_vec(),
            dma: self.dma.to_vec(),
            console_bytes: self.console_bytes,
            info_reads: self.info_reads,
            shutdowns: self.shutdowns,
        }
    }

    fn outcome(&self) -> ModelOutcome {
        ModelOutcome {
            protected: self.protected(),
            evidence_events: self.evidence_events,
            recovery_acks: self.recovery_acks,
            counts: self.counts,
        }
    }

    fn record(&mut self, disposition: StepDisposition) {
        match disposition {
            StepDisposition::Accepted => self.counts.accepted += 1,
            StepDisposition::SafeReject => self.counts.safe_rejects += 1,
            StepDisposition::Busy => self.counts.busy += 1,
        }
    }

    fn apply(&mut self, operation: &SyscallOp) -> Result<StepDisposition, String> {
        match *operation {
            SyscallOp::MintCap {
                task,
                slot,
                profile,
            } => {
                if task >= MAX_TASKS || slot >= RUNTIME_CAP_SLOTS {
                    return Err(format!(
                        "SYS-INV-001: mint decoded out-of-range task {task} slot {slot}"
                    ));
                }
                self.tasks[task].caps[slot] = Some(profile_cap(profile));
                Ok(StepDisposition::Accepted)
            }
            SyscallOp::RevokeCap { task, slot } => {
                if task >= MAX_TASKS || slot >= RUNTIME_CAP_SLOTS {
                    return Err(format!(
                        "SYS-INV-001: revoke decoded out-of-range task {task} slot {slot}"
                    ));
                }
                self.tasks[task].caps[slot] = None;
                Ok(StepDisposition::Accepted)
            }
            SyscallOp::FaultTask { task } => {
                if task >= MAX_TASKS {
                    return Err(format!(
                        "SYS-INV-001: fault decoded out-of-range task {task}"
                    ));
                }
                if self.tasks[task].life != Life::Ready {
                    return Ok(StepDisposition::SafeReject);
                }
                self.tasks[task].life = Life::Faulted;
                self.irq_drop_for_task(task);
                self.notify_endpoint(EP_FAULT);
                self.notify_endpoint(EP_EVENT);
                self.evidence_events += 1;
                Ok(StepDisposition::Accepted)
            }
            SyscallOp::Invoke {
                caller,
                number,
                args,
            } => self.invoke(caller, number, args),
        }
    }

    /// Kernel fault/event notification (dispatch.rs notify_endpoint):
    /// a waiting receiver is made Ready with a pending delivery.
    fn notify_endpoint(&mut self, ep_id: u32) {
        if let EpState::ReceiverWaiting { tid, .. } = self.endpoints[ep_id as usize] {
            self.tasks[tid].life = Life::Ready;
            self.tasks[tid].pending_delivery = true;
            self.endpoints[ep_id as usize] = EpState::Idle;
        }
    }

    fn ep_clear_for_task(&mut self, slot: usize) {
        for endpoint in &mut self.endpoints {
            match *endpoint {
                EpState::SenderWaiting { tid, .. } | EpState::ReceiverWaiting { tid, .. }
                    if tid == slot =>
                {
                    *endpoint = EpState::Idle;
                }
                _ => {}
            }
        }
    }

    fn irq_drop_for_task(&mut self, slot: usize) {
        for route in &mut self.irq_routes {
            if route.receiver == slot {
                route.pending = false;
            }
        }
    }

    // ---- Capability checks (runtime fixed order, docs/06 §4) -----------

    fn cap_check_endpoint(&self, task: usize, slot: u64, required: u16) -> Result<u32, i64> {
        if slot >= RUNTIME_CAP_SLOTS as u64 {
            return Err(ERR_INVALID_CAP);
        }
        match self.tasks[task].caps[slot as usize] {
            None => Err(ERR_INVALID_CAP),
            Some(cap) if cap.kind != ObjKind::Endpoint => Err(ERR_WRONG_OBJECT_TYPE),
            Some(cap) if cap.rights & required != required => Err(ERR_INSUFFICIENT_RIGHTS),
            Some(cap) => Ok(cap.object_id),
        }
    }

    fn device_cap_check(&self, task: usize, slot: u64, required: u16) -> Result<usize, i64> {
        if slot >= RUNTIME_CAP_SLOTS as u64 {
            return Err(ERR_INVALID_CAP);
        }
        match self.tasks[task].caps[slot as usize] {
            None => Err(ERR_INVALID_CAP),
            Some(cap) if cap.kind != ObjKind::Device => Err(ERR_WRONG_OBJECT_TYPE),
            Some(cap) if cap.object_id as usize >= NUM_DEVICES => Err(ERR_INVALID_CAP),
            Some(cap) if cap.rights & required != required => Err(ERR_INSUFFICIENT_RIGHTS),
            Some(cap) => Ok(cap.object_id as usize),
        }
    }

    fn cap_find(&self, task: usize, kind: ObjKind, required: u16) -> bool {
        self.tasks[task].caps.iter().any(
            |cap| matches!(cap, Some(cap) if cap.kind == kind && cap.rights & required == required),
        )
    }

    /// Resolved sys_fault_ack authority (AXIOM-ROBUST-005): a fault
    /// channel endpoint capability carrying the Control right.
    fn holds_fault_ack_authority(&self, task: usize) -> bool {
        self.tasks[task].caps.iter().any(|cap| {
            matches!(cap, Some(cap) if cap.kind == ObjKind::Endpoint
                && cap.object_id == EP_FAULT
                && cap.rights & RIGHT_CONTROL == RIGHT_CONTROL)
        })
    }

    // ---- Window validators (dispatch.rs valid_user_buf/in_stack_window,
    // modeled: the runtime additionally accepts the read-only sectioned
    // user region for sys_con_write; real MMU behavior is ROBUST-013) ----

    fn valid_user_buf(va: u64, len: u64) -> bool {
        len <= IPC_MSG_MAX as u64
            && va >= USER_DATA_VA
            && va.checked_add(len).is_some_and(|end| end <= USER_DATA_END)
    }

    fn in_stack_window(va: u64, len: u64) -> bool {
        va >= USER_DATA_VA && va.checked_add(len).is_some_and(|end| end <= USER_DATA_END)
    }

    // ---- Central invocation (mirrors trap.rs + dispatch.rs) ------------

    fn invoke(
        &mut self,
        caller: usize,
        number: u64,
        args: [u64; 4],
    ) -> Result<StepDisposition, String> {
        if caller >= MAX_TASKS {
            return Err(format!("SYS-INV-001: decoded out-of-range caller {caller}"));
        }
        // Only a running task can reach the syscall boundary.
        if self.tasks[caller].life != Life::Ready {
            return Ok(StepDisposition::SafeReject);
        }

        let before = self.protected();
        let (result, disposition) = self.dispatch(caller, number, args)?;

        // SYS-INV-002/003/014: a rejected syscall must leave every piece
        // of protected state untouched (denial evidence is separate).
        if result < 0 && self.protected() != before {
            return Err(format!(
                "SYS-INV-014: rejected syscall {number} (result {result}) mutated protected state"
            ));
        }
        match classify(number) {
            SyscallClass::Invalid => {
                if result != ERR_INVALID_SYSCALL {
                    return Err(format!(
                        "SYS-INV-001: invalid syscall {number} produced result {result}"
                    ));
                }
            }
            SyscallClass::RecognizedNotImplemented => {
                if result != ERR_NOT_IMPLEMENTED {
                    return Err(format!(
                        "SYS-INV-002: recognized stub {number} produced result {result}"
                    ));
                }
            }
            SyscallClass::Implemented => {}
        }
        Ok(disposition)
    }

    fn dispatch(
        &mut self,
        caller: usize,
        number: u64,
        args: [u64; 4],
    ) -> Result<(i64, StepDisposition), String> {
        let outcome = match classify(number) {
            SyscallClass::Invalid => (ERR_INVALID_SYSCALL, StepDisposition::SafeReject),
            SyscallClass::RecognizedNotImplemented => {
                (ERR_NOT_IMPLEMENTED, StepDisposition::SafeReject)
            }
            SyscallClass::Implemented => match number {
                1 => (OK, StepDisposition::Accepted),
                2 => {
                    self.tasks[caller].life = Life::Killed;
                    (OK, StepDisposition::Accepted)
                }
                3 => self.sys_send(caller, args)?,
                4 => self.sys_recv(caller, args)?,
                7 => self.sys_fault_ack(caller, args),
                8 => self.sys_task_start(caller, args),
                9 => self.sys_con_write(caller, args),
                10 => self.sys_con_read(caller, args),
                11 => self.sys_info(caller, args),
                12 => self.sys_task_kill(caller, args),
                13 => self.sys_task_restart(caller, args),
                14 => self.sys_shutdown(caller),
                15 => self.sys_device_info(caller, args),
                16 => self.sys_mmio(caller, args, false)?,
                17 => self.sys_mmio(caller, args, true)?,
                18 => self.sys_dma(caller, args, false)?,
                19 => self.sys_dma(caller, args, true)?,
                20 => self.sys_irq_raise(caller, args),
                _ => {
                    return Err(format!(
                        "SYS-INV-001: unhandled implemented number {number}"
                    ))
                }
            },
        };
        Ok(outcome)
    }

    fn deny(&mut self, code: i64) -> (i64, StepDisposition) {
        // Runtime denials push a CAP_DENIED/`*_DENIED` ring event.
        self.evidence_events += 1;
        (code, StepDisposition::SafeReject)
    }

    /// SYS-INV-005 guard: no capability may resolve to an endpoint id at
    /// or beyond NUM_ENDPOINTS. The live kernel guarantees this through
    /// boot-frozen definitions (docs/36 §5.5, ROBUST-011); the model
    /// enforces the bound explicitly before any endpoint array access.
    fn ep_guarded(&self, ep_id: u32) -> Result<usize, (i64, StepDisposition)> {
        if (ep_id as usize) < NUM_ENDPOINTS {
            Ok(ep_id as usize)
        } else {
            Err((ERR_INVALID_ARG, StepDisposition::SafeReject))
        }
    }

    fn sys_send(
        &mut self,
        caller: usize,
        args: [u64; 4],
    ) -> Result<(i64, StepDisposition), String> {
        let ep_id = match self.cap_check_endpoint(caller, args[0], RIGHT_SEND) {
            Ok(id) => id,
            Err(code) => return Ok(self.deny(code)),
        };
        let (va, len) = (args[1], args[2]);
        if len > IPC_MSG_MAX as u64 {
            return Ok((ERR_MSG_TOO_LARGE, StepDisposition::SafeReject));
        }
        if !Self::valid_user_buf(va, len) {
            return Ok((ERR_INVALID_ARG, StepDisposition::SafeReject));
        }
        let ep = match self.ep_guarded(ep_id) {
            Ok(index) => index,
            Err(outcome) => return Ok(outcome),
        };
        match self.endpoints[ep] {
            EpState::ReceiverWaiting { tid, cap } => {
                if len as usize <= cap {
                    self.tasks[tid].pending_delivery = true;
                }
                self.tasks[tid].life = Life::Ready;
                self.endpoints[ep] = EpState::Idle;
                Ok((len as i64, StepDisposition::Accepted))
            }
            EpState::Idle => {
                self.endpoints[ep] = EpState::SenderWaiting {
                    tid: caller,
                    len: len as usize,
                };
                self.tasks[caller].life = Life::Blocked;
                Ok((len as i64, StepDisposition::Accepted))
            }
            EpState::SenderWaiting { .. } => Ok((ERR_INVALID_ARG, StepDisposition::Busy)),
        }
    }

    fn sys_recv(
        &mut self,
        caller: usize,
        args: [u64; 4],
    ) -> Result<(i64, StepDisposition), String> {
        let ep_id = match self.cap_check_endpoint(caller, args[0], RIGHT_RECV) {
            Ok(id) => id,
            Err(code) => return Ok(self.deny(code)),
        };
        let (dst, cap) = (args[1], args[2]);
        let ep = match self.ep_guarded(ep_id) {
            Ok(index) => index,
            Err(outcome) => return Ok(outcome),
        };

        // Recv-side IRQ delivery (docs/31 §9) runs before the rendezvous.
        for (device, irq_endpoint) in IRQ_ENDPOINTS.iter().enumerate() {
            if *irq_endpoint as usize == ep
                && self.irq_routes[device].pending
                && self.irq_routes[device].receiver == caller
            {
                if cap < 1 || !Self::valid_user_buf(dst, 1) {
                    return Ok((ERR_INVALID_ARG, StepDisposition::SafeReject));
                }
                self.irq_routes[device].pending = false;
                return Ok((1, StepDisposition::Accepted));
            }
        }

        match self.endpoints[ep] {
            EpState::SenderWaiting { tid, len } => {
                if len as u64 > cap || !Self::valid_user_buf(dst, len as u64) {
                    return Ok((ERR_INVALID_ARG, StepDisposition::SafeReject));
                }
                self.tasks[tid].life = Life::Ready;
                self.endpoints[ep] = EpState::Idle;
                Ok((len as i64, StepDisposition::Accepted))
            }
            EpState::Idle => {
                if cap > IPC_MSG_MAX as u64
                    || !Self::valid_user_buf(dst, cap.min(IPC_MSG_MAX as u64))
                {
                    return Ok((ERR_INVALID_ARG, StepDisposition::SafeReject));
                }
                self.endpoints[ep] = EpState::ReceiverWaiting {
                    tid: caller,
                    cap: cap as usize,
                };
                self.tasks[caller].life = Life::Blocked;
                Ok((0, StepDisposition::Accepted))
            }
            EpState::ReceiverWaiting { .. } => Ok((ERR_INVALID_ARG, StepDisposition::Busy)),
        }
    }

    /// Resolved contract (AXIOM-ROBUST-005): authority gate, then the
    /// docs/19 §4 record-only acknowledgement. SYS-INV-017: unauthorized
    /// callers are rejected without recording recovery evidence.
    /// SYS-INV-018: an authorized ack never mutates protected state.
    fn sys_fault_ack(&mut self, caller: usize, _args: [u64; 4]) -> (i64, StepDisposition) {
        if !self.holds_fault_ack_authority(caller) {
            return self.deny(ERR_INVALID_CAP);
        }
        self.recovery_acks += 1;
        self.evidence_events += 1;
        (OK, StepDisposition::Accepted)
    }

    fn sys_task_start(&mut self, caller: usize, args: [u64; 4]) -> (i64, StepDisposition) {
        if !self.cap_find(caller, ObjKind::Control, RIGHT_CONTROL) {
            return self.deny(ERR_INVALID_CAP);
        }
        let index = args[0];
        if index >= NUM_SERVICES as u64 {
            return (ERR_INVALID_ARG, StepDisposition::SafeReject);
        }
        // Service i occupies TCB slot i+1 (init holds slot 0, docs/25 §3).
        let slot = index as usize + 1;
        match self.tasks[slot].life {
            Life::Empty => {
                self.tasks[slot] = ModelTask {
                    life: Life::Ready,
                    caps: boot_caps(slot),
                    pending_delivery: false,
                    entry_va: 0x1_0000 + slot as u64,
                };
                self.register_irq_receivers(slot);
            }
            Life::Killed | Life::Faulted => {
                self.tasks[slot].life = Life::Ready;
                self.tasks[slot].pending_delivery = false;
                self.ep_clear_for_task(slot);
            }
            _ => return (ERR_NO_SLOT, StepDisposition::Busy),
        }
        self.evidence_events += 1;
        (slot as i64, StepDisposition::Accepted)
    }

    fn sys_con_write(&mut self, caller: usize, args: [u64; 4]) -> (i64, StepDisposition) {
        if !self.cap_find(caller, ObjKind::Console, RIGHT_SEND) {
            return self.deny(ERR_INVALID_CAP);
        }
        let (va, len) = (args[0], args[1]);
        if len > CON_WRITE_MAX as u64 || !Self::in_stack_window(va, len) {
            return (ERR_INVALID_ARG, StepDisposition::SafeReject);
        }
        self.console_bytes += len;
        (len as i64, StepDisposition::Accepted)
    }

    fn sys_con_read(&mut self, caller: usize, args: [u64; 4]) -> (i64, StepDisposition) {
        if !self.cap_find(caller, ObjKind::Console, RIGHT_RECV) {
            return self.deny(ERR_INVALID_CAP);
        }
        let (va, max) = (args[0], args[1].min(IPC_MSG_MAX as u64));
        if !Self::in_stack_window(va, max) {
            return (ERR_INVALID_ARG, StepDisposition::SafeReject);
        }
        (0, StepDisposition::Accepted)
    }

    fn sys_info(&mut self, caller: usize, args: [u64; 4]) -> (i64, StepDisposition) {
        if !self.cap_find(caller, ObjKind::Info, RIGHT_RECV) {
            return self.deny(ERR_INVALID_CAP);
        }
        let (kind, va, max, slot) = (args[0], args[1], args[2], args[3]);
        if !Self::in_stack_window(va, max.min(INFO_MAX as u64)) {
            return (ERR_INVALID_ARG, StepDisposition::SafeReject);
        }
        if kind > 7 {
            return (ERR_INVALID_ARG, StepDisposition::SafeReject);
        }
        if kind == 7 && slot >= MAX_TASKS as u64 {
            return (ERR_INVALID_ARG, StepDisposition::SafeReject);
        }
        self.info_reads += 1;
        (max.min(INFO_MAX as u64) as i64, StepDisposition::Accepted)
    }

    fn sys_task_kill(&mut self, caller: usize, args: [u64; 4]) -> (i64, StepDisposition) {
        if !self.cap_find(caller, ObjKind::Control, RIGHT_CONTROL) {
            return self.deny(ERR_INVALID_CAP);
        }
        let slot = args[0];
        if slot >= MAX_TASKS as u64 || self.tasks[slot as usize].life == Life::Empty {
            return (ERR_INVALID_ARG, StepDisposition::SafeReject);
        }
        let slot = slot as usize;
        self.tasks[slot].life = Life::Killed;
        self.tasks[slot].pending_delivery = false;
        self.ep_clear_for_task(slot);
        self.irq_drop_for_task(slot);
        self.evidence_events += 1;
        (OK, StepDisposition::Accepted)
    }

    fn sys_task_restart(&mut self, caller: usize, args: [u64; 4]) -> (i64, StepDisposition) {
        if !self.cap_find(caller, ObjKind::Control, RIGHT_CONTROL) {
            return self.deny(ERR_INVALID_CAP);
        }
        let slot = args[0];
        if slot >= MAX_TASKS as u64
            || slot as usize == caller
            || self.tasks[slot as usize].life == Life::Empty
            || self.tasks[slot as usize].entry_va == 0
        {
            return (ERR_INVALID_ARG, StepDisposition::SafeReject);
        }
        let slot = slot as usize;
        self.tasks[slot].life = Life::Ready;
        self.tasks[slot].pending_delivery = false;
        self.ep_clear_for_task(slot);
        self.evidence_events += 1;
        (OK, StepDisposition::Accepted)
    }

    fn sys_shutdown(&mut self, caller: usize) -> (i64, StepDisposition) {
        if !self.cap_find(caller, ObjKind::Control, RIGHT_CONTROL) {
            return self.deny(ERR_INVALID_CAP);
        }
        // The live syscall does not return; the model records the event.
        self.shutdowns += 1;
        (OK, StepDisposition::Accepted)
    }

    fn sys_device_info(&mut self, caller: usize, args: [u64; 4]) -> (i64, StepDisposition) {
        if let Err(code) = self.device_cap_check(caller, args[0], DEV_INFO) {
            return self.deny(code);
        }
        let (va, max) = (args[1], args[2]);
        if !Self::in_stack_window(va, max.min(INFO_MAX as u64)) {
            return (ERR_INVALID_ARG, StepDisposition::SafeReject);
        }
        self.info_reads += 1;
        (max.min(INFO_MAX as u64) as i64, StepDisposition::Accepted)
    }

    fn device_mmio_size(device: usize) -> u64 {
        if device == 0 {
            MMIO0_SIZE
        } else {
            0
        }
    }

    fn device_dma_size(device: usize) -> u64 {
        if device == 0 {
            DMA0_SIZE
        } else {
            0
        }
    }

    fn sys_mmio(
        &mut self,
        caller: usize,
        args: [u64; 4],
        write: bool,
    ) -> Result<(i64, StepDisposition), String> {
        let required = if write { DEV_MMIO_WRITE } else { DEV_MMIO_READ };
        let device = match self.device_cap_check(caller, args[0], required) {
            Ok(device) => device,
            Err(code) => return Ok(self.deny(code)),
        };
        let (offset, width, value) = (args[1], args[2], args[3]);
        // The real runtime validator (kernel::device::access_in_bounds):
        // width in {1,2,4}, width-aligned offset, checked offset+width.
        if !access_in_bounds(Self::device_mmio_size(device), offset, width) {
            return Ok(self.deny(ERR_INVALID_ARG));
        }
        let start = offset as usize;
        let end = start + width as usize;
        if end > self.mmio.len() {
            return Err(format!(
                "SYS-INV-011: validated MMIO access [{start}, {end}) escapes the 0x{MMIO0_SIZE:x} window"
            ));
        }
        if write {
            for (index, byte) in self.mmio[start..end].iter_mut().enumerate() {
                *byte = (value >> (8 * index)) as u8;
            }
            Ok((OK, StepDisposition::Accepted))
        } else {
            let mut value: u64 = 0;
            for (index, byte) in self.mmio[start..end].iter().enumerate() {
                value |= u64::from(*byte) << (8 * index);
            }
            Ok((value as i64, StepDisposition::Accepted))
        }
    }

    fn sys_dma(
        &mut self,
        caller: usize,
        args: [u64; 4],
        write: bool,
    ) -> Result<(i64, StepDisposition), String> {
        let required = if write { DEV_DMA_WRITE } else { DEV_DMA_READ };
        let device = match self.device_cap_check(caller, args[0], required) {
            Ok(device) => device,
            Err(code) => return Ok(self.deny(code)),
        };
        let (offset, width, value) = (args[1], args[2], args[3]);
        if !access_in_bounds(Self::device_dma_size(device), offset, width) {
            return Ok(self.deny(ERR_INVALID_ARG));
        }
        let start = offset as usize;
        let end = start + width as usize;
        if end > self.dma.len() {
            return Err(format!(
                "SYS-INV-012: validated DMA access [{start}, {end}) escapes the {DMA0_SIZE}-byte page"
            ));
        }
        if write {
            for (index, byte) in self.dma[start..end].iter_mut().enumerate() {
                *byte = (value >> (8 * index)) as u8;
            }
            Ok((OK, StepDisposition::Accepted))
        } else {
            let mut value: u64 = 0;
            for (index, byte) in self.dma[start..end].iter().enumerate() {
                value |= u64::from(*byte) << (8 * index);
            }
            Ok((value as i64, StepDisposition::Accepted))
        }
    }

    fn sys_irq_raise(&mut self, caller: usize, args: [u64; 4]) -> (i64, StepDisposition) {
        let device = match self.device_cap_check(caller, args[0], DEV_DRIVER_CONTROL) {
            Ok(device) => device,
            Err(code) => return self.deny(code),
        };
        let route = self.irq_routes[device];
        let dead = route.receiver >= MAX_TASKS
            || matches!(
                self.tasks[route.receiver].life,
                Life::Faulted | Life::Killed | Life::Empty
            );
        if dead {
            self.irq_routes[device].pending = false;
            self.evidence_events += 1;
            return (ERR_NO_SLOT, StepDisposition::SafeReject);
        }
        let ep = IRQ_ENDPOINTS[device] as usize;
        if let EpState::ReceiverWaiting { tid, cap } = self.endpoints[ep] {
            if tid == route.receiver && cap >= 1 {
                self.tasks[tid].life = Life::Ready;
                self.tasks[tid].pending_delivery = true;
                self.endpoints[ep] = EpState::Idle;
                return (OK, StepDisposition::Accepted);
            }
            // An unauthorized waiter never receives a device event; its
            // wait state is left untouched (docs/31 §9).
            self.evidence_events += 1;
            return (ERR_NO_SLOT, StepDisposition::SafeReject);
        }
        self.irq_routes[device].pending = true;
        (IRQ_PENDING, StepDisposition::Accepted)
    }
}

// ---- Tests --------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn invoke(caller: usize, number: u64, args: [u64; 4]) -> SyscallOp {
        SyscallOp::Invoke {
            caller,
            number,
            args,
        }
    }

    fn run(operations: &[SyscallOp]) -> ModelOutcome {
        run_once(operations).expect("model run must not fail an invariant")
    }

    fn assert_safe(operations: &[SyscallOp]) {
        let result = evaluate_operations(operations);
        assert_ne!(
            result.class,
            ResultClass::KernelInvariantFailure,
            "unexpected invariant failure: {}",
            result.reason
        );
    }

    // -- SYS-INV-001: inventory completeness and non-aliasing ------------

    #[test]
    fn inventory_matches_documented_contract() {
        assert_eq!(classify(0), SyscallClass::Invalid);
        assert_eq!(classify(1), SyscallClass::Implemented);
        assert_eq!(classify(4), SyscallClass::Implemented);
        assert_eq!(classify(5), SyscallClass::RecognizedNotImplemented);
        assert_eq!(classify(6), SyscallClass::RecognizedNotImplemented);
        assert_eq!(classify(7), SyscallClass::Implemented);
        assert_eq!(classify(20), SyscallClass::Implemented);
        assert_eq!(classify(21), SyscallClass::Invalid);
        assert_eq!(classify(u64::MAX), SyscallClass::Invalid);

        let implemented = SYSCALL_INVENTORY
            .iter()
            .filter(|(_, _, class)| *class == SyscallClass::Implemented)
            .count();
        let recognized = SYSCALL_INVENTORY
            .iter()
            .filter(|(_, _, class)| *class == SyscallClass::RecognizedNotImplemented)
            .count();
        assert_eq!(implemented, 18, "implemented numbers are exactly 1-4, 7-20");
        assert_eq!(recognized, 2, "recognized stubs are exactly 5 and 6");

        // Contiguity: 1..=20 recognized (implemented or stub), all other
        // probed numbers invalid. Adding a runtime syscall without
        // updating SYSCALL_INVENTORY breaks this relationship.
        for number in 1..=20u64 {
            assert_ne!(classify(number), SyscallClass::Invalid, "number {number}");
        }
        for number in 21..=4096u64 {
            assert_eq!(classify(number), SyscallClass::Invalid, "number {number}");
        }
        for number in [
            u64::from(u8::MAX),
            u64::from(u16::MAX),
            u64::from(u32::MAX),
            u64::MAX,
        ] {
            assert_eq!(classify(number), SyscallClass::Invalid, "number {number}");
        }
    }

    #[test]
    fn every_inventory_entry_is_deterministically_unique() {
        for (index, (number, name, _)) in SYSCALL_INVENTORY.iter().enumerate() {
            for (other_number, other_name, _) in SYSCALL_INVENTORY.iter().skip(index + 1) {
                assert_ne!(number, other_number, "duplicate number {number}");
                assert_ne!(name, other_name, "duplicate name {name}");
            }
        }
    }

    // -- SYS-INV-002/003: stubs and invalid numbers mutate nothing -------

    #[test]
    fn stubs_5_and_6_return_not_implemented_without_mutation() {
        for number in [5u64, 6] {
            for args in [
                [0u64, 0, 0, 0],
                [u64::MAX, u64::MAX, u64::MAX, u64::MAX],
                [0, VALID_VA, 128, 3],
            ] {
                let outcome = run(&[invoke(4, number, args)]);
                let baseline = run(&[]);
                assert_eq!(
                    outcome.protected, baseline.protected,
                    "stub {number} mutated protected state"
                );
                assert_eq!(outcome.counts.accepted, 0, "stub {number} was accepted");
                assert_eq!(outcome.counts.safe_rejects, 1);
            }
        }
    }

    #[test]
    fn stub_results_are_deterministic_across_repeats() {
        let ops = [
            invoke(4, 5, [1, 2, 3, 4]),
            invoke(4, 5, [1, 2, 3, 4]),
            invoke(4, 6, [u64::MAX, 0, 0, 0]),
            invoke(4, 6, [u64::MAX, 0, 0, 0]),
        ];
        let first = run(&ops);
        let second = run(&ops);
        assert_eq!(first, second);
        assert_eq!(first.counts.safe_rejects, 4);
    }

    #[test]
    fn invalid_numbers_are_rejected_without_mutation() {
        let baseline = run(&[]);
        for number in [0u64, 21, 32, 255, 65535, u64::from(u32::MAX), u64::MAX] {
            let outcome = run(&[invoke(4, number, [u64::MAX, u64::MAX, u64::MAX, u64::MAX])]);
            assert_eq!(
                outcome.protected, baseline.protected,
                "invalid number {number} mutated protected state"
            );
            assert_eq!(outcome.counts.accepted, 0);
        }
    }

    // -- SYS-INV-004: capability slot boundaries -------------------------

    #[test]
    fn cap_slot_boundaries_cannot_grant_authority() {
        let baseline = run(&[]);
        for slot in [
            RUNTIME_CAP_SLOTS as u64,
            (RUNTIME_CAP_SLOTS + 1) as u64,
            u64::from(u8::MAX),
            u64::MAX,
        ] {
            let outcome = run(&[invoke(4, 3, [slot, VALID_VA, 8, 0])]);
            assert_eq!(
                outcome.protected, baseline.protected,
                "slot {slot} granted authority"
            );
        }
        // Last valid slot works only when occupied.
        let occupied = run(&[
            SyscallOp::MintCap {
                task: 4,
                slot: RUNTIME_CAP_SLOTS - 1,
                profile: MintProfile::EndpointSendRecv0,
            },
            invoke(4, 3, [(RUNTIME_CAP_SLOTS - 1) as u64, VALID_VA, 8, 0]),
        ]);
        assert_eq!(occupied.counts.accepted, 2);
        let empty = run(&[invoke(
            4,
            3,
            [(RUNTIME_CAP_SLOTS - 1) as u64, VALID_VA, 8, 0],
        )]);
        assert_eq!(empty.counts.accepted, 0);
    }

    #[test]
    fn revoked_wrong_type_and_wrong_rights_slots_are_rejected() {
        // Revoked.
        assert_eq!(
            run(&[
                SyscallOp::RevokeCap { task: 4, slot: 0 },
                invoke(4, 3, [0, VALID_VA, 8, 0]),
            ])
            .counts
            .safe_rejects,
            1
        );
        // Wrong type (console as endpoint).
        assert_eq!(
            run(&[
                SyscallOp::MintCap {
                    task: 4,
                    slot: 2,
                    profile: MintProfile::ConsoleSendRecv,
                },
                invoke(4, 3, [2, VALID_VA, 8, 0]),
            ])
            .counts
            .safe_rejects,
            1
        );
        // Wrong rights (send-only used for recv).
        assert_eq!(
            run(&[invoke(4, 4, [0, VALID_VA, 8, 0])])
                .counts
                .safe_rejects,
            1
        );
        // Zero rights.
        assert_eq!(
            run(&[
                SyscallOp::MintCap {
                    task: 4,
                    slot: 2,
                    profile: MintProfile::EndpointZeroRights,
                },
                invoke(4, 3, [2, VALID_VA, 8, 0]),
            ])
            .counts
            .safe_rejects,
            1
        );
    }

    // -- SYS-INV-005: endpoint and index bounds before array access ------

    #[test]
    fn out_of_range_endpoint_caps_never_reach_endpoint_state() {
        let baseline = run(&[]);
        for profile in [
            MintProfile::EndpointOutOfRange12,
            MintProfile::EndpointOutOfRangeMax,
        ] {
            let outcome = run(&[
                SyscallOp::MintCap {
                    task: 4,
                    slot: 2,
                    profile,
                },
                invoke(4, 3, [2, VALID_VA, 8, 0]),
                invoke(4, 4, [2, VALID_VA, 8, 0]),
            ]);
            assert_eq!(outcome.protected.endpoints, baseline.protected.endpoints);
            assert_eq!(outcome.counts.safe_rejects, 2);
        }
    }

    #[test]
    fn endpoint_boundary_ids_0_and_11_work_when_minted() {
        let outcome = run(&[
            invoke(4, 3, [0, VALID_VA, 8, 0]),
            invoke(15, 3, [0, VALID_VA, 8, 0]),
        ]);
        // Both sends block on idle endpoints 0 and 11: accepted.
        assert_eq!(outcome.counts.accepted, 2);
        assert!(matches!(
            outcome.protected.endpoints[0],
            EpState::SenderWaiting { tid: 4, len: 8 }
        ));
        assert!(matches!(
            outcome.protected.endpoints[NUM_ENDPOINTS - 1],
            EpState::SenderWaiting { tid: 15, len: 8 }
        ));
    }

    // -- SYS-INV-006/007/008: scalar, length, and checked arithmetic -----

    #[test]
    fn oversized_and_overflowing_lengths_are_rejected_before_copy() {
        let baseline = run(&[]);
        for (va, len) in [
            (VALID_VA, (IPC_MSG_MAX + 1) as u64),
            (VALID_VA, u64::MAX),
            (u64::MAX, 8),
            (u64::MAX, u64::MAX),
            (USER_DATA_END - 1, 2),
            (USER_DATA_END, 1),
            (KERNEL_BASE_VA, 8),
            (0, 8),
            (0, 0),
        ] {
            let outcome = run(&[invoke(4, 3, [0, va, len, 0])]);
            assert_eq!(
                outcome.protected, baseline.protected,
                "send va={va:#x} len={len} mutated state"
            );
        }
        // Zero length at a valid address is legal (bounded, no copy).
        let ok = run(&[invoke(4, 3, [0, VALID_VA, 0, 0])]);
        assert_eq!(ok.counts.accepted, 1);
        // Exact maximum at the last fitting address is legal.
        let max = run(&[invoke(
            4,
            3,
            [0, USER_DATA_END - IPC_MSG_MAX as u64, IPC_MSG_MAX as u64, 0],
        )]);
        assert_eq!(max.counts.accepted, 1);
    }

    // -- SYS-INV-009/013: lifecycle validation ---------------------------

    #[test]
    fn task_boundaries_kill_and_restart() {
        // Valid extremes.
        assert_eq!(run(&[invoke(3, 12, [0, 0, 0, 0])]).counts.accepted, 1);
        assert_eq!(
            run(&[invoke(3, 12, [(MAX_TASKS - 1) as u64, 0, 0, 0])])
                .counts
                .accepted,
            1
        );
        // Out of range / empty / self / non-restartable.
        let baseline = run(&[]);
        for (number, slot) in [
            (12u64, MAX_TASKS as u64),
            (12, u64::MAX),
            (12, 10),
            (13, MAX_TASKS as u64),
            (13, u64::MAX),
            (13, 3),
            (13, 8),
            (13, 10),
        ] {
            let outcome = run(&[invoke(3, number, [slot, 0, 0, 0])]);
            assert_eq!(
                outcome.protected, baseline.protected,
                "syscall {number} slot {slot} mutated state"
            );
        }
        // Faulted task 9 is restartable.
        assert_eq!(run(&[invoke(3, 13, [9, 0, 0, 0])]).counts.accepted, 1);
    }

    #[test]
    fn kill_and_restart_clear_only_the_target_task() {
        let outcome = run(&[
            invoke(4, 3, [0, VALID_VA, 8, 0]), // task 4 blocks on ep 0
            invoke(3, 12, [4, 0, 0, 0]),       // kill it
        ]);
        assert_eq!(outcome.protected.tasks[4].life, Life::Killed);
        assert!(matches!(outcome.protected.endpoints[0], EpState::Idle));
        // Unrelated tasks unchanged.
        let baseline = run(&[]);
        for slot in [0usize, 1, 3, 5, 6, 7, 15] {
            assert_eq!(
                outcome.protected.tasks[slot],
                baseline.protected.tasks[slot]
            );
        }
    }

    #[test]
    fn repeated_kill_and_restart_are_deterministic() {
        assert_safe(&[
            invoke(3, 12, [7, 0, 0, 0]),
            invoke(3, 12, [7, 0, 0, 0]),
            invoke(3, 13, [7, 0, 0, 0]),
            invoke(3, 13, [7, 0, 0, 0]),
        ]);
    }

    #[test]
    fn service_start_boundaries() {
        // The last service index resolves to live slot 15: bounded-busy,
        // never an out-of-range error.
        assert_eq!(
            run(&[invoke(3, 8, [(NUM_SERVICES - 1) as u64, 0, 0, 0])])
                .counts
                .busy,
            1
        );
        // An empty slot (service 13 -> slot 14) arms and starts.
        assert_eq!(run(&[invoke(3, 8, [13, 0, 0, 0])]).counts.accepted, 1);
        let busy = run(&[invoke(3, 8, [0, 0, 0, 0]), invoke(3, 8, [0, 0, 0, 0])]);
        assert_eq!(busy.counts.busy, 2, "start of a live slot is bounded-busy");
        let baseline = run(&[]);
        for index in [NUM_SERVICES as u64, u64::from(u8::MAX), u64::MAX] {
            let outcome = run(&[invoke(3, 8, [index, 0, 0, 0])]);
            assert_eq!(outcome.protected, baseline.protected);
        }
    }

    // -- SYS-INV-010: authority checks precede protected operations ------

    #[test]
    fn missing_task_authority_is_denied_before_any_validation() {
        let baseline = run(&[]);
        // Task 4 has no control/console/info authority; even wildly
        // invalid arguments must produce the capability denial only.
        for (number, args) in [
            (8u64, [u64::MAX, 0, 0, 0]),
            (9, [u64::MAX, u64::MAX, 0, 0]),
            (10, [0, u64::MAX, 0, 0]),
            (11, [u64::MAX, 0, u64::MAX, u64::MAX]),
            (12, [u64::MAX, 0, 0, 0]),
            (13, [u64::MAX, 0, 0, 0]),
            (14, [0, 0, 0, 0]),
            (15, [u64::MAX, 0, 0, 0]),
            (16, [0, u64::MAX, u64::MAX, 0]),
            (17, [0, 0, 0, 0]),
            (18, [0, u64::MAX, 0, 0]),
            (19, [0, 0, 0, 0]),
            (20, [0, 0, 0, 0]),
        ] {
            let outcome = run(&[invoke(4, number, args)]);
            assert_eq!(
                outcome.protected, baseline.protected,
                "unauthorized syscall {number} mutated state"
            );
            assert_eq!(outcome.counts.accepted, 0, "syscall {number} was accepted");
        }
    }

    // -- SYS-INV-011: MMIO width/alignment/bounds ------------------------

    #[test]
    fn mmio_boundary_matrix() {
        // Valid: first, last aligned per width.
        for (offset, width) in [
            (0u64, 1u64),
            (0, 2),
            (0, 4),
            (MMIO0_SIZE - 4, 4),
            (MMIO0_SIZE - 2, 2),
            (MMIO0_SIZE - 1, 1),
        ] {
            let outcome = run(&[invoke(6, 16, [0, offset, width, 0])]);
            assert_eq!(
                outcome.counts.accepted, 1,
                "offset {offset:#x} width {width} must be legal"
            );
        }
        // Invalid: misaligned, boundary-crossing, out of range, bad width.
        let baseline = run(&[]);
        for (offset, width) in [
            (1u64, 2u64),
            (1, 4),
            (2, 4),
            (MMIO0_SIZE - 2, 4),
            (MMIO0_SIZE - 1, 2),
            (MMIO0_SIZE, 1),
            (MMIO0_SIZE + 1, 1),
            (u64::MAX, 1),
            (u64::MAX - 3, 4),
            (0, 0),
            (0, 3),
            (0, 8),
            (0, 16),
            (0, u64::from(u8::MAX)),
            (0, u64::MAX),
        ] {
            let outcome = run(&[invoke(6, 17, [0, offset, width, 0xff])]);
            assert_eq!(
                outcome.protected.mmio, baseline.protected.mmio,
                "offset {offset:#x} width {width} reached MMIO"
            );
            assert_eq!(outcome.counts.accepted, 0);
        }
        // net0 has a zero-size window: everything rejects.
        let outcome = run(&[
            SyscallOp::MintCap {
                task: 6,
                slot: 2,
                profile: MintProfile::DeviceNet,
            },
            invoke(6, 16, [2, 0, 1, 0]),
        ]);
        assert_eq!(outcome.counts.safe_rejects, 1);
    }

    #[test]
    fn mmio_write_reaches_only_validated_bytes() {
        let outcome = run(&[
            invoke(6, 17, [0, 8, 4, 0xdead_beef]),
            invoke(6, 16, [0, 8, 4, 0]),
        ]);
        assert_eq!(&outcome.protected.mmio[8..12], &[0xef, 0xbe, 0xad, 0xde]);
        assert!(outcome.protected.mmio[..8].iter().all(|byte| *byte == 0));
        assert!(outcome.protected.mmio[12..].iter().all(|byte| *byte == 0));
    }

    // -- SYS-INV-012: DMA width/bounds/direction -------------------------

    #[test]
    fn dma_boundary_matrix() {
        for (offset, width) in [
            (0u64, 1u64),
            (0, 2),
            (0, 4),
            (DMA0_SIZE - 4, 4),
            (DMA0_SIZE - 1, 1),
        ] {
            let outcome = run(&[invoke(6, 18, [0, offset, width, 0])]);
            assert_eq!(outcome.counts.accepted, 1, "offset {offset} width {width}");
        }
        let baseline = run(&[]);
        for (offset, width) in [
            (DMA0_SIZE - 2, 4u64),
            (DMA0_SIZE - 1, 2),
            (DMA0_SIZE, 1),
            (DMA0_SIZE + 1, 1),
            (u64::MAX - 3, 4),
            (u64::MAX, 1),
            (0, 0),
            (0, 8),
            (0, u64::MAX),
        ] {
            let outcome = run(&[invoke(6, 19, [0, offset, width, 0xff])]);
            assert_eq!(
                outcome.protected.dma, baseline.protected.dma,
                "offset {offset:#x} width {width} reached DMA"
            );
        }
    }

    #[test]
    fn dma_and_mmio_direction_mismatch_is_denied() {
        let baseline = run(&[]);
        let outcome = run(&[
            SyscallOp::MintCap {
                task: 5,
                slot: 2,
                profile: MintProfile::DeviceBlockReadOnly,
            },
            invoke(5, 19, [2, 0, 1, 1]),
            invoke(5, 17, [2, 0, 4, 1]),
        ]);
        assert_eq!(outcome.protected.dma, baseline.protected.dma);
        assert_eq!(outcome.protected.mmio, baseline.protected.mmio);
        assert_eq!(outcome.counts.safe_rejects, 2);
    }

    // -- fault_ack resolved contract (SYS-INV-017/018) -------------------

    #[test]
    fn fault_ack_requires_fault_channel_control_authority() {
        // Supervisor (task 1) holds the fault channel Control right.
        let authorized = run(&[invoke(1, 7, [0, 2, 0, 0])]);
        assert_eq!(authorized.counts.accepted, 1);
        assert_eq!(authorized.recovery_acks, 1);

        // No other boot profile may acknowledge; no recovery evidence.
        let baseline = run(&[]);
        for caller in [0usize, 3, 4, 5, 6, 7, 15] {
            let outcome = run(&[invoke(caller, 7, [0, 2, 0, 0])]);
            assert_eq!(outcome.recovery_acks, 0, "task {caller} forged an ack");
            assert_eq!(outcome.counts.accepted, 0);
            assert_eq!(
                outcome.protected, baseline.protected,
                "unauthorized ack by task {caller} mutated state"
            );
        }

        // A recv-only fault channel capability is not enough.
        let recv_only = run(&[
            SyscallOp::MintCap {
                task: 4,
                slot: 3,
                profile: MintProfile::FaultChannelRecvOnly,
            },
            invoke(4, 7, [0, 2, 0, 0]),
        ]);
        assert_eq!(recv_only.recovery_acks, 0);

        // A minted Control-bearing fault channel capability suffices.
        let minted = run(&[
            SyscallOp::MintCap {
                task: 4,
                slot: 3,
                profile: MintProfile::FaultChannelControl,
            },
            invoke(4, 7, [0, 2, 0, 0]),
        ]);
        assert_eq!(minted.recovery_acks, 1);
    }

    #[test]
    fn fault_ack_is_record_only_for_every_decision_value() {
        let baseline = run(&[]);
        for decision in [0u64, 1, 2, 3, 7, u64::from(u8::MAX), u64::MAX] {
            let outcome = run(&[invoke(1, 7, [0, decision, 0, 0])]);
            assert_eq!(outcome.recovery_acks, 1, "decision {decision}");
            assert_eq!(
                outcome.protected, baseline.protected,
                "decision {decision} mutated protected state"
            );
        }
    }

    // -- SYS-INV-014/015: rejection leaves state; determinism ------------

    #[test]
    fn rejected_syscalls_after_accepted_prefix_leave_state_unchanged() {
        let prefix = [
            invoke(6, 17, [0, 0, 4, 0x1234_5678]),
            invoke(6, 19, [0, 8, 2, 0xbeef]),
            invoke(4, 3, [0, VALID_VA, 8, 0]),
        ];
        let with_rejects: Vec<SyscallOp> = prefix
            .iter()
            .cloned()
            .chain([
                invoke(6, 17, [0, u64::MAX, 4, 0]),
                invoke(6, 19, [0, 0, 8, 0]),
                invoke(3, 12, [u64::MAX, 0, 0, 0]),
                invoke(7, 4, [0, KERNEL_BASE_VA, 8, 0]),
                invoke(4, 42, [0, 0, 0, 0]),
            ])
            .collect();
        let clean = run(&prefix);
        let noisy = run(&with_rejects);
        assert_eq!(clean.protected, noisy.protected);
    }

    #[test]
    fn identical_streams_produce_identical_outcomes() {
        let ops = decode_operations(3, &[7, 1, 200, 13, 44, 5, 0, 9, 250, 3, 3, 3]);
        assert!(ops.len() <= MAX_SYSCALL_OPS_PER_CASE);
        assert_eq!(run(&ops), run(&ops));
        assert_eq!(
            decode_operations(3, &[7, 1, 200, 13, 44, 5, 0, 9, 250, 3, 3, 3]),
            ops,
            "decoding must be deterministic"
        );
    }

    #[test]
    fn evaluate_operations_enforces_the_op_bound() {
        let ops: Vec<SyscallOp> = (0..MAX_SYSCALL_OPS_PER_CASE + 1)
            .map(|_| invoke(4, 1, [0, 0, 0, 0]))
            .collect();
        let result = evaluate_operations(&ops);
        assert_eq!(result.class, ResultClass::KernelInvariantFailure);
    }

    // -- Bank integrity and full-input sweep -----------------------------

    #[test]
    fn boundary_bank_names_are_unique_and_covered() {
        for (index, name) in FIXED_SCENARIO_NAMES.iter().enumerate() {
            for other in FIXED_SCENARIO_NAMES.iter().skip(index + 1) {
                assert_ne!(name, other);
            }
        }
        for index in 0..MANDATORY_SCENARIOS {
            let ops = mandatory_scenario(index);
            assert!(!ops.is_empty(), "scenario {index} is empty");
            assert!(ops.len() <= MAX_SYSCALL_OPS_PER_CASE);
            let result = evaluate_operations(&ops);
            assert_ne!(
                result.class,
                ResultClass::KernelInvariantFailure,
                "scenario {} ({}) failed: {}",
                index,
                FIXED_SCENARIO_NAMES[index as usize],
                result.reason
            );
        }
    }

    #[test]
    fn every_opcode_and_boundary_selector_decodes_safely() {
        for opcode in 0..=u8::MAX {
            let input = [opcode, opcode.wrapping_mul(7), 255, 0, 128, 9];
            let ops = decode_operations(u64::from(opcode), &input);
            assert_safe(&ops);
        }
    }

    #[test]
    fn irq_raise_paths_are_bounded() {
        // Pending (driver alive, not waiting), then delivered via recv.
        let outcome = run(&[
            invoke(5, 20, [0, 0, 0, 0]),
            invoke(6, 4, [1, VALID_VA, 8, 0]),
        ]);
        assert!(!outcome.protected.irq_routes[0].pending);
        // Dead receiver: dropped with no wait-state change.
        let dead = run(&[invoke(3, 12, [6, 0, 0, 0]), invoke(5, 20, [0, 0, 0, 0])]);
        assert!(!dead.protected.irq_routes[0].pending);
    }
}
