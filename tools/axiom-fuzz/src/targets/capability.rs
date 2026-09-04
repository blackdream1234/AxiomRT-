//! Deterministic adversarial target for AxiomRT capability authority.
//!
//! Generic endpoint/task authority is checked through the real CapTable API.
//! Device authority is checked through the real, separately typed DeviceTable
//! API. A small adapter supplies per-task ownership and documented boot-state
//! restoration; it does not claim equivalence with the private RISC-V table.

use crate::{CaseResult, FuzzCase, FuzzTarget, ResultClass};
use kernel::caps::table::CapError;
use kernel::caps::{CapTable, Capability, ObjectRef, ObjectType, Rights, CAP_TABLE_SLOTS};
use kernel::device::{
    DeviceAccessError, DeviceCapability, DeviceId, DeviceKind, DeviceObject, DeviceRights,
    DeviceTable, DmaRegion, IrqLine, MmioRegion,
};
use kernel::ipc::{send_checked, Endpoint, EndpointId, EndpointState, IpcCapError, Message};
use kernel::thread::ThreadId;
use std::panic::{catch_unwind, AssertUnwindSafe};

pub const NAME: &str = "capability";
pub const MAX_CAP_OPS_PER_CASE: usize = 16;
pub const RUNTIME_CAP_SLOTS: usize = 9;

const TASK_COUNT: usize = 3;
const SERVICE_TASK: usize = 0;
const MANAGER_TASK: usize = 1;
const APP_TASK: usize = 2;
const ENDPOINT_ID: u32 = 7;
const WRONG_ENDPOINT_ID: u32 = 8;
const BLOCK_DEVICE_ID: u32 = 0;
const NET_DEVICE_ID: u32 = 1;
const NONEXISTENT_DEVICE_ID: u32 = 2;
const DYNAMIC_SLOT: usize = 2;
const TEST_SLOT: usize = 3;
const GENERIC_KNOWN_MASK: u16 = 0x00ff;
const DEVICE_KNOWN_MASK: u16 = 0x00ff;
const UNKNOWN_RIGHT_BIT: u16 = 1 << 15;

pub const FIXED_SCENARIO_NAMES: [&str; 35] = [
    "valid_lookup",
    "empty_slot",
    "invalid_slot",
    "runtime_max_valid_slot",
    "runtime_one_beyond",
    "host_max_valid_slot",
    "repeated_lookup",
    "clear_then_lookup",
    "revoked_cap",
    "double_revoke",
    "use_revoked_ipc_cap",
    "use_revoked_device_cap",
    "wrong_object_type",
    "wrong_control_type",
    "wrong_object_id",
    "wrong_endpoint",
    "wrong_device",
    "nonexistent_device",
    "missing_right",
    "zero_rights",
    "all_known_rights",
    "all_bits_set",
    "unknown_right_bits",
    "required_plus_unknown_bits",
    "derive_subset",
    "derive_equal",
    "derive_empty",
    "derive_superset_attempt",
    "derive_from_revoked",
    "cross_task_slot_reuse",
    "kill_fault_authority",
    "restart_stale_authority",
    "application_deny_by_default",
    "host_full_table",
    "valid_device",
];

const MANDATORY_SCENARIOS: u64 = FIXED_SCENARIO_NAMES.len() as u64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LookupRequest {
    EndpointSend,
    EndpointReceive,
    ThreadControl,
    EndpointAllKnown,
}

