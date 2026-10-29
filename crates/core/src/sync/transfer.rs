//! Handing a whole vault to a new device on the local network (docs/sync.md,
//! "Vault transfer"), without any I/O.
//!
//! Who gets the file can try passwords on it offline, so it only leaves when:
//!
//! 1. the user picked the receiving device (by its name) and typed the vault
//!    password again on the sending one (checked by the app);
//! 2. the receiving device accepted the request;
//! 3. the six-digit code it then shows was typed on the sending device.
//!
//! The code is checked with SPAKE2 inside the Noise channel: a device in the
//! middle gets one guess per connection and learns nothing it could test
//! offline. The PAKE key is bound to the Noise handshake hash, so a
//! confirmation cannot be carried over to another connection.

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use spake2::{Ed25519Group, Identity, Password, Spake2};

use crate::error::{Error, PeerSyncFailure, Result};

type HmacSha256 = Hmac<Sha256>;

/// Version of the messages below (not the sync protocol's).
pub const TRANSFER_PROTOCOL: u32 = 1;
/// Bytes of the vault file per message.
pub const CHUNK_BYTES: usize = 1 << 20;
/// Largest vault accepted (a guard against a peer that never stops sending).
pub const MAX_VAULT_BYTES: u64 = 1 << 30;
/// Wrong codes before the request is dropped.
pub const CODE_ATTEMPTS: u32 = 3;
/// How long a device waits to receive.
pub const RECEIVE_WINDOW_SECS: u64 = 300;

const PAKE_ID: &[u8] = b"mwt-vault-transfer-v1";

/// What the sending device proposes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Offer {
    pub protocol: u32,
    pub device_id: String,
    pub device_name: String,
    /// File name only, never a path.
    pub file_name: String,
    pub size: u64,
    #[serde(with = "serde_bytes")]
    pub sha256: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum TransferMessage {
    /// Sender → receiver, first.
    Offer(Offer),
    /// Receiver → sender: the user accepted and the code is on its screen.
    Accepted {
        device_id: String,
        device_name: String,
    },
    /// Receiver → sender: the user declined (or the code was wrong too often).
    Declined,
    /// Both: a SPAKE2 message for one attempt at the code.
    Pake {
        #[serde(with = "serde_bytes")]
        message: Vec<u8>,
    },
    /// Both: proof of the same key ([`confirmation`]).
    Confirm {
        #[serde(with = "serde_bytes")]
        mac: Vec<u8>,
    },
    /// Receiver → sender: the code did not match; this many tries remain.
    CodeWrong {
        attempts_left: u32,
    },
    /// Sender → receiver: the file, in order.
    Chunk {
        index: u32,
        #[serde(with = "serde_bytes")]
        bytes: Vec<u8>,
    },
    /// Sender → receiver: that was all.
    Done,
    /// Receiver → sender: saved, the checksum matched.
    Received,
    Bye {
        reason: String,
    },
}

pub fn encode(message: &TransferMessage) -> Result<Vec<u8>> {
    rmp_serde::to_vec_named(message).map_err(|err| Error::Serialize(err.to_string()))
}

pub fn decode(bytes: &[u8]) -> Result<TransferMessage> {
    if bytes.len() > super::protocol::MAX_MESSAGE {
        return Err(Error::InvalidInput("transfer message too large".into()));
    }
    rmp_serde::from_slice(bytes)
        .map_err(|err| Error::InvalidInput(format!("transfer message: {err}")))
}

/// A fresh six-digit code from the OS random source.
pub fn new_code() -> String {
    use rand::Rng;
    let number: u32 = rand::rngs::OsRng.gen_range(0..1_000_000);
    format!("{number:06}")
}

/// Whether `code` looks like one (six digits), before anything is sent.
pub fn is_code(code: &str) -> bool {
    code.len() == 6 && code.bytes().all(|byte| byte.is_ascii_digit())
}

/// One attempt at the code: send [`CodeAttempt::message`], then
/// [`CodeAttempt::finish`] with the other side's.
pub struct CodeAttempt {
    state: Spake2<Ed25519Group>,
    message: Vec<u8>,
}

impl CodeAttempt {
    pub fn start(code: &str) -> Self {
        let (state, message) = Spake2::<Ed25519Group>::start_symmetric(
            &Password::new(code.trim().as_bytes()),
            &Identity::new(PAKE_ID),
        );
        Self { state, message }
    }

    pub fn message(&self) -> &[u8] {
        &self.message
    }

