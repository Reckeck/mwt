//! Messages two devices exchange inside the encrypted channel, and how they
//! are cut into Noise messages. MessagePack with field names, like the vault.

use serde::{Deserialize, Serialize};

use super::state::{RecordState, TrustedPeer};
use crate::error::{Error, Result};

/// Bumped on an incompatible change; devices on different versions refuse
/// to sync and ask to update instead.
/// 2: notices may carry the `security` category (vault transfer).
pub const PROTOCOL_VERSION: u32 = 2;

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
    /// The user pressed «Сопрячь» on the sender: show the code here too.
    PairRequest,
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
    /// Who shows a system notification. Client to main: the client's window
    /// has focus. Main to client: some other device of the group has focus.
    Presence {
        focused: bool,
    },
    /// The sender's clock, for the jump check.
    Time {
        now_ms: u64,
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
    /// From the main device: the receiver's place in the queue to take over
    /// when the main device goes (0 = first).
    pub rank: u32,
    /// The sender's mDNS instance: the main device calls back the devices it
    /// sees but has no link with.
    #[serde(default)]
    pub instance: String,
    /// The sender tried to connect to the receiver and could not (its
    /// firewall drops incoming connections).
    #[serde(default)]
    pub unreached: bool,
    /// The sender's app version, for the message of a refusal. Informational:
    /// never decides anything. Empty from a build that does not send it.
    #[serde(default)]
    pub app_version: String,
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

    /// A `Hello` as a build from before `instance`, `unreached` and `app_version` wrote it.
    #[test]
    fn an_older_hello_without_new_fields_decodes() {
        #[derive(Serialize)]
        struct OldHello {
            protocol: u32,
            format: u32,
            device_id: String,
            device_name: String,
            proof: Vec<u8>,
            now_ms: u64,
            main: bool,
            prefer_main: bool,
            needs_pairing: bool,
            rank: u32,
        }
        let old = OldHello {
            protocol: PROTOCOL_VERSION,
            format: 3,
            device_id: "d".into(),
            device_name: "Old".into(),
            proof: vec![1, 2, 3],
            now_ms: 7,
            main: true,
            prefer_main: false,
            needs_pairing: false,
            rank: 2,
        };
        let bytes = rmp_serde::to_vec_named(&serde_json::json!({ "hello": old })).unwrap();
        let Message::Hello(hello) = decode(&bytes).unwrap() else {
            panic!("not a hello");
        };
        assert_eq!(hello.device_name, "Old");
        assert_eq!(hello.rank, 2);
        assert_eq!(hello.instance, "");
        assert!(!hello.unreached);
        assert_eq!(hello.app_version, "");
    }

    #[test]
    fn an_app_version_round_trips_in_the_hello() {
        let mut hello = Hello {
            protocol: PROTOCOL_VERSION,
            format: 3,
            device_id: "d".into(),
            device_name: "New".into(),
            proof: vec![],
            now_ms: 1,
            main: false,
            prefer_main: false,
            needs_pairing: false,
            rank: 0,
            instance: String::new(),
            unreached: false,
            app_version: "1.2.3".into(),
        };
        let message = Message::Hello(hello.clone());
        assert_eq!(decode(&encode(&message).unwrap()).unwrap(), message);
        hello.app_version.clear();
        assert_eq!(hello.app_version, "");
    }

    #[test]
    fn an_unknown_message_variant_is_an_error_not_a_panic() {
        let bytes =
            rmp_serde::to_vec_named(&serde_json::json!({"from_a_newer_build": {"x": 1}})).unwrap();
        assert!(decode(&bytes).is_err());
    }
}
