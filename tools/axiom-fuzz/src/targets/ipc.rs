//! Deterministic fuzz target for AxiomRT's bounded IPC host model.
//!
//! The target calls the real kernel Message, Endpoint, CapTable, and Thread
//! APIs. It models only the scheduler glue needed to apply their explicit
//! blocked/ready outcomes; RISC-V pointer and MMU behavior is out of scope.

use crate::{CaseResult, FuzzCase, FuzzTarget, ResultClass};
use kernel::caps::table::CapError;
use kernel::caps::{CapTable, Capability, ObjectRef, ObjectType, Rights};
use kernel::ipc::{
    cancel, recv_checked, send_checked, CancelOutcome, Endpoint, EndpointId, EndpointState,
    IpcCapError, IpcError, Message, MessageError, RecvOutcome, SendOutcome, MSG_MAX_BYTES,
};
use kernel::memory::AddressSpaceId;
use kernel::thread::{Thread, ThreadId, ThreadState};

pub const NAME: &str = "ipc";
pub const MAX_OPS_PER_CASE: usize = 16;

const ACTOR_COUNT: usize = 3;
const ENDPOINT_ID: u32 = 7;
const CAP_SLOT: usize = 0;
const MANDATORY_SCENARIOS: u64 = 20;
const MAX_REQUESTED_MESSAGE: usize = MSG_MAX_BYTES * 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    Send,
    Receive,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IpcOperation {
    Send {
        actor: u8,
        requested_len: usize,
        fill: u8,
    },
    Receive {
        actor: u8,
    },
    Cancel {
        actor: u8,
    },
    RevokeCap {
        actor: u8,
    },
    RestoreCap {
        actor: u8,
    },
    UseWrongEndpoint {
        actor: u8,
        direction: Direction,
    },
    UseWrongObjectType {
        actor: u8,
        direction: Direction,
    },
    UseWrongRights {
        actor: u8,
        direction: Direction,
    },
    FaultTask {
        actor: u8,
    },
    KillTask {
        actor: u8,
    },
    MakeReady {
        actor: u8,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CapProfile {
    Missing,
    Revoked,
    ValidSend,
    ValidReceive,
    ValidBoth,
    WrongEndpointSend,
    WrongEndpointReceive,
    WrongTypeSend,
    WrongTypeReceive,
    WrongRightsSend,
    WrongRightsReceive,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Pending {
    sender: ThreadId,
    data: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct OperationCounts {
    accepted: u64,
    safe_rejects: u64,
    resource_exhaustions: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ModelSnapshot {
    endpoint: EndpointState,
    task_states: [ThreadState; ACTOR_COUNT],
    cap_profiles: [CapProfile; ACTOR_COUNT],
    pending: Option<Pending>,
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

struct Actor {
    thread: Thread,
    caps: CapTable,
    profile: CapProfile,
}

struct IpcModel {
    endpoint: Endpoint,
    actors: [Actor; ACTOR_COUNT],
    pending: Option<Pending>,
    counts: OperationCounts,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct IpcTarget;

impl FuzzTarget for IpcTarget {
    fn name(&self) -> &'static str {
        NAME
    }

    fn evaluate(&mut self, case: &FuzzCase) -> CaseResult {
        if case.target != NAME {
            return CaseResult::invariant_failure("ipc target received a case for another target");
        }
        evaluate_operations(&decode_operations(case.iteration, &case.input))
    }
}

pub fn decode_operations(iteration: u64, input: &[u8]) -> Vec<IpcOperation> {
    let mut operations = mandatory_scenario(iteration % MANDATORY_SCENARIOS);
    for chunk in input.chunks(4) {
        if operations.len() == MAX_OPS_PER_CASE {
            break;
        }
        let opcode = chunk[0];
        let actor = chunk.get(1).copied().unwrap_or(0) % ACTOR_COUNT as u8;
        let parameter = chunk.get(2).copied().unwrap_or(0);
        let fill = chunk.get(3).copied().unwrap_or(opcode);
        let direction = if parameter & 1 == 0 {
            Direction::Send
        } else {
            Direction::Receive
        };
        let operation = match opcode % 14 {
            0 => IpcOperation::Send {
                actor,
                requested_len: boundary_length(parameter),
                fill,
            },
            1 => IpcOperation::Receive { actor },
            2 => IpcOperation::Cancel { actor },
            3 => IpcOperation::RevokeCap { actor },
            4 => IpcOperation::RestoreCap { actor },
            5 => IpcOperation::UseWrongEndpoint { actor, direction },
            6 => IpcOperation::UseWrongObjectType { actor, direction },
            7 => IpcOperation::UseWrongRights { actor, direction },
            8 => IpcOperation::FaultTask { actor },
            9 => IpcOperation::KillTask { actor },
            10 => IpcOperation::MakeReady { actor },
            11 => IpcOperation::Send {
                actor,
                requested_len: MSG_MAX_BYTES,
                fill,
            },
            12 => IpcOperation::Send {
                actor,
                requested_len: MSG_MAX_BYTES + 1,
                fill,
            },
            _ => IpcOperation::Receive { actor: 2 },
        };
        operations.push(operation);
    }
    operations
}

pub fn evaluate_operations(operations: &[IpcOperation]) -> CaseResult {
    if operations.len() > MAX_OPS_PER_CASE {
        return CaseResult::invariant_failure(format!(
            "IPC operation count {} exceeds bound {MAX_OPS_PER_CASE}",
            operations.len()
        ));
    }

    let first = match run_once(operations) {
        Ok(outcome) => outcome,
        Err(reason) => return CaseResult::invariant_failure(reason),
    };
    let second = match run_once(operations) {
        Ok(outcome) => outcome,
        Err(reason) => return CaseResult::invariant_failure(reason),
    };
    if first != second {
        return CaseResult::invariant_failure(
            "IPC-INV-012: identical operation sequences produced different final states",
        );
    }

    if first.snapshot.counts.resource_exhaustions > 0 {
        CaseResult::new(
            ResultClass::BoundedResourceExhaustion,
            "bounded one-party rendezvous capacity was reached",
        )
    } else {
        CaseResult::safe_reject("IPC sequence completed without an invariant failure")
    }
}

fn mandatory_scenario(index: u64) -> Vec<IpcOperation> {
    use Direction::{Receive as RecvDirection, Send as SendDirection};
    use IpcOperation::*;

    let send = |requested_len, fill| Send {
        actor: 0,
        requested_len,
        fill,
    };
    let recv = || Receive { actor: 1 };

    match index {
        0 => vec![send(0, 0x00), recv()],
        1 => vec![send(1, 0x11), recv()],
        2 => vec![send(MSG_MAX_BYTES - 1, 0x22), recv()],
        3 => vec![send(MSG_MAX_BYTES, 0x33), recv()],
        4 => vec![send(MSG_MAX_BYTES + 1, 0x44)],
        5 => vec![send(MAX_REQUESTED_MESSAGE, 0x55)],
        6 => vec![send(4, 0x61), recv()],
        7 => vec![recv(), send(4, 0x71)],
        8 => vec![
            RestoreCap { actor: 2 },
            send(2, 0x81),
            Send {
                actor: 2,
                requested_len: 2,
                fill: 0x82,
            },
        ],
        9 => vec![RestoreCap { actor: 2 }, recv(), Receive { actor: 2 }],
        10 => vec![send(3, 0xa1), Cancel { actor: 0 }, Cancel { actor: 0 }],
        11 => vec![recv(), Cancel { actor: 1 }, Cancel { actor: 1 }],
        12 => vec![RevokeCap { actor: 0 }, send(1, 0xc1)],
        13 => vec![
            UseWrongRights {
                actor: 0,
                direction: SendDirection,
            },
            send(1, 0xd1),
        ],
        14 => vec![
            UseWrongRights {
                actor: 1,
                direction: RecvDirection,
            },
            recv(),
        ],
        15 => vec![
            UseWrongEndpoint {
                actor: 0,
                direction: SendDirection,
            },
            send(1, 0xe1),
            UseWrongObjectType {
                actor: 1,
                direction: RecvDirection,
            },
            recv(),
            Send {
                actor: 2,
                requested_len: 1,
                fill: 0xe2,
            },
        ],
        16 => vec![
            FaultTask { actor: 0 },
            send(1, 0xf1),
            KillTask { actor: 1 },
            recv(),
            MakeReady { actor: 0 },
            MakeReady { actor: 1 },
        ],
        17 => vec![send(MSG_MAX_BYTES, 0x17), recv()],
        18 => vec![send(3, 0x18), recv(), Cancel { actor: 0 }],
        _ => vec![
            RevokeCap { actor: 0 },
            send(1, 0x19),
            RestoreCap { actor: 0 },
            send(1, 0x1a),
            recv(),
        ],
    }
}

fn boundary_length(selector: u8) -> usize {
    match selector % 6 {
        0 => 0,
        1 => 1,
        2 => MSG_MAX_BYTES - 1,
        3 => MSG_MAX_BYTES,
        4 => MSG_MAX_BYTES + 1,
        _ => MAX_REQUESTED_MESSAGE,
    }
}

fn run_once(operations: &[IpcOperation]) -> Result<ModelOutcome, String> {
    let mut model = IpcModel::new()?;
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

impl IpcModel {
    fn new() -> Result<Self, String> {
        let actors = std::array::from_fn(|index| Actor {
            thread: Thread::new(ThreadId(index as u32 + 1), AddressSpaceId(index as u32 + 1)),
            caps: CapTable::new(),
            profile: CapProfile::Missing,
        });
        let mut model = Self {
            endpoint: Endpoint::new(EndpointId(ENDPOINT_ID)),
            actors,
            pending: None,
            counts: OperationCounts::default(),
        };
        model.set_profile(0, CapProfile::ValidSend)?;
        model.set_profile(1, CapProfile::ValidReceive)?;
        Ok(model)
    }

    fn apply(&mut self, operation: &IpcOperation) -> Result<StepDisposition, String> {
        match *operation {
            IpcOperation::Send {
                actor,
                requested_len,
                fill,
            } => self.perform_send(actor, requested_len, fill),
            IpcOperation::Receive { actor } => self.perform_receive(actor),
            IpcOperation::Cancel { actor } => self.perform_cancel(actor),
            IpcOperation::RevokeCap { actor } => self.revoke_cap(actor),
            IpcOperation::RestoreCap { actor } => self.restore_cap(actor),
            IpcOperation::UseWrongEndpoint { actor, direction } => {
                self.configure_wrong_endpoint(actor, direction)
            }
            IpcOperation::UseWrongObjectType { actor, direction } => {
                self.configure_wrong_type(actor, direction)
            }
            IpcOperation::UseWrongRights { actor, direction } => {
                self.configure_wrong_rights(actor, direction)
            }
            IpcOperation::FaultTask { actor } => self.fault_task(actor),
            IpcOperation::KillTask { actor } => self.kill_task(actor),
            IpcOperation::MakeReady { actor } => self.make_ready(actor),
        }
    }

    fn perform_send(
        &mut self,
        actor: u8,
        requested_len: usize,
        fill: u8,
    ) -> Result<StepDisposition, String> {
        let index = actor_index(actor);
        let sender = self.actors[index].thread.id();
        let state = self.actors[index].thread.state();

        if matches!(
            state,
            ThreadState::Faulted | ThreadState::Killed | ThreadState::Suspended
        ) {
            return Ok(StepDisposition::SafeReject);
        }
        if requested_len > MAX_REQUESTED_MESSAGE {
            return Ok(StepDisposition::SafeReject);
        }

        let before_endpoint = self.endpoint.state();
        let before_pending = self.pending.clone();
        let mut source = vec![fill; requested_len];
        let message = match Message::new(sender, &source) {
            Ok(message) => message,
            Err(MessageError::TooLarge) => {
                self.ensure_endpoint_unchanged(before_endpoint, &before_pending)?;
                return Ok(StepDisposition::SafeReject);
            }
        };
        let expected = source.clone();
        source.fill(fill.wrapping_add(1));

        let started_running = match state {
            ThreadState::Ready => {
                self.transition(index, ThreadState::Running)?;
                true
            }
            ThreadState::Blocked
                if matches!(
                    self.endpoint.state(),
                    EndpointState::SenderWaiting { sender: waiting } if waiting == sender
                ) =>
            {
                false
            }
            ThreadState::Blocked => return Ok(StepDisposition::SafeReject),
            ThreadState::Running => {
                return Err("IPC-INV-013: task remained Running between operations".to_string())
            }
            ThreadState::Faulted | ThreadState::Killed | ThreadState::Suspended => unreachable!(),
        };

        let result = send_checked(
            &self.actors[index].caps,
            CAP_SLOT,
            &mut self.endpoint,
            sender,
            message,
        );
        match result {
            Ok(SendOutcome::Blocked) => {
                if !started_running {
                    return Err("IPC-INV-004: repeated sender created another wait".to_string());
                }
                self.transition(index, ThreadState::Blocked)?;
                self.pending = Some(Pending {
                    sender,
                    data: expected,
                });
                Ok(StepDisposition::Accepted)
            }
            Ok(SendOutcome::Delivered { to, msg }) => {
                if !started_running {
                    return Err(
                        "IPC-INV-010: blocked sender delivered while not runnable".to_string()
                    );
                }
                self.validate_message(&msg, sender, &expected)?;
                let receiver_index = self
                    .actor_for_tid(to)
                    .ok_or_else(|| "IPC-INV-013: delivery named an unknown receiver".to_string())?;
                if self.actors[receiver_index].thread.state() != ThreadState::Blocked {
                    return Err("IPC-INV-010: delivery readied a non-blocked receiver".to_string());
                }
                self.transition(receiver_index, ThreadState::Ready)?;
                self.transition(index, ThreadState::Ready)?;
                self.pending = None;
                Ok(StepDisposition::Accepted)
            }
            Err(IpcCapError::Cap(_)) | Err(IpcCapError::WrongEndpoint) => {
                self.finish_running(index, started_running)?;
                self.ensure_endpoint_unchanged(before_endpoint, &before_pending)?;
                Ok(StepDisposition::SafeReject)
            }
            Err(IpcCapError::Ipc(IpcError::Busy | IpcError::AlreadyWaiting)) => {
                self.finish_running(index, started_running)?;
                self.ensure_endpoint_unchanged(before_endpoint, &before_pending)?;
                Ok(StepDisposition::ResourceExhaustion)
            }
        }
    }

    fn perform_receive(&mut self, actor: u8) -> Result<StepDisposition, String> {
        let index = actor_index(actor);
        let receiver = self.actors[index].thread.id();
        let state = self.actors[index].thread.state();

        if matches!(
            state,
            ThreadState::Faulted | ThreadState::Killed | ThreadState::Suspended
        ) {
            return Ok(StepDisposition::SafeReject);
        }

        let before_endpoint = self.endpoint.state();
        let before_pending = self.pending.clone();
        let started_running = match state {
            ThreadState::Ready => {
                self.transition(index, ThreadState::Running)?;
                true
            }
            ThreadState::Blocked
                if matches!(
                    self.endpoint.state(),
                    EndpointState::ReceiverWaiting { receiver: waiting } if waiting == receiver
                ) =>
            {
                false
            }
            ThreadState::Blocked => return Ok(StepDisposition::SafeReject),
            ThreadState::Running => {
                return Err("IPC-INV-013: task remained Running between operations".to_string())
            }
            ThreadState::Faulted | ThreadState::Killed | ThreadState::Suspended => unreachable!(),
        };

        let result = recv_checked(
            &self.actors[index].caps,
            CAP_SLOT,
            &mut self.endpoint,
            receiver,
        );
        match result {
            Ok(RecvOutcome::Blocked) => {
                if !started_running {
                    return Err("IPC-INV-005: repeated receiver created another wait".to_string());
                }
                self.transition(index, ThreadState::Blocked)?;
                Ok(StepDisposition::Accepted)
            }
            Ok(RecvOutcome::Received { msg, unblock }) => {
                if !started_running {
                    return Err(
                        "IPC-INV-010: blocked receiver received while not runnable".to_string()
                    );
                }
                let pending = self.pending.clone().ok_or_else(|| {
                    "IPC-INV-013: SenderWaiting had no modeled pending message".to_string()
                })?;
                if unblock != pending.sender {
                    return Err("IPC-INV-002: receive unblocked the wrong sender".to_string());
                }
                self.validate_message(&msg, pending.sender, &pending.data)?;
                let sender_index = self
                    .actor_for_tid(unblock)
                    .ok_or_else(|| "IPC-INV-013: receive named an unknown sender".to_string())?;
                if self.actors[sender_index].thread.state() != ThreadState::Blocked {
                    return Err("IPC-INV-010: receive readied a non-blocked sender".to_string());
                }
                self.transition(sender_index, ThreadState::Ready)?;
                self.transition(index, ThreadState::Ready)?;
                self.pending = None;
                Ok(StepDisposition::Accepted)
            }
            Err(IpcCapError::Cap(_)) | Err(IpcCapError::WrongEndpoint) => {
                self.finish_running(index, started_running)?;
                self.ensure_endpoint_unchanged(before_endpoint, &before_pending)?;
                Ok(StepDisposition::SafeReject)
            }
            Err(IpcCapError::Ipc(IpcError::Busy | IpcError::AlreadyWaiting)) => {
                self.finish_running(index, started_running)?;
                self.ensure_endpoint_unchanged(before_endpoint, &before_pending)?;
                Ok(StepDisposition::ResourceExhaustion)
            }
        }
    }

    fn perform_cancel(&mut self, actor: u8) -> Result<StepDisposition, String> {
        let index = actor_index(actor);
        let tid = self.actors[index].thread.id();
        let before_endpoint = self.endpoint.state();
        let before_pending = self.pending.clone();
        let outcome = cancel(&mut self.endpoint, tid);

        if self.actors[index].thread.state() == ThreadState::Blocked {
            if outcome != CancelOutcome::Cancelled {
                return Err("IPC-INV-011: blocked IPC party was not cancelled".to_string());
            }
            self.pending = None;
            self.transition(index, ThreadState::Ready)?;
            Ok(StepDisposition::Accepted)
        } else {
            if outcome != CancelOutcome::NotWaiting {
                return Err("IPC-INV-011: non-blocked task cancelled endpoint state".to_string());
            }
            self.ensure_endpoint_unchanged(before_endpoint, &before_pending)?;
            Ok(StepDisposition::SafeReject)
        }
    }

    fn revoke_cap(&mut self, actor: u8) -> Result<StepDisposition, String> {
        let index = actor_index(actor);
        match self.actors[index].caps.revoke(CAP_SLOT) {
            Ok(()) => {
                self.actors[index].profile = CapProfile::Revoked;
                Ok(StepDisposition::Accepted)
            }
            Err(CapError::EmptySlot) => {
                self.actors[index].profile = CapProfile::Revoked;
                Ok(StepDisposition::SafeReject)
            }
            Err(error) => Err(format!(
                "IPC-INV-009: unexpected capability revoke result {error:?}"
            )),
        }
    }

    fn restore_cap(&mut self, actor: u8) -> Result<StepDisposition, String> {
        let index = actor_index(actor);
        let profile = match index {
            0 => CapProfile::ValidSend,
            1 => CapProfile::ValidReceive,
            _ => CapProfile::ValidBoth,
        };
        self.set_profile(index, profile)?;
        Ok(StepDisposition::Accepted)
    }

    fn configure_wrong_endpoint(
        &mut self,
        actor: u8,
        direction: Direction,
    ) -> Result<StepDisposition, String> {
        let profile = match direction {
            Direction::Send => CapProfile::WrongEndpointSend,
            Direction::Receive => CapProfile::WrongEndpointReceive,
        };
        self.set_profile(actor_index(actor), profile)?;
        Ok(StepDisposition::Accepted)
    }

    fn configure_wrong_type(
        &mut self,
        actor: u8,
        direction: Direction,
    ) -> Result<StepDisposition, String> {
        let profile = match direction {
            Direction::Send => CapProfile::WrongTypeSend,
            Direction::Receive => CapProfile::WrongTypeReceive,
        };
        self.set_profile(actor_index(actor), profile)?;
        Ok(StepDisposition::Accepted)
    }

    fn configure_wrong_rights(
        &mut self,
        actor: u8,
        direction: Direction,
    ) -> Result<StepDisposition, String> {
        let profile = match direction {
            Direction::Send => CapProfile::WrongRightsSend,
            Direction::Receive => CapProfile::WrongRightsReceive,
        };
        self.set_profile(actor_index(actor), profile)?;
        Ok(StepDisposition::Accepted)
    }

    fn fault_task(&mut self, actor: u8) -> Result<StepDisposition, String> {
        let index = actor_index(actor);
        match self.actors[index].thread.state() {
            ThreadState::Faulted | ThreadState::Killed => Ok(StepDisposition::SafeReject),
            ThreadState::Blocked => {
                self.cancel_for_terminal(index)?;
                self.transition(index, ThreadState::Faulted)?;
                Ok(StepDisposition::Accepted)
            }
            ThreadState::Ready => {
                self.transition(index, ThreadState::Faulted)?;
                Ok(StepDisposition::Accepted)
            }
            ThreadState::Suspended => Ok(StepDisposition::SafeReject),
            ThreadState::Running => {
                Err("IPC-INV-013: task remained Running between operations".to_string())
            }
        }
    }

    fn kill_task(&mut self, actor: u8) -> Result<StepDisposition, String> {
        let index = actor_index(actor);
        match self.actors[index].thread.state() {
            ThreadState::Killed => Ok(StepDisposition::SafeReject),
            ThreadState::Blocked => {
                self.cancel_for_terminal(index)?;
                self.transition(index, ThreadState::Killed)?;
                Ok(StepDisposition::Accepted)
            }
            ThreadState::Ready | ThreadState::Faulted | ThreadState::Suspended => {
                self.transition(index, ThreadState::Killed)?;
                Ok(StepDisposition::Accepted)
            }
            ThreadState::Running => {
                Err("IPC-INV-013: task remained Running between operations".to_string())
            }
        }
    }

    fn make_ready(&mut self, actor: u8) -> Result<StepDisposition, String> {
        let index = actor_index(actor);
        if self.actors[index].thread.state() == ThreadState::Blocked {
            self.cancel_for_terminal(index)?;
            self.transition(index, ThreadState::Ready)?;
            return Ok(StepDisposition::Accepted);
        }

        let before = self.actors[index].thread.state();
        match self.actors[index].thread.transition(ThreadState::Ready) {
            Ok(()) if before == ThreadState::Suspended => Ok(StepDisposition::Accepted),
            Ok(()) => Err(format!(
                "IPC-INV-010: terminal/already-ready state {before:?} became Ready"
            )),
            Err(_) => Ok(StepDisposition::SafeReject),
        }
    }

    fn cancel_for_terminal(&mut self, index: usize) -> Result<(), String> {
        let tid = self.actors[index].thread.id();
        if cancel(&mut self.endpoint, tid) != CancelOutcome::Cancelled {
            return Err("IPC-INV-011: terminal transition left stale IPC state".to_string());
        }
        self.pending = None;
        Ok(())
    }

    fn set_profile(&mut self, index: usize, profile: CapProfile) -> Result<(), String> {
        match self.actors[index].caps.revoke(CAP_SLOT) {
            Ok(()) | Err(CapError::EmptySlot) => {}
            Err(error) => {
                return Err(format!(
                    "IPC-INV-006: could not clear capability slot: {error:?}"
                ))
            }
        }

        let capability = match profile {
            CapProfile::Missing | CapProfile::Revoked => None,
            CapProfile::ValidSend => Some(endpoint_cap(ENDPOINT_ID, Rights::SEND)),
            CapProfile::ValidReceive => Some(endpoint_cap(ENDPOINT_ID, Rights::RECEIVE)),
            CapProfile::ValidBoth => Some(endpoint_cap(
                ENDPOINT_ID,
                Rights::SEND.union(Rights::RECEIVE),
            )),
            CapProfile::WrongEndpointSend => Some(endpoint_cap(ENDPOINT_ID + 1, Rights::SEND)),
            CapProfile::WrongEndpointReceive => {
                Some(endpoint_cap(ENDPOINT_ID + 1, Rights::RECEIVE))
            }
            CapProfile::WrongTypeSend => Some(Capability::new(
                ObjectRef {
                    object_type: ObjectType::Thread,
                    object_id: ENDPOINT_ID,
                },
                Rights::SEND,
            )),
            CapProfile::WrongTypeReceive => Some(Capability::new(
                ObjectRef {
                    object_type: ObjectType::Thread,
                    object_id: ENDPOINT_ID,
                },
                Rights::RECEIVE,
            )),
            CapProfile::WrongRightsSend => Some(endpoint_cap(ENDPOINT_ID, Rights::RECEIVE)),
            CapProfile::WrongRightsReceive => Some(endpoint_cap(ENDPOINT_ID, Rights::SEND)),
        };

        if let Some(capability) = capability {
            self.actors[index]
                .caps
                .insert(CAP_SLOT, capability)
                .map_err(|error| {
                    format!("IPC-INV-006: capability installation failed: {error:?}")
                })?;
        }
        self.actors[index].profile = profile;
        Ok(())
    }

    fn transition(&mut self, index: usize, state: ThreadState) -> Result<(), String> {
        self.actors[index]
            .thread
            .transition(state)
            .map_err(|error| {
                format!(
                    "IPC-INV-010: illegal task transition {:?}->{:?}",
                    error.from, error.to
                )
            })
    }

    fn finish_running(&mut self, index: usize, started_running: bool) -> Result<(), String> {
        if started_running {
            self.transition(index, ThreadState::Ready)
        } else {
            Ok(())
        }
    }

    fn validate_message(
        &self,
        message: &Message,
        sender: ThreadId,
        expected: &[u8],
    ) -> Result<(), String> {
        if message.header().sender != sender || message.data() != expected {
            return Err("IPC-INV-002: delivery did not preserve exact message bytes".to_string());
        }
        if message.data().len() > MSG_MAX_BYTES {
            return Err("IPC-INV-001: delivered message exceeded MSG_MAX_BYTES".to_string());
        }
        Ok(())
    }

    fn ensure_endpoint_unchanged(
        &self,
        endpoint: EndpointState,
        pending: &Option<Pending>,
    ) -> Result<(), String> {
        if self.endpoint.state() != endpoint || &self.pending != pending {
            Err("IPC-INV-006/007/008/009: rejected request mutated IPC state".to_string())
        } else {
            Ok(())
        }
    }

    fn actor_for_tid(&self, tid: ThreadId) -> Option<usize> {
        self.actors
            .iter()
            .position(|actor| actor.thread.id() == tid)
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
            endpoint: self.endpoint.state(),
            task_states: std::array::from_fn(|index| self.actors[index].thread.state()),
            cap_profiles: std::array::from_fn(|index| self.actors[index].profile),
            pending: self.pending.clone(),
            counts: self.counts,
        }
    }

    fn check_invariants(&self) -> Result<(), String> {
        let blocked_count = self
            .actors
            .iter()
            .filter(|actor| actor.thread.state() == ThreadState::Blocked)
            .count();

        match self.endpoint.state() {
            EndpointState::Idle => {
                if self.pending.is_some() || blocked_count != 0 {
                    return Err(
                        "IPC-INV-011: Idle endpoint retained blocked/pending state".to_string()
                    );
                }
            }
            EndpointState::SenderWaiting { sender } => {
                let Some(index) = self.actor_for_tid(sender) else {
                    return Err("IPC-INV-013: endpoint references unknown sender".to_string());
                };
                if blocked_count != 1 || self.actors[index].thread.state() != ThreadState::Blocked {
                    return Err("IPC-INV-004/010: sender wait state is inconsistent".to_string());
                }
                let pending = self
                    .pending
                    .as_ref()
                    .ok_or_else(|| "IPC-INV-013: sender wait has no pending payload".to_string())?;
                if pending.sender != sender || pending.data.len() > MSG_MAX_BYTES {
                    return Err("IPC-INV-001/002: pending message metadata is invalid".to_string());
                }
            }
            EndpointState::ReceiverWaiting { receiver } => {
                let Some(index) = self.actor_for_tid(receiver) else {
                    return Err("IPC-INV-013: endpoint references unknown receiver".to_string());
                };
                if blocked_count != 1
                    || self.actors[index].thread.state() != ThreadState::Blocked
                    || self.pending.is_some()
                {
                    return Err("IPC-INV-005/010: receiver wait state is inconsistent".to_string());
                }
            }
        }

        for actor in &self.actors {
            if matches!(
                actor.thread.state(),
                ThreadState::Faulted | ThreadState::Killed
            ) {
                let tid = actor.thread.id();
                if matches!(
                    self.endpoint.state(),
                    EndpointState::SenderWaiting { sender } if sender == tid
                ) || matches!(
                    self.endpoint.state(),
                    EndpointState::ReceiverWaiting { receiver } if receiver == tid
                ) {
                    return Err(
                        "IPC-INV-010/011: terminal task retained IPC wait state".to_string()
                    );
                }
            }
            if actor.thread.state() == ThreadState::Running {
                return Err("IPC-INV-013: task remained Running after an operation".to_string());
            }
        }
        Ok(())
    }
}

fn actor_index(actor: u8) -> usize {
    usize::from(actor) % ACTOR_COUNT
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CaseGenerator, Corpus, Engine, FailureArtifact, RunConfig};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    fn send(length: usize, fill: u8) -> IpcOperation {
        IpcOperation::Send {
            actor: 0,
            requested_len: length,
            fill,
        }
    }

    fn recv() -> IpcOperation {
        IpcOperation::Receive { actor: 1 }
    }

    fn temp_dir() -> PathBuf {
        let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("axiom-fuzz-ipc-{}-{sequence}", std::process::id()))
    }

    #[test]
    fn target_decodes_deterministic_bounded_operations() {
        let input = vec![0, 1, 2, 3, 255, 254, 253, 252];
        assert_eq!(decode_operations(7, &input), decode_operations(7, &input));
        assert!(decode_operations(7, &[0xff; 256]).len() <= MAX_OPS_PER_CASE);
    }

    #[test]
    fn same_seed_produces_same_operation_stream() {
        let mut left = CaseGenerator::new(NAME, 77, 128, Corpus::empty()).expect("left generator");
        let mut right =
            CaseGenerator::new(NAME, 77, 128, Corpus::empty()).expect("right generator");
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
    fn mandatory_message_boundaries_are_exercised() {
        let mut lengths = Vec::new();
        for iteration in 0..6 {
            for operation in decode_operations(iteration, &[]) {
                if let IpcOperation::Send { requested_len, .. } = operation {
                    lengths.push(requested_len);
                }
            }
        }
        for required in [
            0,
            1,
            MSG_MAX_BYTES - 1,
            MSG_MAX_BYTES,
            MSG_MAX_BYTES + 1,
            MAX_REQUESTED_MESSAGE,
        ] {
            assert!(lengths.contains(&required), "missing length {required}");
        }
    }

    #[test]
    fn max_plus_one_is_rejected_without_state_change() {
        let outcome = run_once(&[send(MSG_MAX_BYTES + 1, 1)]).expect("evaluate");
        assert_eq!(outcome.snapshot.endpoint, EndpointState::Idle);
        assert_eq!(outcome.snapshot.counts.safe_rejects, 1);
        assert_eq!(outcome.snapshot.counts.accepted, 0);
    }

    #[test]
    fn sender_first_rendezvous_returns_idle_and_ready() {
        let outcome = run_once(&[send(4, 0x41), recv()]).expect("sender first");
        assert_eq!(outcome.snapshot.endpoint, EndpointState::Idle);
        assert_eq!(
            outcome.snapshot.task_states,
            [ThreadState::Ready; ACTOR_COUNT]
        );
        assert_eq!(outcome.snapshot.counts.accepted, 2);
    }

    #[test]
    fn receiver_first_rendezvous_returns_idle_and_ready() {
        let outcome = run_once(&[recv(), send(4, 0x42)]).expect("receiver first");
        assert_eq!(outcome.snapshot.endpoint, EndpointState::Idle);
        assert_eq!(
            outcome.snapshot.task_states,
            [ThreadState::Ready; ACTOR_COUNT]
        );
        assert_eq!(outcome.snapshot.counts.accepted, 2);
    }

    #[test]
    fn repeated_send_and_receive_hit_bounded_capacity() {
        let send_outcome = run_once(&[
            IpcOperation::RestoreCap { actor: 2 },
            send(1, 1),
            IpcOperation::Send {
                actor: 2,
                requested_len: 1,
                fill: 2,
            },
        ])
        .expect("repeated send");
        assert_eq!(send_outcome.snapshot.counts.resource_exhaustions, 1);

        let recv_outcome = run_once(&[
            IpcOperation::RestoreCap { actor: 2 },
            recv(),
            IpcOperation::Receive { actor: 2 },
        ])
        .expect("repeated receive");
        assert_eq!(recv_outcome.snapshot.counts.resource_exhaustions, 1);
    }

    #[test]
    fn missing_wrong_endpoint_and_wrong_object_caps_are_denied() {
        let missing = run_once(&[IpcOperation::Send {
            actor: 2,
            requested_len: 1,
            fill: 1,
        }])
        .expect("missing capability");
        assert_eq!(missing.snapshot.endpoint, EndpointState::Idle);
        assert_eq!(missing.snapshot.counts.safe_rejects, 1);

        let wrong_endpoint = run_once(&[
            IpcOperation::UseWrongEndpoint {
                actor: 0,
                direction: Direction::Send,
            },
            send(1, 2),
        ])
        .expect("wrong endpoint");
        assert_eq!(wrong_endpoint.snapshot.endpoint, EndpointState::Idle);
        assert_eq!(wrong_endpoint.snapshot.counts.safe_rejects, 1);

        let wrong_object = run_once(&[
            IpcOperation::UseWrongObjectType {
                actor: 0,
                direction: Direction::Send,
            },
            send(1, 3),
        ])
        .expect("wrong object type");
        assert_eq!(wrong_object.snapshot.endpoint, EndpointState::Idle);
        assert_eq!(wrong_object.snapshot.counts.safe_rejects, 1);
    }

    #[test]
    fn wrong_rights_and_revoked_capabilities_are_denied() {
        let wrong = run_once(&[
            IpcOperation::UseWrongRights {
                actor: 0,
                direction: Direction::Send,
            },
            send(1, 1),
        ])
        .expect("wrong rights");
        assert_eq!(wrong.snapshot.endpoint, EndpointState::Idle);
        assert_eq!(wrong.snapshot.counts.safe_rejects, 1);

        let revoked =
            run_once(&[IpcOperation::RevokeCap { actor: 0 }, send(1, 1)]).expect("revoked");
        assert_eq!(revoked.snapshot.endpoint, EndpointState::Idle);
        assert_eq!(revoked.snapshot.counts.safe_rejects, 1);
    }

    #[test]
    fn repeated_cancel_is_stable_and_clears_pending_message() {
        let outcome = run_once(&[
            send(8, 0x5a),
            IpcOperation::Cancel { actor: 0 },
            IpcOperation::Cancel { actor: 0 },
        ])
        .expect("cancel");
        assert_eq!(outcome.snapshot.endpoint, EndpointState::Idle);
        assert!(outcome.snapshot.pending.is_none());
        assert_eq!(outcome.snapshot.task_states[0], ThreadState::Ready);
        assert_eq!(outcome.snapshot.counts.safe_rejects, 1);
    }

    #[test]
    fn exact_copy_and_source_alias_invariants_hold() {
        let result = evaluate_operations(&[send(MSG_MAX_BYTES, 0xa5), recv()]);
        assert_ne!(result.class, ResultClass::KernelInvariantFailure);
    }

    #[test]
    fn faulted_and_killed_tasks_are_never_readied_by_ipc() {
        let outcome = run_once(&[
            IpcOperation::FaultTask { actor: 0 },
            send(1, 1),
            IpcOperation::KillTask { actor: 1 },
            recv(),
            IpcOperation::MakeReady { actor: 0 },
            IpcOperation::MakeReady { actor: 1 },
        ])
        .expect("terminal task sequence");
        assert_eq!(outcome.snapshot.task_states[0], ThreadState::Faulted);
        assert_eq!(outcome.snapshot.task_states[1], ThreadState::Killed);
        assert_eq!(outcome.snapshot.endpoint, EndpointState::Idle);
    }

    #[test]
    fn identical_sequence_has_identical_final_state() {
        let operations = decode_operations(19, &[0, 2, 3, 4, 9, 1, 0, 0]);
        assert_eq!(
            run_once(&operations).expect("first"),
            run_once(&operations).expect("second")
        );
    }

    struct FailingIpcTarget;

    impl FuzzTarget for FailingIpcTarget {
        fn name(&self) -> &'static str {
            NAME
        }

        fn evaluate(&mut self, _case: &FuzzCase) -> CaseResult {
            CaseResult::invariant_failure("IPC-INV-test injected failure")
        }
    }

    #[test]
    fn ipc_invariant_failure_artifact_replays_exact_input() {
        let directory = temp_dir();
        let config = RunConfig {
            target: NAME.to_string(),
            seed: 20260904,
            iterations: 1,
            max_len: 128,
            failure_dir: directory.clone(),
        };
        let summary = Engine
            .run(&config, Corpus::empty(), &mut FailingIpcTarget)
            .expect("write IPC artifact");
        assert_eq!(summary.exit_code(), 1);

        let path = directory.join("ipc/seed-20260904-iteration-0.txt");
        let artifact = FailureArtifact::read(&path).expect("read IPC artifact");
        assert_eq!(artifact.target, NAME);
        assert_eq!(artifact.seed, 20260904);
        assert_eq!(artifact.iteration, 0);
        assert!(artifact.input.is_empty());

        let replay = Engine
            .replay(&artifact, &mut FailingIpcTarget)
            .expect("replay exact IPC input");
        assert_eq!(replay.exit_code(), 1);
        assert_eq!(artifact.to_case().input, Vec::<u8>::new());
        fs::remove_dir_all(directory).expect("remove test directory");
    }
}
