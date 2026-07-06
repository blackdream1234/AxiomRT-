//! IPC integration tests (AXIOM-IPC-002).
//!
//! Requirement reference: docs/08_IPC_MODEL.md §4,
//! docs/14_TEST_STRATEGY.md.
//!
//! Host-run, deterministic, no hardware dependency. Exercises the
//! synchronous rendezvous semantics: blocking on both sides, bounded
//! copy, no shared memory, cancellation on kill.

use kernel::ipc::{
    cancel, recv, send, CancelOutcome, Endpoint, EndpointId, EndpointState, IpcError, Message,
    MessageError, RecvOutcome, SendOutcome, MSG_MAX_BYTES,
};
use kernel::thread::ThreadId;

fn ep() -> Endpoint {
    Endpoint::new(EndpointId(1))
}
fn msg(sender: u32, data: &[u8]) -> Message {
    Message::new(ThreadId(sender), data).unwrap()
}

#[test]
fn send_blocks_if_no_receiver() {
    let mut e = ep();
    let out = send(&mut e, ThreadId(1), msg(1, b"hi")).unwrap();
    assert_eq!(
        out,
        SendOutcome::Blocked,
        "send must block when nobody receives"
    );
    assert_eq!(
        e.state(),
        EndpointState::SenderWaiting {
            sender: ThreadId(1)
        }
    );
}

#[test]
fn recv_blocks_if_no_sender() {
    let mut e = ep();
    let out = recv(&mut e, ThreadId(2)).unwrap();
    assert_eq!(
        out,
        RecvOutcome::Blocked,
        "receive must block when nobody sends"
    );
    assert_eq!(
        e.state(),
        EndpointState::ReceiverWaiting {
            receiver: ThreadId(2)
        }
    );
}

#[test]
fn sender_first_rendezvous_delivers_and_unblocks() {
    let mut e = ep();
    assert_eq!(
        send(&mut e, ThreadId(1), msg(1, b"evt")).unwrap(),
        SendOutcome::Blocked
    );
    match recv(&mut e, ThreadId(2)).unwrap() {
        RecvOutcome::Received { msg, unblock } => {
            assert_eq!(msg.data(), b"evt");
            assert_eq!(
                msg.header().sender,
                ThreadId(1),
                "kernel-written sender identity"
            );
            assert_eq!(unblock, ThreadId(1), "the parked sender is released");
        }
        other => panic!("expected Received, got {other:?}"),
    }
    assert_eq!(e.state(), EndpointState::Idle, "rendezvous complete");
}

#[test]
fn receiver_first_rendezvous_delivers_to_waiting_receiver() {
    let mut e = ep();
    assert_eq!(recv(&mut e, ThreadId(2)).unwrap(), RecvOutcome::Blocked);
    match send(&mut e, ThreadId(1), msg(1, b"ping")).unwrap() {
        SendOutcome::Delivered { to, msg } => {
            assert_eq!(to, ThreadId(2));
            assert_eq!(msg.data(), b"ping");
        }
        other => panic!("expected Delivered, got {other:?}"),
    }
    assert_eq!(e.state(), EndpointState::Idle);
}

#[test]
fn bounded_no_second_sender_queue() {
    let mut e = ep();
    send(&mut e, ThreadId(1), msg(1, b"a")).unwrap();
    assert_eq!(
        send(&mut e, ThreadId(3), msg(3, b"b")),
        Err(IpcError::Busy),
        "v0.1 has no sender queue: bounded by structure"
    );
    assert_eq!(
        send(&mut e, ThreadId(1), msg(1, b"a")),
        Err(IpcError::AlreadyWaiting)
    );
}

#[test]
fn bounded_message_size_enforced_before_copy() {
    let too_big = [0u8; MSG_MAX_BYTES + 1];
    assert_eq!(
        Message::new(ThreadId(1), &too_big),
        Err(MessageError::TooLarge)
    );
}

#[test]
fn copy_semantics_no_shared_memory() {
    let mut e = ep();
    let mut source = *b"secret42";
    send(
        &mut e,
        ThreadId(1),
        Message::new(ThreadId(1), &source).unwrap(),
    )
    .unwrap();
    // Sender mutates its buffer while parked: must not affect delivery.
    source.fill(0);
    match recv(&mut e, ThreadId(2)).unwrap() {
        RecvOutcome::Received { msg, .. } => {
            assert_eq!(msg.data(), b"secret42", "payload was copied at send time");
        }
        other => panic!("expected Received, got {other:?}"),
    }
}

#[test]
fn deterministic_same_history_same_outcome() {
    for _ in 0..3 {
        let mut e = ep();
        assert_eq!(
            send(&mut e, ThreadId(1), msg(1, b"x")).unwrap(),
            SendOutcome::Blocked
        );
        let r = recv(&mut e, ThreadId(2)).unwrap();
        match r {
            RecvOutcome::Received { unblock, .. } => assert_eq!(unblock, ThreadId(1)),
            other => panic!("expected Received, got {other:?}"),
        }
    }
}

#[test]
fn cancel_parked_sender_drops_message_undelivered() {
    let mut e = ep();
    send(&mut e, ThreadId(1), msg(1, b"gone")).unwrap();
    assert_eq!(cancel(&mut e, ThreadId(1)), CancelOutcome::Cancelled);
    assert_eq!(e.state(), EndpointState::Idle);
    // The receiver arriving later observes nothing: it blocks.
    assert_eq!(recv(&mut e, ThreadId(2)).unwrap(), RecvOutcome::Blocked);
}

#[test]
fn cancel_non_waiting_thread_is_noop() {
    let mut e = ep();
    send(&mut e, ThreadId(1), msg(1, b"m")).unwrap();
    assert_eq!(cancel(&mut e, ThreadId(9)), CancelOutcome::NotWaiting);
    assert_eq!(
        e.state(),
        EndpointState::SenderWaiting {
            sender: ThreadId(1)
        }
    );
}