    /// The shared key, bound to this connection. A wrong code still yields a
    /// key: the confirmations tell.
    pub fn finish(self, theirs: &[u8], handshake_hash: &[u8]) -> Result<[u8; 32]> {
        let pake = self
            .state
            .finish(theirs)
            .map_err(|_| Error::PeerSync(PeerSyncFailure::Channel))?;
        let mut mac = keyed(&pake);
        mac.update(b"mwt-transfer-key");
        mac.update(handshake_hash);
        Ok(mac.finalize().into_bytes().into())
    }
}

fn keyed(key: &[u8]) -> HmacSha256 {
    <HmacSha256 as Mac>::new_from_slice(key).unwrap_or_else(|_| {
        // HMAC takes keys of any length; this branch cannot be reached.
        <HmacSha256 as Mac>::new_from_slice(&[0]).expect("hmac key")
    })
}

fn confirmation_mac(key: &[u8; 32], sender: bool) -> HmacSha256 {
    let mut mac = keyed(key);
    mac.update(b"mwt-transfer-confirm");
    mac.update(&[u8::from(sender)]);
    mac
}

/// This side's proof of the key (`sender` = the side that sends the vault).
pub fn confirmation(key: &[u8; 32], sender: bool) -> Vec<u8> {
    confirmation_mac(key, sender)
        .finalize()
        .into_bytes()
        .to_vec()
}

/// Check the other side's proof (constant time).
pub fn verify_confirmation(key: &[u8; 32], other_is_sender: bool, mac: &[u8]) -> bool {
    confirmation_mac(key, other_is_sender)
        .verify_slice(mac)
        .is_ok()
}

pub fn sha256(bytes: &[u8]) -> Vec<u8> {
    Sha256::digest(bytes).to_vec()
}

/// The vault file split into messages.
pub fn chunks(bytes: &[u8]) -> impl Iterator<Item = TransferMessage> + '_ {
    bytes
        .chunks(CHUNK_BYTES)
        .enumerate()
        .map(|(index, part)| TransferMessage::Chunk {
            index: u32::try_from(index).unwrap_or(u32::MAX),
            bytes: part.to_vec(),
        })
}

