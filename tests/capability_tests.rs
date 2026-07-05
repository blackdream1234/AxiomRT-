//! Capability table tests (AXIOM-CAP-002).
//!
//! Requirement reference: docs/06_CAPABILITY_MODEL.md §4,
//! docs/14_TEST_STRATEGY.md.
//!
//! Mandatory cases from the task: lookup valid capability, reject
//! missing capability, reject insufficient rights, reject wrong object
//! type. Error behavior is explicit for every case.

use kernel::caps::table::{CapError, CapTable, CAP_TABLE_SLOTS};
use kernel::caps::{Capability, ObjectRef, ObjectType, Rights};

fn endpoint_cap(id: u32, rights: Rights) -> Capability {
    Capability::new(ObjectRef { object_type: ObjectType::Endpoint, object_id: id }, rights)
}

#[test]
fn lookup_valid_capability() {
    let mut t = CapTable::new();
    t.insert(0, endpoint_cap(7, Rights::SEND.union(Rights::RECEIVE))).unwrap();
    let obj = t.lookup(0, ObjectType::Endpoint, Rights::SEND).unwrap();
    assert_eq!(obj.object_id, 7);
    assert_eq!(obj.object_type, ObjectType::Endpoint);
}

#[test]
fn reject_missing_capability() {
    let t = CapTable::new();
    assert_eq!(
        t.lookup(0, ObjectType::Endpoint, Rights::SEND),
        Err(CapError::EmptySlot),
        "empty slot never grants access"
    );
    assert_eq!(
        t.lookup(CAP_TABLE_SLOTS, ObjectType::Endpoint, Rights::SEND),
        Err(CapError::InvalidIndex),
        "out-of-range index never grants access"
    );
}

#[test]
fn reject_insufficient_rights() {
    let mut t = CapTable::new();
    t.insert(1, endpoint_cap(7, Rights::SEND)).unwrap();
    assert_eq!(
        t.lookup(1, ObjectType::Endpoint, Rights::RECEIVE),
        Err(CapError::InsufficientRights),
        "Send-only capability cannot receive"
    );
    assert_eq!(
        t.lookup(1, ObjectType::Endpoint, Rights::SEND.union(Rights::RECEIVE)),
        Err(CapError::InsufficientRights),
        "subset check requires ALL required rights"
    );
}

#[test]
fn reject_wrong_object_type() {
    let mut t = CapTable::new();
    t.insert(2, endpoint_cap(7, Rights::SEND)).unwrap();
    assert_eq!(
        t.lookup(2, ObjectType::Thread, Rights::SEND),
        Err(CapError::WrongObjectType),
        "type confusion is structurally rejected"
    );
}

#[test]
fn revoked_capability_fails_lookup() {
    let mut t = CapTable::new();
    t.insert(3, endpoint_cap(9, Rights::SEND)).unwrap();
    t.revoke(3).unwrap();
    assert_eq!(t.lookup(3, ObjectType::Endpoint, Rights::SEND), Err(CapError::EmptySlot));
    assert_eq!(t.revoke(3), Err(CapError::EmptySlot), "double revoke is explicit");
}

#[test]
fn insert_rules() {
    let mut t = CapTable::new();
    t.insert(4, endpoint_cap(1, Rights::SEND)).unwrap();
    assert_eq!(
        t.insert(4, endpoint_cap(2, Rights::SEND)),
        Err(CapError::SlotOccupied),
        "no silent overwrite of authority"
    );
    assert_eq!(t.insert(CAP_TABLE_SLOTS, endpoint_cap(1, Rights::SEND)), Err(CapError::InvalidIndex));
}

#[test]
fn query_reveals_own_authority_only_as_metadata() {
    let mut t = CapTable::new();
    t.insert(5, endpoint_cap(1, Rights::SEND)).unwrap();
    let (ty, rights) = t.query(5).unwrap();
    assert_eq!(ty, ObjectType::Endpoint);
    assert!(rights.contains(Rights::SEND));
    assert_eq!(t.query(6), Err(CapError::EmptySlot), "probing empty slots is a clean error");
}
