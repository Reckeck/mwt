//! Messages two devices exchange inside the encrypted channel, and how they
//! are cut into Noise messages. MessagePack with field names, like the vault.

use serde::{Deserialize, Serialize};

use super::state::{RecordState, TrustedPeer};
use crate::error::{Error, Result};

/// Bumped on an incompatible change; devices on different versions refuse
/// to sync and ask to update instead.
pub const PROTOCOL_VERSION: u32 = 1;

/// Largest message after reassembly: a manifest or records batch is split
/// well below it, so anything larger is a broken or hostile peer.
pub const MAX_MESSAGE: usize = 16 * 1024 * 1024;

/// Records per [`Message::Records`]; keeps one message well under the limit.
pub const RECORDS_PER_MESSAGE: usize = 500;
/// Manifest entries per [`Message::Manifest`].
pub const MANIFEST_PER_MESSAGE: usize = 5_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Message {
    /// First message each way after the handshake.
    Hello(Hello),
    /// The user's answer to the pairing code on this side.
    Pairing {
        accept: bool,
    },
    /// Trusted devices, so a «forget» reaches everyone.
    Peers {
        peers: Vec<TrustedPeer>,
    },
    /// One part of the manifest (`collection:key` → digest).
    Manifest {
        entries: Vec<(String, u64)>,
        last: bool,
    },
    /// Records this side wants in full.
    Request {
        keys: Vec<String>,
    },
    /// Records in full: the answer to a request (`last` closes it).
    Records {
        records: Vec<(String, RecordState)>,
        last: bool,
    },
    /// Records changed by a save during a live session.
    Changed {
        records: Vec<(String, RecordState)>,
    },
    /// Whether this device's window has focus (who shows a notification).
    Presence {
        focused: bool,
    },
    /// The sender's clock, for the jump check.
    Time {
        now_ms: u64,
    },
    /// A notification was read here: dismiss it everywhere.
    NoticeRead {
        key: String,
    },
    /// Hand over the main role (the preferred device came back).
    TakeMain,
    Bye {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
    pub protocol: u32,
    /// `VAULT_FORMAT_VERSION` of the sender.
    pub format: u32,
    pub device_id: String,
    pub device_name: String,
    /// [`super::session::Channel::proof`]: the sender holds this vault open.
    pub proof: Vec<u8>,
    pub now_ms: u64,
    /// The sender is the main device of its group.
    pub main: bool,
    pub prefer_main: bool,
    /// The sender will show a code and wait for the user (it does not trust us yet).
    pub needs_pairing: bool,
}

pub fn encode(message: &Message) -> Result<Vec<u8>> {
    rmp_serde::to_vec_named(message).map_err(|err| Error::Serialize(err.to_string()))
}

pub fn decode(bytes: &[u8]) -> Result<Message> {
    if bytes.len() > MAX_MESSAGE {
        return Err(Error::InvalidInput("sync message too large".into()));
    }
    rmp_serde::from_slice(bytes).map_err(|err| Error::InvalidInput(format!("sync message: {err}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_round_trip() {
        let message = Message::Records {
            records: vec![(
                "stores:a".into(),
                RecordState {
                    value: Some(serde_json::json!({"id": "a"})),
                    ..Default::default()
                },
            )],
            last: true,
        };
        assert_eq!(decode(&encode(&message).unwrap()).unwrap(), message);
    }

    #[test]
    fn garbage_is_an_error_not_a_panic() {
        assert!(decode(&[0xc1, 0xff, 0x00]).is_err());
        assert!(decode(&[]).is_err());
    }
}
