//! Capability-controlled IPC tests (AXIOM-CAP-003).
//!
//! Requirement reference: docs/06_CAPABILITY_MODEL.md §5,
//! docs/08_IPC_MODEL.md, docs/04_SYSCALL_MODEL.md (sys_send/sys_recv).
//!
//! Definition of done: IPC without capability fails; IPC with
//! capability succeeds. Every failure leaves the endpoint untouched.

use kernel::caps::table::CapError;
use kernel::caps::{Capability, CapTable, ObjectRef, ObjectType, Rights};
use kernel::ipc::{
    recv_checked, send_checked, Endpoint, EndpointId, EndpointState, IpcCapError, Message,
    RecvOutcome, SendOutcome,
};
use kernel::thread::ThreadId;

const EP_ID: u32 = 7;

fn endpoint() -> Endpoint {
    Endpoint::new(EndpointId(EP_ID))
}

fn table_with(index: usize, object_id: u32, rights: Rights) -> CapTable {
    let mut t = CapTable::new();
    t.insert(
        index,
        Capability::new(ObjectRef { object_type: ObjectType::Endpoint, object_id }, rights),
    )
    .unwrap();
    t
}

fn msg(sender: u32) -> Message {
    Message::new(ThreadId(sender), b"payload").unwrap()
}

#[test]
fn ipc_without_capability_fails() {
    let empty = CapTable::new();
    let mut ep = endpoint();
    let r = send_checked(&empty, 0, &mut ep, ThreadId(1), msg(1));
    assert_eq!(r, Err(IpcCapError::Cap(CapError::EmptySlot)));
    assert_eq!(ep.state(), EndpointState::Idle, "endpoint never touched on failure");

    let r = recv_checked(&empty, 0, &mut ep, ThreadId(2));
    assert_eq!(r, Err(IpcCapError::Cap(CapError::EmptySlot)));
    assert_eq!(ep.state(), EndpointState::Idle);
}

#[test]
fn ipc_with_capability_succeeds() {
    let sender_table = table_with(0, EP_ID, Rights::SEND);
    let receiver_table = table_with(0, EP_ID, Rights::RECEIVE);
    let mut ep = endpoint();

    // Sender arrives first and parks.
    let out = send_checked(&sender_table, 0, &mut ep, ThreadId(1), msg(1)).unwrap();
    assert_eq!(out, SendOutcome::Blocked);

    // Receiver with Receive right completes the rendezvous.
    match recv_checked(&receiver_table, 0, &mut ep, ThreadId(2)).unwrap() {
        RecvOutcome::Received { msg, unblock } => {
            assert_eq!(msg.data(), b"payload");
            assert_eq!(unblock, ThreadId(1));
        }
        other => panic!("expected Received, got {other:?}"),
    }
    assert_eq!(ep.state(), EndpointState::Idle);
}

#[test]
fn send_requires_send_right() {
    // Receive-only capability must not authorize sending.
    let t = table_with(0, EP_ID, Rights::RECEIVE);
    let mut ep = endpoint();
    assert_eq!(
        send_checked(&t, 0, &mut ep, ThreadId(1), msg(1)),
        Err(IpcCapError::Cap(CapError::InsufficientRights))
    );
    assert_eq!(ep.state(), EndpointState::Idle);
}

#[test]
fn recv_requires_receive_right() {
    let t = table_with(0, EP_ID, Rights::SEND);
    let mut ep = endpoint();
    assert_eq!(
        recv_checked(&t, 0, &mut ep, ThreadId(2)),
        Err(IpcCapError::Cap(CapError::InsufficientRights))
    );
    assert_eq!(ep.state(), EndpointState::Idle);
}

#[test]
fn capability_is_bound_to_its_endpoint() {
    // Send right on endpoint 99 grants nothing on endpoint 7.
    let t = table_with(0, 99, Rights::SEND);
    let mut ep = endpoint();
    assert_eq!(
        send_checked(&t, 0, &mut ep, ThreadId(1), msg(1)),
        Err(IpcCapError::WrongEndpoint)
    );
    assert_eq!(ep.state(), EndpointState::Idle);
}

#[test]
fn wrong_object_type_rejected() {
    let mut t = CapTable::new();
    t.insert(
        0,
        Capability::new(
            ObjectRef { object_type: ObjectType::Thread, object_id: EP_ID },
            Rights::SEND,
        ),
    )
    .unwrap();
    let mut ep = endpoint();
    assert_eq!(
        send_checked(&t, 0, &mut ep, ThreadId(1), msg(1)),
        Err(IpcCapError::Cap(CapError::WrongObjectType))
    );
}

#[test]
fn revoked_capability_stops_ipc() {
    let mut t = table_with(0, EP_ID, Rights::SEND);
    let mut ep = endpoint();
    assert!(send_checked(&t, 0, &mut ep, ThreadId(1), msg(1)).is_ok());
    // Rendezvous completes; then the capability is revoked.
    let rt = table_with(0, EP_ID, Rights::RECEIVE);
    recv_checked(&rt, 0, &mut ep, ThreadId(2)).unwrap();
    t.revoke(0).unwrap();
    assert_eq!(
        send_checked(&t, 0, &mut ep, ThreadId(1), msg(1)),
        Err(IpcCapError::Cap(CapError::EmptySlot)),
        "use after revocation fails closed"
    );
}