impl LookupRequest {
    fn parts(self) -> (ObjectType, Rights, u32) {
        match self {
            Self::EndpointSend => (ObjectType::Endpoint, Rights::SEND, ENDPOINT_ID),
            Self::EndpointReceive => (ObjectType::Endpoint, Rights::RECEIVE, ENDPOINT_ID),
            Self::ThreadControl => (ObjectType::Thread, Rights::CONTROL, MANAGER_TASK as u32),
            Self::EndpointAllKnown => (ObjectType::Endpoint, all_generic_rights(), ENDPOINT_ID),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceRequest {
    Info,
    MmioRead,
    DriverControl,
    NetworkDriver,
    AllKnown,
}

impl DeviceRequest {
    fn rights(self) -> DeviceRights {
        match self {
            Self::Info => DeviceRights::DEVICE_INFO,
            Self::MmioRead => DeviceRights::MMIO_READ,
            Self::DriverControl => DeviceRights::DRIVER_CONTROL,
            Self::NetworkDriver => DeviceRights::NETWORK_DRIVER,
            Self::AllKnown => all_device_rights(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeriveMode {
    Equal,
    StrictSubset,
    Empty,
    StrictSuperset,
    ArbitrarySuperset,
    RevokedParent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapProfile {
    EndpointSend,
    EndpointSendReceiveGrant,
    EndpointWrongId,
    ThreadControl,
    ThreadSend,
    EndpointZero,
    EndpointAllKnown,
    DeviceBlockRead,
    DeviceBlockControl,
    DeviceNet,
    DeviceZero,
    DeviceUnknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CapabilityOperation {
    Lookup {
        task: u8,
        slot: usize,
        request: LookupRequest,
    },
    LookupAfterRevoke {
        task: u8,
        slot: usize,
    },
    Derive {
        task: u8,
        mode: DeriveMode,
    },
    ReduceRights {
        task: u8,
        selector: u8,
    },
    AttemptAmplification {
        task: u8,
    },
    ReplaceSlot {
        task: u8,
        slot: usize,
        profile: CapProfile,
    },
    ClearSlot {
        task: u8,
        slot: usize,
    },
    CrossTaskUse {
        owner: u8,
        slot: usize,
    },
    WrongObjectType {
        task: u8,
    },
    WrongObjectId {
        task: u8,
    },
    WrongEndpoint {
        task: u8,
    },
    WrongDevice {
        task: u8,
        nonexistent: bool,
    },
    UnknownRightsBits {
        task: u8,
        include_required: bool,
    },
    ZeroRights {
        task: u8,
        device: bool,
    },
    AllBitsSet {
        task: u8,
        device: bool,
    },
    Revoke {
        task: u8,
        slot: usize,
    },
    DoubleRevoke {
        task: u8,
        slot: usize,
    },
    RestoreBootCap {
        task: u8,
    },
    UseAfterTaskRestart {
        task: u8,
        fault: bool,
    },
    KillTask {
        task: u8,
    },
    FaultTask {
        task: u8,
    },
    DeviceUse {
        task: u8,
        slot: usize,
        target: u32,
        request: DeviceRequest,
    },
    FillHostTable {
        task: u8,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StoredCap {
    Generic(Capability),
    Device(DeviceCapability),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Lifecycle {
    Alive,
    Killed,
    Faulted,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct OperationCounts {
    accepted: u64,
    safe_rejects: u64,
    resource_exhaustions: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TaskSnapshot {
    slots: [Option<StoredCap>; CAP_TABLE_SLOTS],
    lifecycle: Lifecycle,
    restart_count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ModelSnapshot {
    tasks: [TaskSnapshot; TASK_COUNT],
    target_mutations: u64,
    counts: OperationCounts,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ModelOutcome {
    snapshot: ModelSnapshot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StepDisposition {
    Accepted,
    SafeReject,
    ResourceExhaustion,
}

struct TaskCaps {
    table: CapTable,
    slots: [Option<StoredCap>; CAP_TABLE_SLOTS],
    boot: [Option<StoredCap>; RUNTIME_CAP_SLOTS],
    lifecycle: Lifecycle,
    restart_count: u64,
}

struct CapabilityModel {
    tasks: [TaskCaps; TASK_COUNT],
    target_mutations: u64,
    counts: OperationCounts,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CapabilityTarget;

impl FuzzTarget for CapabilityTarget {
    fn name(&self) -> &'static str {
        NAME
    }

    fn evaluate(&mut self, case: &FuzzCase) -> CaseResult {
        if case.target != NAME {
            return CaseResult::invariant_failure(
                "capability target received a case for another target",
            );
        }
        evaluate_operations(&decode_operations(case.iteration, &case.input))
    }
}

pub fn decode_operations(iteration: u64, input: &[u8]) -> Vec<CapabilityOperation> {
    let mut operations = mandatory_scenario(iteration % MANDATORY_SCENARIOS);
    for chunk in input.chunks(5) {
        if operations.len() == MAX_CAP_OPS_PER_CASE {
            break;
        }

        let opcode = chunk[0];
        let task = chunk.get(1).copied().unwrap_or(0) % TASK_COUNT as u8;
        let parameter = chunk.get(2).copied().unwrap_or(0);
        let extra = chunk.get(3).copied().unwrap_or(opcode);
        let flags = chunk.get(4).copied().unwrap_or(parameter);
        let slot = boundary_slot(parameter);
        let request = match extra % 4 {
            0 => LookupRequest::EndpointSend,
            1 => LookupRequest::EndpointReceive,
            2 => LookupRequest::ThreadControl,
            _ => LookupRequest::EndpointAllKnown,
        };
        let profile = match extra % 12 {
            0 => CapProfile::EndpointSend,
            1 => CapProfile::EndpointSendReceiveGrant,
            2 => CapProfile::EndpointWrongId,
            3 => CapProfile::ThreadControl,
            4 => CapProfile::ThreadSend,
            5 => CapProfile::EndpointZero,
            6 => CapProfile::EndpointAllKnown,
            7 => CapProfile::DeviceBlockRead,
            8 => CapProfile::DeviceBlockControl,
            9 => CapProfile::DeviceNet,
            10 => CapProfile::DeviceZero,
            _ => CapProfile::DeviceUnknown,
        };
        let mode = match flags % 6 {
            0 => DeriveMode::Equal,
            1 => DeriveMode::StrictSubset,
            2 => DeriveMode::Empty,
            3 => DeriveMode::StrictSuperset,
            4 => DeriveMode::ArbitrarySuperset,
            _ => DeriveMode::RevokedParent,
        };
        let device_request = match extra % 5 {
            0 => DeviceRequest::Info,
            1 => DeviceRequest::MmioRead,
            2 => DeviceRequest::DriverControl,
            3 => DeviceRequest::NetworkDriver,
            _ => DeviceRequest::AllKnown,
        };

        let operation = match opcode % 23 {
            0 => CapabilityOperation::Lookup {
                task,
                slot,
                request,
            },
            1 => CapabilityOperation::LookupAfterRevoke { task, slot },
            2 => CapabilityOperation::Derive { task, mode },
            3 => CapabilityOperation::ReduceRights {
                task,
                selector: extra,
            },
            4 => CapabilityOperation::AttemptAmplification { task },
            5 => CapabilityOperation::ReplaceSlot {
                task,
                slot,
                profile,
            },
            6 => CapabilityOperation::ClearSlot { task, slot },
            7 => CapabilityOperation::CrossTaskUse { owner: task, slot },
            8 => CapabilityOperation::WrongObjectType { task },
            9 => CapabilityOperation::WrongObjectId { task },
            10 => CapabilityOperation::WrongEndpoint { task },
            11 => CapabilityOperation::WrongDevice {
                task,
                nonexistent: flags & 1 != 0,
            },
            12 => CapabilityOperation::UnknownRightsBits {
                task,
                include_required: flags & 1 != 0,
            },
            13 => CapabilityOperation::ZeroRights {
                task,
                device: flags & 1 != 0,
            },
            14 => CapabilityOperation::AllBitsSet {
                task,
                device: flags & 1 != 0,
            },
            15 => CapabilityOperation::Revoke { task, slot },
            16 => CapabilityOperation::DoubleRevoke { task, slot },
            17 => CapabilityOperation::RestoreBootCap { task },
            18 => CapabilityOperation::UseAfterTaskRestart {
                task,
                fault: flags & 1 != 0,
            },
            19 => CapabilityOperation::KillTask { task },
            20 => CapabilityOperation::FaultTask { task },
            21 => CapabilityOperation::DeviceUse {
                task,
                slot,
                target: u32::from(flags % 4),
                request: device_request,
            },
            _ => CapabilityOperation::FillHostTable { task },
        };
        operations.push(operation);
    }
    operations
}

pub fn evaluate_operations(operations: &[CapabilityOperation]) -> CaseResult {
    if operations.len() > MAX_CAP_OPS_PER_CASE {
        return CaseResult::invariant_failure(format!(
            "capability operation count {} exceeds bound {MAX_CAP_OPS_PER_CASE}",
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
            "CAP-INV-014: identical capability streams produced different final states",
        );
    }

    if first.snapshot.counts.resource_exhaustions > 0 {
        CaseResult::new(
            ResultClass::BoundedResourceExhaustion,
            "generic host capability table reached its documented fixed capacity",
        )
    } else {
        CaseResult::safe_reject("capability sequence completed without an invariant failure")
    }
}

fn run_caught(operations: &[CapabilityOperation]) -> Result<ModelOutcome, String> {
    match catch_unwind(AssertUnwindSafe(|| run_once(operations))) {
        Ok(result) => result,
        Err(_) => Err(
            "CAP-INV-015: user-controlled capability input triggered a host-reachable panic"
                .to_string(),
        ),
    }
}

fn run_once(operations: &[CapabilityOperation]) -> Result<ModelOutcome, String> {
    let mut model = CapabilityModel::new()?;
    model.check_invariants()?;
    for operation in operations {
        let disposition = model.apply(operation)?;
        model.record(disposition);
        model.check_invariants()?;
    }
    Ok(ModelOutcome {
        snapshot: model.snapshot(),
    })
}

fn mandatory_scenario(index: u64) -> Vec<CapabilityOperation> {
    use CapabilityOperation::*;

    let lookup = |task, slot, request| Lookup {
        task,
        slot,
        request,
    };
    let replace = |task, slot, profile| ReplaceSlot {
        task,
        slot,
        profile,
    };

    match index {
        0 => vec![lookup(0, 0, LookupRequest::EndpointSend)],
        1 => vec![lookup(APP_TASK as u8, 0, LookupRequest::EndpointSend)],
        2 => vec![lookup(0, CAP_TABLE_SLOTS, LookupRequest::EndpointSend)],
        3 => vec![
            replace(0, RUNTIME_CAP_SLOTS - 1, CapProfile::EndpointSend),
            lookup(0, RUNTIME_CAP_SLOTS - 1, LookupRequest::EndpointSend),
        ],
        4 => vec![lookup(0, RUNTIME_CAP_SLOTS, LookupRequest::EndpointSend)],
        5 => vec![
            replace(0, CAP_TABLE_SLOTS - 1, CapProfile::EndpointSend),
            lookup(0, CAP_TABLE_SLOTS - 1, LookupRequest::EndpointSend),
        ],
        6 => vec![
            lookup(0, 0, LookupRequest::EndpointSend),
            lookup(0, 0, LookupRequest::EndpointSend),
        ],
        7 => vec![
            ClearSlot { task: 0, slot: 0 },
            lookup(0, 0, LookupRequest::EndpointSend),
        ],
        8 => vec![
            Revoke { task: 0, slot: 0 },
            lookup(0, 0, LookupRequest::EndpointSend),
        ],
        9 => vec![DoubleRevoke { task: 0, slot: 0 }],
        10 => vec![LookupAfterRevoke { task: 0, slot: 0 }],
        11 => vec![
            Revoke { task: 0, slot: 1 },
            DeviceUse {
                task: 0,
                slot: 1,
                target: BLOCK_DEVICE_ID,
                request: DeviceRequest::MmioRead,
            },
        ],
        12 => vec![WrongObjectType { task: 0 }],
        13 => vec![WrongObjectType {
            task: MANAGER_TASK as u8,
        }],
        14 => vec![WrongObjectId { task: 0 }],
        15 => vec![WrongEndpoint { task: 0 }],
        16 => vec![WrongDevice {
            task: 0,
            nonexistent: false,
        }],
        17 => vec![WrongDevice {
            task: 0,
            nonexistent: true,
        }],
        18 => vec![
            replace(0, TEST_SLOT, CapProfile::EndpointSend),
            lookup(0, TEST_SLOT, LookupRequest::EndpointReceive),
        ],
        19 => vec![
            ZeroRights {
                task: 0,
                device: false,
            },
            ZeroRights {
                task: 0,
                device: true,
            },
        ],
        20 => vec![
            replace(0, TEST_SLOT, CapProfile::EndpointAllKnown),
            lookup(0, TEST_SLOT, LookupRequest::EndpointAllKnown),
        ],
        21 => vec![AllBitsSet {
            task: 0,
            device: false,
        }],
        22 => vec![UnknownRightsBits {
            task: 0,
            include_required: false,
        }],
        23 => vec![UnknownRightsBits {
            task: 0,
            include_required: true,
        }],
        24 => vec![Derive {
            task: 0,
            mode: DeriveMode::StrictSubset,
        }],
        25 => vec![Derive {
            task: 0,
            mode: DeriveMode::Equal,
        }],
        26 => vec![Derive {
            task: 0,
            mode: DeriveMode::Empty,
        }],
        27 => vec![AttemptAmplification { task: 0 }],
        28 => vec![Derive {
            task: 0,
            mode: DeriveMode::RevokedParent,
        }],
        29 => vec![CrossTaskUse { owner: 0, slot: 0 }],
        30 => vec![
            KillTask { task: 0 },
            lookup(0, 0, LookupRequest::EndpointSend),
            RestoreBootCap { task: 0 },
            FaultTask { task: 0 },
            lookup(0, 0, LookupRequest::EndpointSend),
        ],
        31 => vec![UseAfterTaskRestart {
            task: 0,
            fault: true,
        }],
        32 => vec![
            RestoreBootCap {
                task: APP_TASK as u8,
            },
            lookup(APP_TASK as u8, 0, LookupRequest::EndpointSend),
            DeviceUse {
                task: APP_TASK as u8,
                slot: 0,
                target: BLOCK_DEVICE_ID,
                request: DeviceRequest::Info,
            },
        ],
        33 => vec![FillHostTable {
            task: MANAGER_TASK as u8,
        }],
        _ => vec![DeviceUse {
            task: SERVICE_TASK as u8,
            slot: 1,
            target: BLOCK_DEVICE_ID,
            request: DeviceRequest::MmioRead,
        }],
    }
}

fn boundary_slot(selector: u8) -> usize {
    match selector % 6 {
        0 => 0,
        1 => RUNTIME_CAP_SLOTS - 1,
        2 => RUNTIME_CAP_SLOTS,
        3 => CAP_TABLE_SLOTS - 1,
        4 => CAP_TABLE_SLOTS,
        _ => usize::MAX,
    }
}

impl CapabilityModel {
    fn new() -> Result<Self, String> {
        Ok(Self {
            tasks: [
                TaskCaps::new(SERVICE_TASK)?,
                TaskCaps::new(MANAGER_TASK)?,
                TaskCaps::new(APP_TASK)?,
            ],
            target_mutations: 0,
            counts: OperationCounts::default(),
        })
    }

    fn apply(&mut self, operation: &CapabilityOperation) -> Result<StepDisposition, String> {
        match *operation {
            CapabilityOperation::Lookup {
                task,
                slot,
                request,
            } => self.lookup(task_index(task), slot, request),
            CapabilityOperation::LookupAfterRevoke { task, slot } => {
                self.lookup_after_revoke(task_index(task), slot)
            }
            CapabilityOperation::Derive { task, mode } => self.derive(task_index(task), mode),
            CapabilityOperation::ReduceRights { task, selector } => {
                self.reduce_rights(task_index(task), selector)
            }
            CapabilityOperation::AttemptAmplification { task } => {
                self.derive(task_index(task), DeriveMode::StrictSuperset)
            }
            CapabilityOperation::ReplaceSlot {
                task,
                slot,
                profile,
            } => self.replace_slot(task_index(task), slot, profile),
            CapabilityOperation::ClearSlot { task, slot } => {
                self.clear_slot(task_index(task), slot)
            }
            CapabilityOperation::CrossTaskUse { owner, slot } => {
                self.cross_task_use(task_index(owner), slot)
            }
            CapabilityOperation::WrongObjectType { task } => {
                self.wrong_object_type(task_index(task))
            }
            CapabilityOperation::WrongObjectId { task } => self.wrong_object_id(task_index(task)),
            CapabilityOperation::WrongEndpoint { task } => self.wrong_endpoint(task_index(task)),
            CapabilityOperation::WrongDevice { task, nonexistent } => {
                self.wrong_device(task_index(task), nonexistent)
            }
            CapabilityOperation::UnknownRightsBits {
                task,
                include_required,
            } => self.unknown_rights(task_index(task), include_required),
            CapabilityOperation::ZeroRights { task, device } => {
                self.zero_rights(task_index(task), device)
            }
            CapabilityOperation::AllBitsSet { task, device } => {
                self.all_bits_set(task_index(task), device)
            }
            CapabilityOperation::Revoke { task, slot } => self.revoke(task_index(task), slot),
            CapabilityOperation::DoubleRevoke { task, slot } => {
                self.double_revoke(task_index(task), slot)
            }
            CapabilityOperation::RestoreBootCap { task } => self.restore_boot(task_index(task)),
            CapabilityOperation::UseAfterTaskRestart { task, fault } => {
                self.use_after_restart(task_index(task), fault)
            }
            CapabilityOperation::KillTask { task } => {
                self.terminal_task(task_index(task), Lifecycle::Killed)
            }
            CapabilityOperation::FaultTask { task } => {
                self.terminal_task(task_index(task), Lifecycle::Faulted)
            }
            CapabilityOperation::DeviceUse {
                task,
                slot,
                target,
                request,
            } => self.device_use(task_index(task), slot, target, request),
            CapabilityOperation::FillHostTable { task } => self.fill_host_table(task_index(task)),
        }
    }

    fn lookup(
        &self,
        task: usize,
        slot: usize,
        request: LookupRequest,
    ) -> Result<StepDisposition, String> {
        if self.tasks[task].lifecycle != Lifecycle::Alive {
            return Ok(StepDisposition::SafeReject);
        }
        let (expected_type, required, target_object) = request.parts();
        let actual = self.tasks[task].table.lookup(slot, expected_type, required);
        let expected = self.expected_lookup(task, slot, expected_type, required);
        if actual != expected {
            return Err(format!(
                "CAP-INV-001/002/003/005: lookup mismatch actual={actual:?} expected={expected:?}"
            ));
        }

        match actual {
            Ok(object) if object.object_id == target_object => Ok(StepDisposition::Accepted),
            Ok(_) | Err(_) => Ok(StepDisposition::SafeReject),
        }
    }

    fn expected_lookup(
        &self,
        task: usize,
        slot: usize,
        expected_type: ObjectType,
        required: Rights,
    ) -> Result<ObjectRef, CapError> {
        if slot >= CAP_TABLE_SLOTS {
            return Err(CapError::InvalidIndex);
        }
        match self.tasks[task].slots[slot] {
            None | Some(StoredCap::Device(_)) => Err(CapError::EmptySlot),
            Some(StoredCap::Generic(cap)) if cap.object().object_type != expected_type => {
                Err(CapError::WrongObjectType)
            }
            Some(StoredCap::Generic(cap)) if !cap.rights().contains(required) => {
                Err(CapError::InsufficientRights)
            }
            Some(StoredCap::Generic(cap)) => Ok(cap.object()),
        }
    }

    fn lookup_after_revoke(&mut self, task: usize, slot: usize) -> Result<StepDisposition, String> {
        let previous = if slot < CAP_TABLE_SLOTS {
            self.tasks[task].slots[slot]
        } else {
            None
        };
        self.tasks[task].clear(slot)?;
        match previous {
            Some(StoredCap::Generic(cap)) => {
                let result =
                    self.tasks[task]
                        .table
                        .lookup(slot, cap.object().object_type, cap.rights());
                let expected = if slot >= CAP_TABLE_SLOTS {
                    Err(CapError::InvalidIndex)
                } else {
                    Err(CapError::EmptySlot)
                };
                if result != expected {
                    return Err(
                        "CAP-INV-002/011: revoked generic capability authorized lookup".to_string(),
                    );
                }
                if cap.object().object_type == ObjectType::Endpoint {
                    let mut endpoint = Endpoint::new(EndpointId(cap.object().object_id));
                    let message = Message::new(ThreadId(1), b"revoked").map_err(|error| {
                        format!("CAP-INV-015: bounded revoked-cap message failed: {error:?}")
                    })?;
                    let actual = send_checked(
                        &self.tasks[task].table,
                        slot,
                        &mut endpoint,
                        ThreadId(1),
                        message,
                    );
                    if actual != Err(IpcCapError::Cap(CapError::EmptySlot))
                        || endpoint.state() != EndpointState::Idle
                    {
                        return Err(
                            "CAP-INV-002/010/011: revoked IPC cap authorized or mutated state"
                                .to_string(),
                        );
                    }
                }
            }
            Some(StoredCap::Device(cap)) => {
                let actual = device_table()
                    .check(None, cap.device(), cap.rights())
                    .map(|device| device.id);
                if actual != Err(DeviceAccessError::NoCapability) {
                    return Err(
                        "CAP-INV-002/011: revoked device capability authorized use".to_string()
                    );
                }
            }
            None => {}
        }
        Ok(StepDisposition::SafeReject)
    }

    fn derive(&mut self, task: usize, mode: DeriveMode) -> Result<StepDisposition, String> {
        if task == APP_TASK || self.tasks[task].lifecycle != Lifecycle::Alive {
            return Ok(StepDisposition::SafeReject);
        }
        if mode == DeriveMode::RevokedParent {
            self.tasks[task].clear(0)?;
        }

        let Some(StoredCap::Generic(parent)) = self.tasks[task].slots[0] else {
            return Ok(StepDisposition::SafeReject);
        };
        let parent_bits = parent.rights().bits();
        let requested_bits = match mode {
            DeriveMode::Equal => parent_bits,
            DeriveMode::StrictSubset => parent_bits & !lowest_set_bit(parent_bits),
            DeriveMode::Empty => 0,
            DeriveMode::StrictSuperset => parent_bits | first_missing_known_bit(parent_bits),
            DeriveMode::ArbitrarySuperset => {
                parent_bits | first_missing_known_bit(parent_bits) | UNKNOWN_RIGHT_BIT
            }
            DeriveMode::RevokedParent => unreachable!(),
        };

        let before_destination = self.tasks[task].slots[DYNAMIC_SLOT];
        let child = derive_requested(parent, requested_bits);
        if matches!(
            mode,
            DeriveMode::StrictSuperset | DeriveMode::ArbitrarySuperset
        ) {
            if child.is_some() || self.tasks[task].slots[DYNAMIC_SLOT] != before_destination {
                return Err("CAP-INV-007: attempted derivation increased authority".to_string());
            }
            return Ok(StepDisposition::SafeReject);
        }

        let child = child.ok_or_else(|| {
            "CAP-INV-007: equal/subset derivation was unexpectedly rejected".to_string()
        })?;
        if child.object() != parent.object() || child.rights().bits() & !parent.rights().bits() != 0
        {
            return Err("CAP-INV-007/008: derived capability amplified authority".to_string());
        }
        if mode == DeriveMode::StrictSubset && parent_bits != 0 && child.rights() == parent.rights()
        {
            return Err("CAP-INV-008: strict subset derivation did not reduce rights".to_string());
        }

        self.tasks[task].clear(DYNAMIC_SLOT)?;
        self.tasks[task].install(DYNAMIC_SLOT, StoredCap::Generic(child))?;
        Ok(StepDisposition::Accepted)
    }

    fn reduce_rights(&mut self, task: usize, selector: u8) -> Result<StepDisposition, String> {
        if task == APP_TASK || self.tasks[task].lifecycle != Lifecycle::Alive {
            return Ok(StepDisposition::SafeReject);
        }
        let Some(StoredCap::Generic(parent)) = self.tasks[task].slots[0] else {
            return Ok(StepDisposition::SafeReject);
        };
        let removed = 1u16 << (selector % 8);
        let requested = parent.rights().bits() & !removed;
        let child = derive_requested(parent, requested).ok_or_else(|| {
            "CAP-INV-007/008: rights reduction was unexpectedly rejected".to_string()
        })?;
        if child.rights().bits() & !parent.rights().bits() != 0 {
            return Err("CAP-INV-007/008: rights reduction amplified authority".to_string());
        }
        self.tasks[task].clear(DYNAMIC_SLOT)?;
        self.tasks[task].install(DYNAMIC_SLOT, StoredCap::Generic(child))?;
        Ok(StepDisposition::Accepted)
    }

    fn replace_slot(
        &mut self,
        task: usize,
        slot: usize,
        profile: CapProfile,
    ) -> Result<StepDisposition, String> {
        if task == APP_TASK || self.tasks[task].lifecycle != Lifecycle::Alive {
            return Ok(StepDisposition::SafeReject);
        }
        let authority = profile_cap(profile);
        if slot >= CAP_TABLE_SLOTS {
            if let StoredCap::Generic(cap) = authority {
                let actual = self.tasks[task].table.insert(slot, cap);
                if actual != Err(CapError::InvalidIndex) {
                    return Err("CAP-INV-001: invalid slot insertion did not fail".to_string());
                }
            }
            return Ok(StepDisposition::SafeReject);
        }

        self.tasks[task].clear(slot)?;
        self.tasks[task].install(slot, authority)?;
        Ok(StepDisposition::Accepted)
    }

    fn clear_slot(&mut self, task: usize, slot: usize) -> Result<StepDisposition, String> {
        if task == APP_TASK || self.tasks[task].lifecycle != Lifecycle::Alive {
            return Ok(StepDisposition::SafeReject);
        }
        if self.tasks[task].clear(slot)? {
            Ok(StepDisposition::Accepted)
        } else {
            Ok(StepDisposition::SafeReject)
        }
    }

    fn cross_task_use(&mut self, owner: usize, slot: usize) -> Result<StepDisposition, String> {
        let owner = owner % 2;
        if slot >= CAP_TABLE_SLOTS {
            return Ok(StepDisposition::SafeReject);
        }
        let Some(authority) = self.tasks[owner].slots[slot] else {
            return Ok(StepDisposition::SafeReject);
        };
        let before = self.target_mutations;
        match authority {
            StoredCap::Generic(cap) => {
                let actual =
                    self.tasks[APP_TASK]
                        .table
                        .lookup(slot, cap.object().object_type, cap.rights());
                if actual != Err(CapError::EmptySlot) {
                    return Err(
                        "CAP-INV-009: copied slot index inherited cross-task authority".to_string(),
                    );
                }
            }
            StoredCap::Device(cap) => {
                let actual = device_table()
                    .check(None, cap.device(), cap.rights())
                    .map(|device| device.id);
                if actual != Err(DeviceAccessError::NoCapability) {
                    return Err(
                        "CAP-INV-009: device slot index inherited cross-task authority".to_string(),
                    );
                }
            }
        }
        if self.target_mutations != before {
            return Err("CAP-INV-009/010: cross-task denial mutated target state".to_string());
        }
        Ok(StepDisposition::SafeReject)
    }

    fn wrong_object_type(&mut self, task: usize) -> Result<StepDisposition, String> {
        if task == APP_TASK || self.tasks[task].lifecycle != Lifecycle::Alive {
            return Ok(StepDisposition::SafeReject);
        }
        let (cap, expected_type, required) = if task == MANAGER_TASK {
            (
                endpoint_cap(ENDPOINT_ID, Rights::CONTROL),
                ObjectType::Thread,
                Rights::CONTROL,
            )
        } else {
            (
                Capability::new(
                    ObjectRef {
                        object_type: ObjectType::Thread,
                        object_id: task as u32,
                    },
                    Rights::SEND,
                ),
                ObjectType::Endpoint,
                Rights::SEND,
            )
        };
        self.tasks[task].clear(TEST_SLOT)?;
        self.tasks[task].install(TEST_SLOT, StoredCap::Generic(cap))?;
        let actual = self.tasks[task]
            .table
            .lookup(TEST_SLOT, expected_type, required);
        if actual != Err(CapError::WrongObjectType) {
            return Err("CAP-INV-003: wrong generic object type was accepted".to_string());
        }

        let before = self.target_mutations;
        let device_actual = device_table()
            .check(None, DeviceId(BLOCK_DEVICE_ID), DeviceRights::DEVICE_INFO)
            .map(|device| device.id);
        if device_actual != Err(DeviceAccessError::NoCapability) {
            return Err("CAP-INV-003: generic capability was used as a device cap".to_string());
        }

        self.tasks[task].clear(1)?;
        self.tasks[task].install(1, profile_cap(CapProfile::DeviceBlockRead))?;

        let mut endpoint = Endpoint::new(EndpointId(ENDPOINT_ID));
        let message = Message::new(ThreadId(1), b"type").map_err(|error| {
            format!("CAP-INV-015: bounded test message construction failed: {error:?}")
        })?;
        let ipc_actual = send_checked(
            &self.tasks[task].table,
            1,
            &mut endpoint,
            ThreadId(1),
            message,
        );
        if ipc_actual != Err(IpcCapError::Cap(CapError::EmptySlot))
            || endpoint.state() != EndpointState::Idle
        {
            return Err(
                "CAP-INV-003/010: device capability was accepted as endpoint authority".to_string(),
            );
        }
        if self.target_mutations != before {
            return Err("CAP-INV-010: wrong-type use mutated target state".to_string());
        }
        Ok(StepDisposition::SafeReject)
    }

    fn wrong_object_id(&mut self, task: usize) -> Result<StepDisposition, String> {
        if task == APP_TASK || self.tasks[task].lifecycle != Lifecycle::Alive {
            return Ok(StepDisposition::SafeReject);
        }
        self.tasks[task].clear(TEST_SLOT)?;
        self.tasks[task].install(
            TEST_SLOT,
            StoredCap::Generic(endpoint_cap(WRONG_ENDPOINT_ID, Rights::SEND)),
        )?;
        let before = self.target_mutations;
        let result = self.lookup(task, TEST_SLOT, LookupRequest::EndpointSend)?;
        if result != StepDisposition::SafeReject || self.target_mutations != before {
            return Err("CAP-INV-004/010: wrong object id authorized target use".to_string());
        }
        Ok(StepDisposition::SafeReject)
    }

    fn wrong_endpoint(&mut self, task: usize) -> Result<StepDisposition, String> {
        if task == APP_TASK || self.tasks[task].lifecycle != Lifecycle::Alive {
            return Ok(StepDisposition::SafeReject);
        }
        self.tasks[task].clear(TEST_SLOT)?;
        self.tasks[task].install(
            TEST_SLOT,
            StoredCap::Generic(endpoint_cap(WRONG_ENDPOINT_ID, Rights::SEND)),
        )?;

        let mut endpoint = Endpoint::new(EndpointId(ENDPOINT_ID));
        let message = Message::new(ThreadId(1), b"endpoint").map_err(|error| {
            format!("CAP-INV-015: bounded test message construction failed: {error:?}")
        })?;
        let actual = send_checked(
            &self.tasks[task].table,
            TEST_SLOT,
            &mut endpoint,
            ThreadId(1),
            message,
        );
        if actual != Err(IpcCapError::WrongEndpoint) || endpoint.state() != EndpointState::Idle {
            return Err(
                "CAP-INV-004/010: wrong endpoint authorized or mutated rendezvous state"
                    .to_string(),
            );
        }
        Ok(StepDisposition::SafeReject)
    }

    fn wrong_device(&mut self, task: usize, nonexistent: bool) -> Result<StepDisposition, String> {
        if task == APP_TASK || self.tasks[task].lifecycle != Lifecycle::Alive {
            return Ok(StepDisposition::SafeReject);
        }
        let profile = if nonexistent {
            CapProfile::DeviceUnknown
        } else {
            CapProfile::DeviceBlockRead
        };
        self.tasks[task].clear(TEST_SLOT)?;
        self.tasks[task].install(TEST_SLOT, profile_cap(profile))?;

        let target = if nonexistent {
            BLOCK_DEVICE_ID
        } else {
            NET_DEVICE_ID
        };
        let before = self.target_mutations;
        let result = self.device_use(task, TEST_SLOT, target, DeviceRequest::MmioRead)?;
        if result != StepDisposition::SafeReject || self.target_mutations != before {
            return Err("CAP-INV-004/010: wrong device authorized target use".to_string());
        }
        Ok(StepDisposition::SafeReject)
    }

    fn unknown_rights(
        &self,
        _task: usize,
        include_required: bool,
    ) -> Result<StepDisposition, String> {
        let raw = UNKNOWN_RIGHT_BIT
            | if include_required {
                Rights::SEND.bits()
            } else {
                0
            };
        if generic_rights_from_bits(raw).is_some() || device_rights_from_bits(raw).is_some() {
            return Err("CAP-INV-006: unknown rights bits entered a host capability".to_string());
        }

        let mut table = CapTable::new();
        table
            .insert(
                0,
                Capability::new(
                    ObjectRef {
                        object_type: ObjectType::Thread,
                        object_id: MANAGER_TASK as u32,
                    },
                    Rights::CONTROL,
                ),
            )
            .map_err(|error| format!("CAP-INV-001: local probe insert failed: {error:?}"))?;
        let actual = table.lookup(0, ObjectType::Thread, Rights::SEND);
        if actual != Err(CapError::InsufficientRights) {
            return Err(
                "CAP-INV-005/006: unknown bits could satisfy a missing known right".to_string(),
            );
        }
        Ok(StepDisposition::SafeReject)
    }

    fn zero_rights(&mut self, task: usize, device: bool) -> Result<StepDisposition, String> {
        if task == APP_TASK || self.tasks[task].lifecycle != Lifecycle::Alive {
            return Ok(StepDisposition::SafeReject);
        }
        let profile = if device {
            CapProfile::DeviceZero
        } else {
            CapProfile::EndpointZero
        };
        self.tasks[task].clear(TEST_SLOT)?;
        self.tasks[task].install(TEST_SLOT, profile_cap(profile))?;

        if device {
            self.device_use(task, TEST_SLOT, BLOCK_DEVICE_ID, DeviceRequest::Info)
        } else {
            self.lookup(task, TEST_SLOT, LookupRequest::EndpointSend)
        }
    }

    fn all_bits_set(&self, _task: usize, _device: bool) -> Result<StepDisposition, String> {
        if generic_rights_from_bits(u16::MAX).is_some()
            || device_rights_from_bits(u16::MAX).is_some()
        {
            return Err("CAP-INV-006/007: all-bits-set authority was constructible".to_string());
        }
        Ok(StepDisposition::SafeReject)
    }

    fn revoke(&mut self, task: usize, slot: usize) -> Result<StepDisposition, String> {
        if task == APP_TASK || self.tasks[task].lifecycle != Lifecycle::Alive {
            return Ok(StepDisposition::SafeReject);
        }
        let previous = if slot < CAP_TABLE_SLOTS {
            self.tasks[task].slots[slot]
        } else {
            None
        };
        let removed = self.tasks[task].clear(slot)?;
        if removed {
            self.verify_revoked(task, slot, previous)?;
            Ok(StepDisposition::Accepted)
        } else {
            Ok(StepDisposition::SafeReject)
        }
    }

    fn double_revoke(&mut self, task: usize, slot: usize) -> Result<StepDisposition, String> {
        if task == APP_TASK || self.tasks[task].lifecycle != Lifecycle::Alive {
            return Ok(StepDisposition::SafeReject);
        }
        let first = self.tasks[task].clear(slot)?;
        let second = self.tasks[task].clear(slot)?;
        if second {
            return Err("CAP-INV-002/011: double revoke removed authority twice".to_string());
        }
        if first {
            Ok(StepDisposition::Accepted)
        } else {
            Ok(StepDisposition::SafeReject)
        }
    }

    fn verify_revoked(
        &self,
        task: usize,
        slot: usize,
        previous: Option<StoredCap>,
    ) -> Result<(), String> {
        match previous {
            Some(StoredCap::Generic(cap)) => {
                let actual =
                    self.tasks[task]
                        .table
                        .lookup(slot, cap.object().object_type, cap.rights());
                if actual != Err(CapError::EmptySlot) {
                    return Err(
                        "CAP-INV-002/011: revocation was not immediately effective".to_string()
                    );
                }
            }
            Some(StoredCap::Device(cap)) => {
                let actual = device_table()
                    .check(None, cap.device(), cap.rights())
                    .map(|device| device.id);
                if actual != Err(DeviceAccessError::NoCapability) {
                    return Err(
                        "CAP-INV-002/011: device revocation was not immediately effective"
                            .to_string(),
                    );
                }
            }
            None => {}
        }
        Ok(())
    }

    fn restore_boot(&mut self, task: usize) -> Result<StepDisposition, String> {
        self.tasks[task].restart()?;
        self.verify_boot_exact(task)?;
        Ok(StepDisposition::Accepted)
    }

    fn use_after_restart(&mut self, task: usize, fault: bool) -> Result<StepDisposition, String> {
        if task != APP_TASK {
            let Some(StoredCap::Generic(parent)) = self.tasks[task].slots[0] else {
                return Ok(StepDisposition::SafeReject);
            };
            let requested = parent.rights().bits() & !lowest_set_bit(parent.rights().bits());
            let child = derive_requested(parent, requested).ok_or_else(|| {
                "CAP-INV-007: could not create pre-restart diminished cap".to_string()
            })?;
            self.tasks[task].clear(DYNAMIC_SLOT)?;
            self.tasks[task].install(DYNAMIC_SLOT, StoredCap::Generic(child))?;
        }

        self.tasks[task].lifecycle = if fault {
            Lifecycle::Faulted
        } else {
            Lifecycle::Killed
        };
        let before = self.target_mutations;
        let terminal = if task == MANAGER_TASK {
            self.lookup(task, 0, LookupRequest::ThreadControl)?
        } else {
            self.lookup(task, 0, LookupRequest::EndpointSend)?
        };
        if terminal != StepDisposition::SafeReject || self.target_mutations != before {
            return Err("CAP-INV-012: terminal task retained usable authority".to_string());
        }

        self.tasks[task].restart()?;
        self.verify_boot_exact(task)?;
        if self.tasks[task].slots[DYNAMIC_SLOT].is_some() {
            return Err(
                "CAP-INV-012: stale dynamically derived authority survived restart".to_string(),
            );
        }
        self.tasks[task].restart()?;
        self.verify_boot_exact(task)?;
        Ok(StepDisposition::Accepted)
    }

    fn terminal_task(
        &mut self,
        task: usize,
        terminal: Lifecycle,
    ) -> Result<StepDisposition, String> {
        if self.tasks[task].lifecycle != Lifecycle::Alive {
            return Ok(StepDisposition::SafeReject);
        }
        self.tasks[task].lifecycle = terminal;
        let before = self.target_mutations;
        let result = if task == MANAGER_TASK {
            self.lookup(task, 0, LookupRequest::ThreadControl)?
        } else {
            self.lookup(task, 0, LookupRequest::EndpointSend)?
        };
        if result != StepDisposition::SafeReject || self.target_mutations != before {
            return Err(
                "CAP-INV-012: killed/faulted task used retained capability state".to_string(),
            );
        }
        Ok(StepDisposition::Accepted)
    }

    fn device_use(
        &mut self,
        task: usize,
        slot: usize,
        target: u32,
        request: DeviceRequest,
    ) -> Result<StepDisposition, String> {
        if self.tasks[task].lifecycle != Lifecycle::Alive {
            return Ok(StepDisposition::SafeReject);
        }
        let held = if slot < CAP_TABLE_SLOTS {
            self.tasks[task].slots[slot]
        } else {
            None
        };
        let cap = match held {
            Some(StoredCap::Device(cap)) => Some(cap),
            None | Some(StoredCap::Generic(_)) => None,
        };
        let required = request.rights();
        let target_id = DeviceId(target);
        let actual = device_table()
            .check(cap.as_ref(), target_id, required)
            .map(|device| device.id);
        let expected = expected_device_check(cap, target_id, required);
        if actual != expected {
            return Err(format!(
                "CAP-INV-001/002/003/004/005: device check mismatch actual={actual:?} expected={expected:?}"
            ));
        }
        match actual {
            Ok(_) => {
                self.target_mutations += 1;
                Ok(StepDisposition::Accepted)
            }
            Err(_) => Ok(StepDisposition::SafeReject),
        }
    }

    fn fill_host_table(&mut self, task: usize) -> Result<StepDisposition, String> {
        if task == APP_TASK || self.tasks[task].lifecycle != Lifecycle::Alive {
            return Ok(StepDisposition::SafeReject);
        }
        self.tasks[task].clear_all()?;
        for slot in 0..CAP_TABLE_SLOTS {
            self.tasks[task].install(
                slot,
                StoredCap::Generic(endpoint_cap(ENDPOINT_ID, Rights::SEND)),
            )?;
        }
        let last =
            self.tasks[task]
                .table
                .lookup(CAP_TABLE_SLOTS - 1, ObjectType::Endpoint, Rights::SEND);
        if last
            != Ok(ObjectRef {
                object_type: ObjectType::Endpoint,
                object_id: ENDPOINT_ID,
            })
        {
            return Err("CAP-INV-001: maximum host slot did not resolve".to_string());
        }
        let overflow = self.tasks[task]
            .table
            .insert(CAP_TABLE_SLOTS, endpoint_cap(ENDPOINT_ID, Rights::SEND));
        if overflow != Err(CapError::InvalidIndex) {
            return Err("CAP-INV-001: one-beyond host slot was accepted".to_string());
        }
        Ok(StepDisposition::ResourceExhaustion)
    }

    fn verify_boot_exact(&self, task: usize) -> Result<(), String> {
        for slot in 0..CAP_TABLE_SLOTS {
            let expected = if slot < RUNTIME_CAP_SLOTS {
                self.tasks[task].boot[slot]
            } else {
                None
            };
            if self.tasks[task].slots[slot] != expected {
                return Err(format!(
                    "CAP-INV-012/013: restart slot {slot} exceeded boot-defined authority"
                ));
            }
        }
        Ok(())
    }

    fn check_invariants(&self) -> Result<(), String> {
        for (task_index, task) in self.tasks.iter().enumerate() {
            for slot in 0..CAP_TABLE_SLOTS {
                match task.slots[slot] {
                    Some(StoredCap::Generic(cap)) => {
                        if cap.rights().bits() & !GENERIC_KNOWN_MASK != 0 {
                            return Err(
                                "CAP-INV-006: stored generic cap contains unknown bits".to_string()
                            );
                        }
                        let query = task.table.query(slot);
                        if query != Ok((cap.object().object_type, cap.rights())) {
                            return Err(format!(
                                "CAP-INV-001/002: task {task_index} slot {slot} diverged from CapTable"
                            ));
                        }
                        let lookup =
                            task.table
                                .lookup(slot, cap.object().object_type, Rights::NONE);
                        if lookup != Ok(cap.object()) {
                            return Err(
                                "CAP-INV-003/004: held generic cap lost type/object binding"
                                    .to_string(),
                            );
                        }
                        let wrong_type = if cap.object().object_type == ObjectType::Endpoint {
                            ObjectType::Thread
                        } else {
                            ObjectType::Endpoint
                        };
                        if task.table.lookup(slot, wrong_type, Rights::NONE)
                            != Err(CapError::WrongObjectType)
                        {
                            return Err(
                                "CAP-INV-003: mismatched object type authorized lookup".to_string()
                            );
                        }
                    }
                    Some(StoredCap::Device(cap)) => {
                        if cap.rights().bits() & !DEVICE_KNOWN_MASK != 0 {
                            return Err(
                                "CAP-INV-006: stored device cap contains unknown bits".to_string()
                            );
                        }
                        if task.table.query(slot) != Err(CapError::EmptySlot) {
                            return Err(
                                "CAP-INV-003: typed device slot leaked into generic CapTable"
                                    .to_string(),
                            );
                        }
                    }
                    None => {
                        if task.table.query(slot) != Err(CapError::EmptySlot) {
                            return Err(
                                "CAP-INV-001/002: empty modeled slot grants generic authority"
                                    .to_string(),
                            );
                        }
                    }
                }
            }
        }

        if self.tasks[APP_TASK].slots.iter().any(Option::is_some) {
            return Err(
                "CAP-INV-009/013: capability-less application gained authority".to_string(),
            );
        }
        Ok(())
    }

    fn record(&mut self, disposition: StepDisposition) {
        match disposition {
            StepDisposition::Accepted => self.counts.accepted += 1,
            StepDisposition::SafeReject => self.counts.safe_rejects += 1,
            StepDisposition::ResourceExhaustion => self.counts.resource_exhaustions += 1,
        }
    }

    fn snapshot(&self) -> ModelSnapshot {
        ModelSnapshot {
            tasks: std::array::from_fn(|index| TaskSnapshot {
                slots: self.tasks[index].slots,
                lifecycle: self.tasks[index].lifecycle,
                restart_count: self.tasks[index].restart_count,
            }),
            target_mutations: self.target_mutations,
            counts: self.counts,
        }
    }
}

impl TaskCaps {
    fn new(task: usize) -> Result<Self, String> {
        let boot = boot_caps(task);
        let mut caps = Self {
            table: CapTable::new(),
            slots: [None; CAP_TABLE_SLOTS],
            boot,
            lifecycle: Lifecycle::Alive,
            restart_count: 0,
        };
        caps.install_boot()?;
        Ok(caps)
    }

    fn install(&mut self, slot: usize, authority: StoredCap) -> Result<(), String> {
        if slot >= CAP_TABLE_SLOTS {
            return Err(format!(
                "CAP-INV-001: internal install used out-of-range slot {slot}"
            ));
        }
        if self.slots[slot].is_some() {
            return Err(format!(
                "CAP-INV-001: internal install overwrote occupied slot {slot}"
            ));
        }
        match authority {
            StoredCap::Generic(cap) => self.table.insert(slot, cap).map_err(|error| {
                format!("CAP-INV-001: CapTable insert failed at slot {slot}: {error:?}")
            })?,
            StoredCap::Device(_) => {
                if self.table.query(slot) != Err(CapError::EmptySlot) {
                    return Err(
                        "CAP-INV-003: device install collided with generic table".to_string()
                    );
                }
            }
        }
        self.slots[slot] = Some(authority);
        Ok(())
    }

    fn clear(&mut self, slot: usize) -> Result<bool, String> {
        if slot >= CAP_TABLE_SLOTS {
            if self.table.revoke(slot) != Err(CapError::InvalidIndex) {
                return Err("CAP-INV-001: invalid revoke did not fail".to_string());
            }
            return Ok(false);
        }

        let previous = self.slots[slot];
        match previous {
            Some(StoredCap::Generic(_)) => {
                if self.table.revoke(slot) != Ok(()) {
                    return Err(
                        "CAP-INV-002/011: generic revoke did not clear held slot".to_string()
                    );
                }
            }
            Some(StoredCap::Device(_)) => {
                if self.table.query(slot) != Err(CapError::EmptySlot) {
                    return Err(
                        "CAP-INV-003: device slot unexpectedly occupied generic table".to_string(),
                    );
                }
            }
            None => {
                if self.table.revoke(slot) != Err(CapError::EmptySlot) {
                    return Err("CAP-INV-001/002: empty slot revoke was not rejected".to_string());
                }
            }
        }
        self.slots[slot] = None;
        Ok(previous.is_some())
    }

    fn clear_all(&mut self) -> Result<(), String> {
        for slot in 0..CAP_TABLE_SLOTS {
            self.clear(slot)?;
        }
        Ok(())
    }

    fn install_boot(&mut self) -> Result<(), String> {
        let boot = self.boot;
        for (slot, authority) in boot.into_iter().enumerate() {
            if let Some(authority) = authority {
                self.install(slot, authority)?;
            }
        }
        Ok(())
    }

    fn restart(&mut self) -> Result<(), String> {
        self.clear_all()?;
        self.install_boot()?;
        self.lifecycle = Lifecycle::Alive;
        self.restart_count += 1;
        Ok(())
    }
}

fn expected_device_check(
    cap: Option<DeviceCapability>,
    target: DeviceId,
    required: DeviceRights,
) -> Result<DeviceId, DeviceAccessError> {
    let Some(cap) = cap else {
        return Err(DeviceAccessError::NoCapability);
    };
    if cap.device().0 >= 2 {
        return Err(DeviceAccessError::UnknownDevice);
    }
    if cap.device() != target {
        return Err(DeviceAccessError::WrongDevice);
    }
    if !cap.rights().contains(required) {
        return Err(DeviceAccessError::InsufficientRights);
    }
    Ok(target)
}

fn derive_requested(parent: Capability, requested_bits: u16) -> Option<Capability> {
    if requested_bits & !GENERIC_KNOWN_MASK != 0 || requested_bits & !parent.rights().bits() != 0 {
        return None;
    }
    let removed_bits = parent.rights().bits() & !requested_bits;
    let removed = generic_rights_from_bits(removed_bits)?;
    Some(parent.derive_diminished(removed))
}

fn generic_rights_from_bits(bits: u16) -> Option<Rights> {
    if bits & !GENERIC_KNOWN_MASK != 0 {
        return None;
    }
    let mut rights = Rights::NONE;
    for (bit, right) in [
        (1 << 0, Rights::READ),
        (1 << 1, Rights::WRITE),
        (1 << 2, Rights::EXECUTE),
        (1 << 3, Rights::SEND),
        (1 << 4, Rights::RECEIVE),
        (1 << 5, Rights::GRANT),
        (1 << 6, Rights::MAP),
        (1 << 7, Rights::CONTROL),
    ] {
        if bits & bit != 0 {
            rights = rights.union(right);
        }
    }
    Some(rights)
}

fn device_rights_from_bits(bits: u16) -> Option<DeviceRights> {
    if bits & !DEVICE_KNOWN_MASK != 0 {
        return None;
    }
    let mut rights = DeviceRights::NONE;
    for (bit, right) in [
        (1 << 0, DeviceRights::DEVICE_INFO),
        (1 << 1, DeviceRights::MMIO_READ),
        (1 << 2, DeviceRights::MMIO_WRITE),
        (1 << 3, DeviceRights::DMA_READ),
        (1 << 4, DeviceRights::DMA_WRITE),
        (1 << 5, DeviceRights::IRQ_RECEIVE),
        (1 << 6, DeviceRights::DRIVER_CONTROL),
        (1 << 7, DeviceRights::NETWORK_DRIVER),
    ] {
        if bits & bit != 0 {
            rights = rights.union(right);
        }
    }
    Some(rights)
}

fn all_generic_rights() -> Rights {
    generic_rights_from_bits(GENERIC_KNOWN_MASK).expect("known rights mask is valid")
}

fn all_device_rights() -> DeviceRights {
    device_rights_from_bits(DEVICE_KNOWN_MASK).expect("known device rights mask is valid")
}

fn lowest_set_bit(bits: u16) -> u16 {
    bits & bits.wrapping_neg()
}

fn first_missing_known_bit(bits: u16) -> u16 {
    let missing = GENERIC_KNOWN_MASK & !bits;
    if missing == 0 {
        UNKNOWN_RIGHT_BIT
    } else {
        lowest_set_bit(missing)
    }
}

fn endpoint_cap(object_id: u32, rights: Rights) -> Capability {
    Capability::new(
        ObjectRef {
            object_type: ObjectType::Endpoint,
            object_id,
        },
        rights,
    )
}

fn profile_cap(profile: CapProfile) -> StoredCap {
    match profile {
        CapProfile::EndpointSend => StoredCap::Generic(endpoint_cap(ENDPOINT_ID, Rights::SEND)),
        CapProfile::EndpointSendReceiveGrant => StoredCap::Generic(endpoint_cap(
            ENDPOINT_ID,
            Rights::SEND.union(Rights::RECEIVE).union(Rights::GRANT),
        )),
        CapProfile::EndpointWrongId => {
            StoredCap::Generic(endpoint_cap(WRONG_ENDPOINT_ID, Rights::SEND))
        }
        CapProfile::ThreadControl => StoredCap::Generic(Capability::new(
            ObjectRef {
                object_type: ObjectType::Thread,
                object_id: MANAGER_TASK as u32,
            },
            Rights::CONTROL,
        )),
        CapProfile::ThreadSend => StoredCap::Generic(Capability::new(
            ObjectRef {
                object_type: ObjectType::Thread,
                object_id: SERVICE_TASK as u32,
            },
            Rights::SEND,
        )),
        CapProfile::EndpointZero => StoredCap::Generic(endpoint_cap(ENDPOINT_ID, Rights::NONE)),
        CapProfile::EndpointAllKnown => {
            StoredCap::Generic(endpoint_cap(ENDPOINT_ID, all_generic_rights()))
        }
        CapProfile::DeviceBlockRead => StoredCap::Device(DeviceCapability::new(
            DeviceId(BLOCK_DEVICE_ID),
            DeviceRights::DEVICE_INFO.union(DeviceRights::MMIO_READ),
        )),
        CapProfile::DeviceBlockControl => StoredCap::Device(DeviceCapability::new(
            DeviceId(BLOCK_DEVICE_ID),
            DeviceRights::DRIVER_CONTROL,
        )),
        CapProfile::DeviceNet => StoredCap::Device(DeviceCapability::new(
            DeviceId(NET_DEVICE_ID),
            DeviceRights::DEVICE_INFO
                .union(DeviceRights::IRQ_RECEIVE)
                .union(DeviceRights::NETWORK_DRIVER),
        )),
        CapProfile::DeviceZero => StoredCap::Device(DeviceCapability::new(
            DeviceId(BLOCK_DEVICE_ID),
            DeviceRights::NONE,
        )),
        CapProfile::DeviceUnknown => StoredCap::Device(DeviceCapability::new(
            DeviceId(NONEXISTENT_DEVICE_ID),
            DeviceRights::MMIO_READ,
        )),
    }
}

fn boot_caps(task: usize) -> [Option<StoredCap>; RUNTIME_CAP_SLOTS] {
    let mut boot = [None; RUNTIME_CAP_SLOTS];
    match task {
        SERVICE_TASK => {
            boot[0] = Some(profile_cap(CapProfile::EndpointSendReceiveGrant));
            boot[1] = Some(profile_cap(CapProfile::DeviceBlockRead));
        }
        MANAGER_TASK => {
            boot[0] = Some(profile_cap(CapProfile::ThreadControl));
            boot[1] = Some(profile_cap(CapProfile::DeviceBlockControl));
        }
        APP_TASK => {}
        _ => unreachable!("bounded task index"),
    }
    boot
}

fn task_index(task: u8) -> usize {
    usize::from(task) % TASK_COUNT
}

fn device_table() -> DeviceTable<2> {
    DeviceTable::new([
        DeviceObject {
            id: DeviceId(BLOCK_DEVICE_ID),
            kind: DeviceKind::BlockDeviceSkeleton,
            mmio: MmioRegion {
                base: 0x1000_1000,
                size: 0x200,
            },
            irq: IrqLine { endpoint: 8 },
            dma: DmaRegion {
                base: 0x8060_0000,
                size: 4096,
            },
        },
        DeviceObject {
            id: DeviceId(NET_DEVICE_ID),
            kind: DeviceKind::SyntheticNetwork,
            mmio: MmioRegion { base: 0, size: 0 },
            irq: IrqLine { endpoint: 10 },
            dma: DmaRegion { base: 0, size: 0 },
        },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CaseGenerator, Corpus, Engine, FailureArtifact, RunConfig};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    fn temp_dir() -> PathBuf {
        let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "axiom-fuzz-capability-{}-{sequence}",
            std::process::id()
        ))
    }

    #[test]
    fn deterministic_decode_is_bounded() {
        let input = vec![0, 1, 2, 3, 4, 255, 254, 253, 252, 251];
        assert_eq!(decode_operations(7, &input), decode_operations(7, &input));
        assert!(decode_operations(7, &[0xff; 512]).len() <= MAX_CAP_OPS_PER_CASE);
    }

    #[test]
    fn same_seed_produces_same_operation_stream() {
        let mut left = CaseGenerator::new(NAME, 20260904, 128, Corpus::empty()).expect("left");
        let mut right = CaseGenerator::new(NAME, 20260904, 128, Corpus::empty()).expect("right");
        for iteration in 0..64 {
            let left_case = left.case(iteration);
            let right_case = right.case(iteration);
            assert_eq!(
                decode_operations(iteration, &left_case.input),
                decode_operations(iteration, &right_case.input)
            );
        }
    }

    #[test]
    fn fixed_boundary_bank_is_named_and_complete() {
        assert_eq!(FIXED_SCENARIO_NAMES.len(), MANDATORY_SCENARIOS as usize);
        for index in 0..MANDATORY_SCENARIOS {
            assert!(!mandatory_scenario(index).is_empty());
        }
    }

    #[test]
    fn slot_presence_and_both_capacity_boundaries_are_denied_or_resolved() {
        let operations = [
            CapabilityOperation::Lookup {
                task: 0,
                slot: 0,
                request: LookupRequest::EndpointSend,
            },
            CapabilityOperation::Lookup {
                task: APP_TASK as u8,
                slot: 0,
                request: LookupRequest::EndpointSend,
            },
            CapabilityOperation::Lookup {
                task: 0,
                slot: CAP_TABLE_SLOTS,
                request: LookupRequest::EndpointSend,
            },
            CapabilityOperation::ReplaceSlot {
                task: 0,
                slot: RUNTIME_CAP_SLOTS - 1,
                profile: CapProfile::EndpointSend,
            },
            CapabilityOperation::Lookup {
                task: 0,
                slot: RUNTIME_CAP_SLOTS - 1,
                request: LookupRequest::EndpointSend,
            },
            CapabilityOperation::Lookup {
                task: 0,
                slot: RUNTIME_CAP_SLOTS,
                request: LookupRequest::EndpointSend,
            },
            CapabilityOperation::ReplaceSlot {
                task: 0,
                slot: CAP_TABLE_SLOTS - 1,
                profile: CapProfile::EndpointSend,
            },
            CapabilityOperation::Lookup {
                task: 0,
                slot: CAP_TABLE_SLOTS - 1,
                request: LookupRequest::EndpointSend,
            },
        ];
        let outcome = run_once(&operations).expect("slot matrix");
        assert_eq!(outcome.snapshot.counts.accepted, 5);
        assert_eq!(outcome.snapshot.counts.safe_rejects, 3);
    }

    #[test]
    fn empty_cleared_and_revoked_caps_are_denied() {
        let outcome = run_once(&[
            CapabilityOperation::ClearSlot { task: 0, slot: 0 },
            CapabilityOperation::Lookup {
                task: 0,
                slot: 0,
                request: LookupRequest::EndpointSend,
            },
            CapabilityOperation::LookupAfterRevoke { task: 0, slot: 0 },
        ])
        .expect("revoke sequence");
        assert_eq!(outcome.snapshot.counts.safe_rejects, 2);
        assert!(outcome.snapshot.tasks[0].slots[0].is_none());
    }

    #[test]
    fn revoked_device_cap_is_denied() {
        let outcome = run_once(&[
            CapabilityOperation::Revoke { task: 0, slot: 1 },
            CapabilityOperation::DeviceUse {
                task: 0,
                slot: 1,
                target: BLOCK_DEVICE_ID,
                request: DeviceRequest::MmioRead,
            },
        ])
        .expect("revoked device");
        assert_eq!(outcome.snapshot.counts.safe_rejects, 1);
        assert_eq!(outcome.snapshot.target_mutations, 0);
    }

    #[test]
    fn wrong_object_type_id_endpoint_and_device_are_denied() {
        for operation in [
            CapabilityOperation::WrongObjectType { task: 0 },
            CapabilityOperation::WrongObjectType {
                task: MANAGER_TASK as u8,
            },
            CapabilityOperation::WrongObjectId { task: 0 },
            CapabilityOperation::WrongEndpoint { task: 0 },
            CapabilityOperation::WrongDevice {
                task: 0,
                nonexistent: false,
            },
            CapabilityOperation::WrongDevice {
                task: 0,
                nonexistent: true,
            },
        ] {
            let outcome = run_once(&[operation]).expect("wrong binding");
            assert_eq!(outcome.snapshot.counts.safe_rejects, 1);
            assert_eq!(outcome.snapshot.target_mutations, 0);
        }
    }

    #[test]
    fn valid_device_cap_authorizes_held_operation() {
        let outcome = run_once(&[CapabilityOperation::DeviceUse {
            task: SERVICE_TASK as u8,
            slot: 1,
            target: BLOCK_DEVICE_ID,
            request: DeviceRequest::MmioRead,
        }])
        .expect("valid device");
        assert_eq!(outcome.snapshot.counts.accepted, 1);
        assert_eq!(outcome.snapshot.counts.safe_rejects, 0);
        assert_eq!(outcome.snapshot.target_mutations, 1);
    }

    #[test]
    fn missing_and_zero_rights_are_denied() {
        let outcome = run_once(&[
            CapabilityOperation::ReplaceSlot {
                task: 0,
                slot: TEST_SLOT,
                profile: CapProfile::EndpointSend,
            },
            CapabilityOperation::Lookup {
                task: 0,
                slot: TEST_SLOT,
                request: LookupRequest::EndpointReceive,
            },
            CapabilityOperation::ZeroRights {
                task: 0,
                device: false,
            },
            CapabilityOperation::ZeroRights {
                task: 0,
                device: true,
            },
        ])
        .expect("rights denial");
        assert_eq!(outcome.snapshot.counts.safe_rejects, 3);
        assert_eq!(outcome.snapshot.target_mutations, 0);
    }

    #[test]
    fn unknown_bits_never_grant_known_rights() {
        for include_required in [false, true] {
            let outcome = run_once(&[CapabilityOperation::UnknownRightsBits {
                task: 0,
                include_required,
            }])
            .expect("unknown bits");
            assert_eq!(outcome.snapshot.counts.safe_rejects, 1);
        }
        let all = run_once(&[CapabilityOperation::AllBitsSet {
            task: 0,
            device: false,
        }])
        .expect("all bits");
        assert_eq!(all.snapshot.counts.safe_rejects, 1);
    }

    #[test]
    fn equal_subset_and_empty_derivations_are_allowed() {
        for mode in [
            DeriveMode::Equal,
            DeriveMode::StrictSubset,
            DeriveMode::Empty,
        ] {
            let outcome =
                run_once(&[CapabilityOperation::Derive { task: 0, mode }]).expect("derive");
            let StoredCap::Generic(parent) = outcome.snapshot.tasks[0].slots[0].expect("parent")
            else {
                panic!("generic parent expected");
            };
            let StoredCap::Generic(child) =
                outcome.snapshot.tasks[0].slots[DYNAMIC_SLOT].expect("child")
            else {
                panic!("generic child expected");
            };
            assert_eq!(child.object(), parent.object());
            assert_eq!(
                child.rights().bits() & !parent.rights().bits(),
                0,
                "child must be a subset"
            );
        }
    }

    #[test]
    fn superset_and_revoked_parent_derivations_are_denied() {
        for mode in [
            DeriveMode::StrictSuperset,
            DeriveMode::ArbitrarySuperset,
            DeriveMode::RevokedParent,
        ] {
            let outcome = run_once(&[CapabilityOperation::Derive { task: 0, mode }]).expect("deny");
            assert_eq!(outcome.snapshot.counts.safe_rejects, 1);
            assert!(outcome.snapshot.tasks[0].slots[DYNAMIC_SLOT].is_none());
        }
    }

    #[test]
    fn cross_task_slot_reuse_is_denied() {
        let outcome = run_once(&[
            CapabilityOperation::CrossTaskUse { owner: 0, slot: 0 },
            CapabilityOperation::CrossTaskUse { owner: 0, slot: 1 },
        ])
        .expect("cross-task");
        assert_eq!(outcome.snapshot.counts.safe_rejects, 2);
        assert!(outcome.snapshot.tasks[APP_TASK]
            .slots
            .iter()
            .all(Option::is_none));
    }

    #[test]
    fn double_revoke_is_stable() {
        let outcome = run_once(&[
            CapabilityOperation::DoubleRevoke { task: 0, slot: 0 },
            CapabilityOperation::DoubleRevoke { task: 0, slot: 0 },
        ])
        .expect("double revoke");
        assert_eq!(outcome.snapshot.counts.accepted, 1);
        assert_eq!(outcome.snapshot.counts.safe_rejects, 1);
    }

    #[test]
    fn killed_and_faulted_tasks_cannot_use_caps() {
        for operation in [
            CapabilityOperation::KillTask { task: 0 },
            CapabilityOperation::FaultTask { task: 0 },
        ] {
            let outcome = run_once(&[operation]).expect("terminal task");
            assert_eq!(outcome.snapshot.target_mutations, 0);
            assert_eq!(outcome.snapshot.counts.accepted, 1);
        }
    }

    #[test]
    fn restart_removes_dynamic_caps_and_restores_only_boot_caps() {
        for fault in [false, true] {
            let outcome = run_once(&[CapabilityOperation::UseAfterTaskRestart { task: 0, fault }])
                .expect("restart");
            assert!(outcome.snapshot.tasks[0].slots[DYNAMIC_SLOT].is_none());
            assert_eq!(outcome.snapshot.tasks[0].restart_count, 2);
            assert_eq!(outcome.snapshot.tasks[0].slots[0], boot_caps(0)[0]);
            assert_eq!(outcome.snapshot.tasks[0].slots[1], boot_caps(0)[1]);
        }
    }

    #[test]
    fn capability_less_application_stays_deny_by_default_across_restart() {
        let outcome = run_once(&[
            CapabilityOperation::RestoreBootCap {
                task: APP_TASK as u8,
            },
            CapabilityOperation::Lookup {
                task: APP_TASK as u8,
                slot: 0,
                request: LookupRequest::EndpointSend,
            },
            CapabilityOperation::DeviceUse {
                task: APP_TASK as u8,
                slot: 0,
                target: BLOCK_DEVICE_ID,
                request: DeviceRequest::Info,
            },
            CapabilityOperation::UseAfterTaskRestart {
                task: APP_TASK as u8,
                fault: true,
            },
        ])
        .expect("application denial");
        assert!(outcome.snapshot.tasks[APP_TASK]
            .slots
            .iter()
            .all(Option::is_none));
        assert_eq!(outcome.snapshot.target_mutations, 0);
    }

    #[test]
    fn full_host_table_is_bounded_resource_exhaustion() {
        let result = evaluate_operations(&[CapabilityOperation::FillHostTable {
            task: MANAGER_TASK as u8,
        }]);
        assert_eq!(result.class, ResultClass::BoundedResourceExhaustion);
    }

    #[test]
    fn identical_sequence_has_identical_final_state() {
        let operations = decode_operations(31, &[2, 0, 1, 2, 3, 18, 0, 0, 0, 1]);
        assert_eq!(
            run_once(&operations).expect("first"),
            run_once(&operations).expect("second")
        );
    }

    struct FailingCapabilityTarget;

    impl FuzzTarget for FailingCapabilityTarget {
        fn name(&self) -> &'static str {
            NAME
        }

        fn evaluate(&mut self, _case: &FuzzCase) -> CaseResult {
            CaseResult::invariant_failure("CAP-INV-test injected failure")
        }
    }

    #[test]
    fn capability_invariant_failure_artifact_replays_exact_input() {
        let directory = temp_dir();
        let config = RunConfig {
            target: NAME.to_string(),
            seed: 20260904,
            iterations: 1,
            max_len: 128,
            failure_dir: directory.clone(),
        };
        let summary = Engine
            .run(&config, Corpus::empty(), &mut FailingCapabilityTarget)
            .expect("write capability artifact");
        assert_eq!(summary.exit_code(), 1);

        let path = directory.join("capability/seed-20260904-iteration-0.txt");
        let artifact = FailureArtifact::read(&path).expect("read artifact");
        assert_eq!(artifact.target, NAME);
        assert_eq!(artifact.seed, 20260904);
        assert_eq!(artifact.iteration, 0);
        assert!(artifact.input.is_empty());

        let replay = Engine
            .replay(&artifact, &mut FailingCapabilityTarget)
            .expect("replay exact capability input");
        assert_eq!(replay.exit_code(), 1);
        assert_eq!(artifact.to_case().input, Vec::<u8>::new());
        fs::remove_dir_all(directory).expect("remove test directory");
    }
}