/// A file name safe to save under: no directories, no hidden or empty name.
pub fn safe_file_name(raw: &str) -> String {
    let base = raw
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default()
        .trim()
        .trim_start_matches('.');
    let cleaned: String = base
        .chars()
        .filter(|ch| !ch.is_control() && !matches!(ch, ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .take(120)
        .collect();
    if cleaned.trim().is_empty() {
        "vault.wallet".into()
    } else {
        cleaned
    }
}

/// Collects the file on the receiving side and checks it as a whole.
pub struct Assembler {
    offer: Offer,
    bytes: Vec<u8>,
    next: u32,
}

impl Assembler {
    pub fn new(offer: Offer) -> Result<Self> {
        if offer.protocol != TRANSFER_PROTOCOL {
            return Err(Error::PeerSync(PeerSyncFailure::VersionMismatch));
        }
        if offer.size == 0 || offer.size > MAX_VAULT_BYTES || offer.sha256.len() != 32 {
            return Err(Error::PeerSync(PeerSyncFailure::NotAVault));
        }
        // Not reserved up front: the size is the sender's word until the code
        // is confirmed, so the buffer grows as parts arrive.
        Ok(Self {
            offer,
            bytes: Vec::new(),
            next: 0,
        })
    }

    /// Bytes received so far and in all.
    pub fn progress(&self) -> (u64, u64) {
        (self.bytes.len() as u64, self.offer.size)
    }

    pub fn push(&mut self, index: u32, part: &[u8]) -> Result<()> {
        let too_long = self.bytes.len() as u64 + part.len() as u64 > self.offer.size;
        if index != self.next || part.is_empty() || too_long {
            return Err(Error::PeerSync(PeerSyncFailure::Channel));
        }
        self.bytes.extend_from_slice(part);
        self.next += 1;
        Ok(())
    }

    /// The whole file: the size and checksum match and it is a vault.
    pub fn finish(self) -> Result<Vec<u8>> {
        if self.bytes.len() as u64 != self.offer.size || sha256(&self.bytes) != self.offer.sha256 {
            return Err(Error::PeerSync(PeerSyncFailure::Channel));
        }
        crate::crypto::parse_file(&self.bytes)
            .map_err(|_| Error::PeerSync(PeerSyncFailure::NotAVault))?;
        Ok(self.bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vault_bytes() -> Vec<u8> {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("a.wallet");
        crate::service::UnlockedVault::create(path.clone(), "pw").expect("vault");
        std::fs::read(path).expect("read")
    }

    fn offer(bytes: &[u8]) -> Offer {
        Offer {
            protocol: TRANSFER_PROTOCOL,
            device_id: "d".into(),
            device_name: "Сочный Апельсин".into(),
            file_name: "a.wallet".into(),
            size: bytes.len() as u64,
            sha256: sha256(bytes),
        }
    }

    fn keys(sender_code: &str, receiver_code: &str, hash: &[u8]) -> ([u8; 32], [u8; 32]) {
        let sender = CodeAttempt::start(sender_code);
        let receiver = CodeAttempt::start(receiver_code);
        let to_receiver = sender.message().to_vec();
        let to_sender = receiver.message().to_vec();
        (
            sender.finish(&to_sender, hash).expect("sender"),
            receiver.finish(&to_receiver, hash).expect("receiver"),
        )
    }

    #[test]
    fn the_right_code_confirms_on_both_sides() {
        let (sender, receiver) = keys("482913", "482913", b"hash");
        assert_eq!(sender, receiver);
        assert!(verify_confirmation(
            &receiver,
            true,
            &confirmation(&sender, true)
        ));
        assert!(verify_confirmation(
            &sender,
            false,
            &confirmation(&receiver, false)
        ));
        // A proof cannot be reflected back as the other role's.
        assert!(!verify_confirmation(
            &sender,
            false,
            &confirmation(&sender, true)
        ));
    }

    #[test]
    fn a_wrong_code_or_another_connection_does_not_confirm() {
        let (sender, receiver) = keys("482914", "482913", b"hash");
        assert!(!verify_confirmation(
            &receiver,
            true,
            &confirmation(&sender, true)
        ));

        // Same code, but the key of one connection proves nothing on another.
        let (first, _) = keys("482913", "482913", b"one");
        let (_, second) = keys("482913", "482913", b"two");
        assert!(!verify_confirmation(
            &second,
            true,
            &confirmation(&first, true)
        ));
    }

    #[test]
    fn codes_are_six_digits() {
        for _ in 0..100 {
            assert!(is_code(&new_code()));
        }
        assert!(!is_code("12345"));
        assert!(!is_code("12a456"));
    }

    #[test]
    fn a_whole_vault_assembles_and_a_broken_one_does_not() {
        let bytes = vault_bytes();
        let mut assembler = Assembler::new(offer(&bytes)).expect("offer");
        for message in chunks(&bytes) {
            let TransferMessage::Chunk { index, bytes } = message else {
                unreachable!()
            };
            assembler.push(index, &bytes).expect("chunk");
        }
        assert_eq!(assembler.finish().expect("vault"), bytes);

        // A flipped byte fails the checksum.
        let mut broken = bytes.clone();
        let last = broken.len() - 1;
        broken[last] ^= 1;
        let mut assembler = Assembler::new(offer(&bytes)).expect("offer");
        assembler.push(0, &broken).expect("chunk");
        assert!(assembler.finish().is_err());

        // Out of order is refused.
        let mut assembler = Assembler::new(offer(&bytes)).expect("offer");
        assert!(assembler.push(1, &bytes).is_err());
    }

    #[test]
    fn something_else_is_not_a_vault() {
        let bytes = b"just some text, not a vault".to_vec();
        let mut assembler = Assembler::new(offer(&bytes)).expect("offer");
        assembler.push(0, &bytes).expect("chunk");
        assert!(matches!(
            assembler.finish(),
            Err(Error::PeerSync(PeerSyncFailure::NotAVault))
        ));
        let mut huge = offer(&bytes);
        huge.size = MAX_VAULT_BYTES + 1;
        assert!(Assembler::new(huge).is_err());
    }

    #[test]
    fn messages_round_trip_and_garbage_is_an_error() {
        let message = TransferMessage::Chunk {
            index: 3,
            bytes: vec![1, 2, 3],
        };
        assert_eq!(
            decode(&encode(&message).expect("encode")).expect("decode"),
            message
        );
        assert!(decode(b"\xff\x00garbage").is_err());
    }

    #[test]
    fn file_names_lose_their_directories() {
        assert_eq!(safe_file_name("../../etc/passwd"), "passwd");
        assert_eq!(safe_file_name("C:\\Users\\me\\Мой.wallet"), "Мой.wallet");
        assert_eq!(safe_file_name(".hidden"), "hidden");
        assert_eq!(safe_file_name("  "), "vault.wallet");
    }
}
