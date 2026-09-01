//! Bounded network-service protocol model (AXIOM-NET-003).
//!
//! The target services use the same byte commands in sectioned U-mode code.
//! This host-testable model fixes the accepted vocabulary and size bound
//! without placing network policy in the kernel dispatcher.

/// Global on-target IPC bound; kept equal to dispatch::IPC_MSG_MAX.
pub const MAX_NETWORK_MESSAGE_BYTES: usize = 128;

/// Size represented by the deterministic synthetic test packet.
pub const TEST_PACKET_BYTES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolError {
    Malformed,
    TooLarge,
    Unsupported,
}

impl ProtocolError {
    pub const fn reply(self) -> &'static [u8] {
        match self {
            ProtocolError::Malformed => b"ERR malformed",
            ProtocolError::TooLarge => b"ERR too_large",
            ProtocolError::Unsupported => b"ERR unsupported",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceRequest {
    Status,
    Stats,
    SendTest,
    RxCount,
    Fault,
    Restart,
}

impl ServiceRequest {
    /// Decode an exact shell-to-net_service request.
    pub fn decode(message: &[u8]) -> Result<Self, ProtocolError> {
        validate(message)?;
        match message {
            b"NET_STATUS" => Ok(Self::Status),
            b"NET_STATS" => Ok(Self::Stats),
            b"NET_SEND_TEST" => Ok(Self::SendTest),
            b"NET_RX_COUNT" => Ok(Self::RxCount),
            b"NET_FAULT" => Ok(Self::Fault),
            b"NET_RESTART" => Ok(Self::Restart),
            _ => Err(ProtocolError::Unsupported),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverRequest {
    Status,
    TxTest,
    RxCount,
    Fault,
    Restart,
}

impl DriverRequest {
    /// Decode an exact net_service-to-net_driver_service request.
    pub fn decode(message: &[u8]) -> Result<Self, ProtocolError> {
        validate(message)?;
        match message {
            b"DRV_STATUS" => Ok(Self::Status),
            b"DRV_TX_TEST" => Ok(Self::TxTest),
            b"DRV_RX_COUNT" => Ok(Self::RxCount),
            b"DRV_FAULT" => Ok(Self::Fault),
            b"DRV_RESTART" => Ok(Self::Restart),
            _ => Err(ProtocolError::Unsupported),
        }
    }
}

fn validate(message: &[u8]) -> Result<(), ProtocolError> {
    if message.is_empty() {
        Err(ProtocolError::Malformed)
    } else if message.len() > MAX_NETWORK_MESSAGE_BYTES {
        Err(ProtocolError::TooLarge)
    } else {
        Ok(())
    }
}

/// Declarative rights carried by network endpoint capabilities.
///
/// Transport SEND/RECEIVE checks remain generic kernel mechanism. These
/// rights describe which network-service operation the holder may request;
/// the U-mode service enforces that policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NetworkRights(u16);

impl NetworkRights {
    pub const NONE: Self = Self(0);
    pub const STATUS: Self = Self(1 << 0);
    pub const TX: Self = Self(1 << 1);
    pub const RX: Self = Self(1 << 2);
    pub const CONTROL: Self = Self(1 << 3);

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn contains(self, required: Self) -> bool {
        self.0 & required.0 == required.0
    }

    pub const fn diminish(self, removed: Self) -> Self {
        Self(self.0 & !removed.0)
    }

    pub const fn bits(self) -> u16 {
        self.0
    }
}

impl Default for NetworkRights {
    fn default() -> Self {
        Self::NONE
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverModelError {
    CounterOverflow,
}

/// Bounded synthetic TX/RX model used by v1.7.
///
/// It stores counters only; no packet payload or queue can grow. One test
/// transmission also models one deterministic loopback reception.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SyntheticDriverModel {
    tx: u64,
    rx: u64,
}

impl SyntheticDriverModel {
    pub const fn tx(self) -> u64 {
        self.tx
    }

    pub const fn rx(self) -> u64 {
        self.rx
    }

    pub fn send_test(&mut self) -> Result<usize, DriverModelError> {
        let Some(next_tx) = self.tx.checked_add(1) else {
            return Err(DriverModelError::CounterOverflow);
        };
        let Some(next_rx) = self.rx.checked_add(1) else {
            return Err(DriverModelError::CounterOverflow);
        };
        self.tx = next_tx;
        self.rx = next_rx;
        Ok(TEST_PACKET_BYTES)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_protocol_accepts_only_exact_commands() {
        let cases = [
            (b"NET_STATUS".as_slice(), ServiceRequest::Status),
            (b"NET_STATS".as_slice(), ServiceRequest::Stats),
            (b"NET_SEND_TEST".as_slice(), ServiceRequest::SendTest),
            (b"NET_RX_COUNT".as_slice(), ServiceRequest::RxCount),
            (b"NET_FAULT".as_slice(), ServiceRequest::Fault),
            (b"NET_RESTART".as_slice(), ServiceRequest::Restart),
        ];
        for (wire, expected) in cases {
            assert_eq!(ServiceRequest::decode(wire), Ok(expected));
        }
    }

    #[test]
    fn driver_protocol_accepts_only_exact_commands() {
        let cases = [
            (b"DRV_STATUS".as_slice(), DriverRequest::Status),
            (b"DRV_TX_TEST".as_slice(), DriverRequest::TxTest),
            (b"DRV_RX_COUNT".as_slice(), DriverRequest::RxCount),
            (b"DRV_FAULT".as_slice(), DriverRequest::Fault),
            (b"DRV_RESTART".as_slice(), DriverRequest::Restart),
        ];
        for (wire, expected) in cases {
            assert_eq!(DriverRequest::decode(wire), Ok(expected));
        }
    }

    #[test]
    fn malformed_and_unknown_requests_fail_safely() {
        assert_eq!(ServiceRequest::decode(b""), Err(ProtocolError::Malformed));
        assert_eq!(
            ServiceRequest::decode(b"NET_STATUS trailing"),
            Err(ProtocolError::Unsupported)
        );
        assert_eq!(
            DriverRequest::decode(b"DRV_STATUS\0"),
            Err(ProtocolError::Unsupported)
        );
        assert_eq!(
            DriverRequest::decode(b"DROP_TABLE"),
            Err(ProtocolError::Unsupported)
        );
    }

    #[test]
    fn oversized_request_is_rejected_before_parsing() {
        let oversized = [b'X'; MAX_NETWORK_MESSAGE_BYTES + 1];
        assert_eq!(
            ServiceRequest::decode(&oversized),
            Err(ProtocolError::TooLarge)
        );
        assert_eq!(
            DriverRequest::decode(&oversized),
            Err(ProtocolError::TooLarge)
        );
    }

    #[test]
    fn error_replies_are_bounded_and_stable() {
        for err in [
            ProtocolError::Malformed,
            ProtocolError::TooLarge,
            ProtocolError::Unsupported,
        ] {
            assert!(err.reply().len() <= MAX_NETWORK_MESSAGE_BYTES);
        }
        assert_eq!(ProtocolError::Malformed.reply(), b"ERR malformed");
        assert_eq!(ProtocolError::TooLarge.reply(), b"ERR too_large");
        assert_eq!(ProtocolError::Unsupported.reply(), b"ERR unsupported");
    }

    #[test]
    fn four_network_rights_are_distinct() {
        let rights = [
            NetworkRights::STATUS,
            NetworkRights::TX,
            NetworkRights::RX,
            NetworkRights::CONTROL,
        ];
        for (i, left) in rights.iter().enumerate() {
            for (j, right) in rights.iter().enumerate() {
                if i != j {
                    assert!(!left.contains(*right));
                }
            }
        }
    }

    #[test]
    fn network_rights_are_deny_by_default_and_never_amplify() {
        assert_eq!(NetworkRights::default(), NetworkRights::NONE);
        assert!(!NetworkRights::NONE.contains(NetworkRights::STATUS));
        assert!(!NetworkRights::NONE.contains(NetworkRights::TX));
        assert!(!NetworkRights::NONE.contains(NetworkRights::RX));
        assert!(!NetworkRights::NONE.contains(NetworkRights::CONTROL));

        let operator = NetworkRights::STATUS
            .union(NetworkRights::TX)
            .union(NetworkRights::RX)
            .union(NetworkRights::CONTROL);
        let observer = operator.diminish(NetworkRights::TX.union(NetworkRights::CONTROL));
        assert!(observer.contains(NetworkRights::STATUS));
        assert!(observer.contains(NetworkRights::RX));
        assert!(!observer.contains(NetworkRights::TX));
        assert!(!observer.contains(NetworkRights::CONTROL));
    }

    #[test]
    fn synthetic_test_packet_has_bounded_deterministic_counters() {
        let mut driver = SyntheticDriverModel::default();
        assert_eq!((driver.tx(), driver.rx()), (0, 0));
        assert_eq!(driver.send_test(), Ok(TEST_PACKET_BYTES));
        assert_eq!((driver.tx(), driver.rx()), (1, 1));
        assert_eq!(driver.send_test(), Ok(TEST_PACKET_BYTES));
        assert_eq!((driver.tx(), driver.rx()), (2, 2));

        let saturated = SyntheticDriverModel {
            tx: u64::MAX,
            rx: 7,
        };
        let mut copy = saturated;
        assert_eq!(copy.send_test(), Err(DriverModelError::CounterOverflow));
        assert_eq!(copy, saturated, "overflow cannot partially update counters");
    }
}
