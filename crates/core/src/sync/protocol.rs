//! Messages two devices exchange inside the encrypted channel, and how they
//! are cut into Noise messages. MessagePack with field names, like the vault.

use serde::{Deserialize, Serialize};

use super::state::{RecordState, TrustedPeer};
use crate::error::{Error, Result};

/// Bumped on an incompatible change; devices on different versions refuse
/// to sync and ask to update instead.
/// 2: notices may carry the `security` category (vault transfer).
/// 3: every purchase of a shared event is an entry of its own
/// (`items.<id>.purchases.<id>`); a device on 2 would overwrite a whole item
/// and lose a parallel purchase (`docs/sync/compatibility.md`).
/// 4: the first [`Hello`] carries no identity; each side tells who it is in
/// an [`Identity`] only after the other side's vault proof passed, and the
/// pairing code binds a committed nonce of each side (`docs/sync/pairing.md`).
pub const PROTOCOL_VERSION: u32 = 4;

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
    /// Who the sender is: the listener sends it once the dialer's proof and
    /// versions passed, the dialer answers with its own (protocol 4).
    Identity(Identity),
    /// The user pressed "Pair" on the sender: show the code here too.
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

/// Protocol 4 sends the identity fields empty: they stay only so a protocol 3
/// build still decodes the message and refuses on the version. A protocol 3
/// build fills them; they are read only for the text of that refusal.
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
    /// From the dialer: `SHA256(nonce)` of the nonce its [`Identity`] will
    /// carry, fixed before it sees the listener's nonce. Empty from the listener.
    #[serde(default)]
    pub commit: Vec<u8>,
}

impl Hello {
    /// The first message of protocol 4: versions and the proof, nothing about the device.
    pub fn first(
        protocol: u32,
        format: u32,
        proof: Vec<u8>,
        app_version: String,
        commit: Vec<u8>,
    ) -> Self {
        Self {
            protocol,
            format,
            device_id: String::new(),
            device_name: String::new(),
            proof,
            now_ms: 0,
            main: false,
            prefer_main: false,
            needs_pairing: false,
            rank: 0,
            instance: String::new(),
            unreached: false,
            app_version,
            commit,
        }
    }
}

/// Who the sender is, sent only to a device that proved it holds the vault.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    pub device_id: String,
    pub device_name: String,
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
    pub instance: String,
    /// The sender tried to connect to the receiver and could not (its
    /// firewall drops incoming connections).
    pub unreached: bool,
    /// [`super::session::NONCE_LEN`] random bytes for the pairing code; the
    /// dialer's must match the commit of its `Hello`.
    pub nonce: Vec<u8>,
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
            commit: Vec::new(),
        };
        let message = Message::Hello(hello.clone());
        assert_eq!(decode(&encode(&message).unwrap()).unwrap(), message);
        hello.app_version.clear();
        assert_eq!(hello.app_version, "");
    }

    fn identity() -> Identity {
        Identity {
            device_id: "d".into(),
            device_name: "Phone".into(),
            now_ms: 42,
            main: true,
            prefer_main: true,
            needs_pairing: true,
            rank: 3,
            instance: "phone._mwt".into(),
            unreached: true,
            nonce: vec![7; 32],
        }
    }

    #[test]
    fn an_identity_round_trips() {
        let message = Message::Identity(identity());
        assert_eq!(decode(&encode(&message).unwrap()).unwrap(), message);
    }

    /// The `Hello` of a protocol 3 build: every field, identity included, and no commit.
    #[test]
    fn a_protocol_three_hello_still_decodes() {
        #[derive(Serialize)]
        struct HelloV3 {
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
            instance: String,
            unreached: bool,
            app_version: String,
        }
        let old = HelloV3 {
            protocol: 3,
            format: 3,
            device_id: "d".into(),
            device_name: "Old phone".into(),
            proof: vec![1, 2, 3],
            now_ms: 7,
            main: false,
            prefer_main: false,
            needs_pairing: true,
            rank: 1,
            instance: "old._mwt".into(),
            unreached: false,
            app_version: "0.1.0-30".into(),
        };
        let bytes = rmp_serde::to_vec_named(&serde_json::json!({ "hello": old })).unwrap();
        let Message::Hello(hello) = decode(&bytes).unwrap() else {
            panic!("not a hello");
        };
        assert_eq!(hello.protocol, 3);
        assert_eq!(hello.device_name, "Old phone");
        assert_eq!(hello.app_version, "0.1.0-30");
        assert!(hello.commit.is_empty());
    }

    /// What a protocol 3 build makes of the first message of a protocol 4
    /// one: it decodes (so it can refuse on the version), and it learns no
    /// identity. The old structs are frozen copies of the protocol 3 ones.
    #[test]
    fn a_protocol_four_hello_decodes_in_an_old_build_without_identity() {
        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct HelloV3 {
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
            #[serde(default)]
            instance: String,
            #[serde(default)]
            unreached: bool,
            #[serde(default)]
            app_version: String,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "snake_case")]
        enum MessageV3 {
            Hello(HelloV3),
        }
        let first = Hello::first(
            PROTOCOL_VERSION,
            3,
            vec![9; 32],
            "0.2.0".into(),
            vec![5; 32],
        );
        let bytes = encode(&Message::Hello(first)).unwrap();
        let MessageV3::Hello(seen) = rmp_serde::from_slice::<MessageV3>(&bytes).unwrap();
        assert_eq!(seen.protocol, 4);
        assert_eq!(seen.app_version, "0.2.0");
        assert_eq!(seen.device_id, "");
        assert_eq!(seen.device_name, "");
        assert_eq!(seen.instance, "");
        assert_eq!(seen.now_ms, 0);
        assert!(!seen.main && !seen.prefer_main && !seen.needs_pairing && !seen.unreached);
        assert_eq!(seen.rank, 0);
        // And a new build reads the commit back.
        let Message::Hello(back) = decode(&bytes).unwrap() else {
            panic!("not a hello");
        };
        assert_eq!(back.commit, vec![5; 32]);
    }

    #[test]
    fn an_unknown_message_variant_is_an_error_not_a_panic() {
        let bytes =
            rmp_serde::to_vec_named(&serde_json::json!({"from_a_newer_build": {"x": 1}})).unwrap();
        assert!(decode(&bytes).is_err());
    }
}
