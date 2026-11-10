//! The encrypted channel between two devices, without any I/O: the app
//! crate moves the bytes, this decides what they mean.
//!
//! - Noise `XX_25519_ChaChaPoly_BLAKE2s`: both sides prove their static key;
//!   a trusted device is recognised by it.
//! - The pairing code is six digits from the handshake hash and a nonce of
//!   each side: a device in the middle would see two different codes, and the
//!   dialer commits to its nonce before it sees the listener's, so the middle
//!   cannot grind its keys until both codes match (protocol 4).
//! - The vault proof is an HMAC of the handshake hash keyed by the vault id,
//!   which only someone who decrypted the vault knows; it is bound to the
//!   role, so a proof cannot be reflected back.
//! - The discovery tag is an HMAC of the day keyed by the vault id: devices
//!   with the same vault recognise each other, anyone else sees noise that
//!   changes daily.

use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::error::{Error, Result};

type HmacSha256 = Hmac<Sha256>;

pub const NOISE_PARAMS: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";
/// Largest Noise message, ciphertext included.
pub const NOISE_MAX: usize = 65_535;
const TAG_LEN: usize = 16;
/// Plaintext per Noise message: the first one also carries a 4-byte length.
const CHUNK: usize = NOISE_MAX - TAG_LEN;

/// Any Noise failure (tampering, a wrong key, a broken frame) ends the
/// session the same way; the detail would only help an attacker.
fn noise_err(_err: snow::Error) -> Error {
    Error::PeerSync(crate::error::PeerSyncFailure::Channel)
}

/// Bytes of the nonce each side adds to the pairing code.
pub const NONCE_LEN: usize = 32;

/// A fresh random nonce for [`Channel::pairing_code`].
pub fn new_nonce() -> Vec<u8> {
    rand::random::<[u8; NONCE_LEN]>().to_vec()
}

/// The commitment the dialer sends in its first `Hello`.
pub fn commit_to(nonce: &[u8]) -> Vec<u8> {
    use sha2::Digest;
    Sha256::digest(nonce).to_vec()
}

/// The dialer's nonce matches what it committed to; anything else ends the
/// session like any other broken channel.
pub fn check_commit(commit: &[u8], nonce: &[u8]) -> Result<()> {
    if nonce.len() == NONCE_LEN && commit_to(nonce) == commit {
        Ok(())
    } else {
        Err(Error::PeerSync(crate::error::PeerSyncFailure::Channel))
    }
}

/// A static key pair for this device, hex.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceKey {
    pub private: Vec<u8>,
    pub public: Vec<u8>,
}

impl DeviceKey {
    pub fn generate() -> Result<Self> {
        let params = NOISE_PARAMS.parse().map_err(noise_err)?;
        let pair = snow::Builder::new(params)
            .generate_keypair()
            .map_err(noise_err)?;
        Ok(Self {
            private: pair.private,
            public: pair.public,
        })
    }

    /// The public half of a stored private key.
    pub fn from_private(private: &[u8]) -> Result<Self> {
        if private.len() != 32 {
            return Err(Error::InvalidInput("device key must be 32 bytes".into()));
        }
        let mut secret = [0u8; 32];
        secret.copy_from_slice(private);
        let public = x25519_public(&secret);
        Ok(Self {
            private: private.to_vec(),
            public: public.to_vec(),
        })
    }

    pub fn public_hex(&self) -> String {
        hex(&self.public)
    }
}

/// X25519 public key of a private one, by a one-message Noise `N` handshake
/// the responder side of which reveals nothing: computed through `snow`'s
/// own resolver so no second curve implementation is pulled in.
fn x25519_public(private: &[u8; 32]) -> [u8; 32] {
    use snow::params::DHChoice;
    use snow::resolvers::{CryptoResolver, DefaultResolver};
    let mut public = [0u8; 32];
    if let Some(mut dh) = DefaultResolver.resolve_dh(&DHChoice::Curve25519) {
        dh.set(private);
        public.copy_from_slice(&dh.pubkey()[..32]);
    }
    public
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(text.get(at..at + 2)?, 16).ok())
        .collect()
}

/// The handshake in progress.
pub struct Handshake {
    state: snow::HandshakeState,
    initiator: bool,
}

impl Handshake {
    pub fn initiator(key: &DeviceKey) -> Result<Self> {
        Self::build(key, true)
    }

    pub fn responder(key: &DeviceKey) -> Result<Self> {
        Self::build(key, false)
    }

    fn build(key: &DeviceKey, initiator: bool) -> Result<Self> {
        let params = NOISE_PARAMS.parse().map_err(noise_err)?;
        let builder = snow::Builder::new(params)
            .local_private_key(&key.private)
            .map_err(noise_err)?;
        let state = if initiator {
            builder.build_initiator()
        } else {
            builder.build_responder()
        }
        .map_err(noise_err)?;
        Ok(Self { state, initiator })
    }

    pub fn is_finished(&self) -> bool {
        self.state.is_handshake_finished()
    }

    /// Whether it is this side's turn to write.
    pub fn my_turn(&self) -> bool {
        self.state.is_my_turn()
    }

    pub fn write(&mut self) -> Result<Vec<u8>> {
        let mut out = vec![0u8; NOISE_MAX];
        let len = self.state.write_message(&[], &mut out).map_err(noise_err)?;
        out.truncate(len);
        Ok(out)
    }

    pub fn read(&mut self, message: &[u8]) -> Result<()> {
        let mut payload = vec![0u8; NOISE_MAX];
        self.state
            .read_message(message, &mut payload)
            .map_err(noise_err)?;
        Ok(())
    }

    pub fn finish(self) -> Result<Channel> {
        if !self.state.is_handshake_finished() {
            return Err(Error::InvalidInput("handshake not finished".into()));
        }
        let hash = self.state.get_handshake_hash().to_vec();
        let remote = self
            .state
            .get_remote_static()
            .map(<[u8]>::to_vec)
            .ok_or_else(|| Error::InvalidInput("peer sent no static key".into()))?;
        let transport = self.state.into_transport_mode().map_err(noise_err)?;
        Ok(Channel {
            transport,
            remote_static: remote,
            hash,
            initiator: self.initiator,
            pending: Vec::new(),
            expected: None,
        })
    }
}

/// The channel after the handshake.
pub struct Channel {
    transport: snow::TransportState,
    pub remote_static: Vec<u8>,
    hash: Vec<u8>,
    pub initiator: bool,
    pending: Vec<u8>,
    expected: Option<usize>,
}

fn mac(key: &[u8], parts: &[&[u8]]) -> HmacSha256 {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(key).unwrap_or_else(|_| {
        // HMAC takes keys of any length; this branch cannot be reached.
        <HmacSha256 as Mac>::new_from_slice(&[0]).expect("hmac key")
    });
    for part in parts {
        mac.update(part);
    }
    mac
}

impl Channel {
    /// The handshake hash: unique to this connection, so anything bound to
    /// it cannot be replayed on another one.
    pub fn handshake_hash(&self) -> &[u8] {
        &self.hash
    }

    pub fn remote_hex(&self) -> String {
        hex(&self.remote_static)
    }

    /// The six-digit code both screens show while pairing: bound to this
    /// connection and to the nonces of the dialer and the listener, in that order.
    pub fn pairing_code(&self, dialer_nonce: &[u8], listener_nonce: &[u8]) -> String {
        let digest = mac(&self.hash, &[b"mwt-sas4", dialer_nonce, listener_nonce])
            .finalize()
            .into_bytes();
        let number = u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]) % 1_000_000;
        format!("{number:06}")
    }

    fn proof_for(&self, vault_id: &str, initiator: bool) -> HmacSha256 {
        mac(
            vault_id.as_bytes(),
            &[b"mwt-proof", &self.hash, &[u8::from(initiator)]],
        )
    }

    /// Proof that this side holds the vault open.
    pub fn proof(&self, vault_id: &str) -> Vec<u8> {
        self.proof_for(vault_id, self.initiator)
            .finalize()
            .into_bytes()
            .to_vec()
    }

    /// Check the other side's proof (constant time).
    pub fn verify_proof(&self, vault_id: &str, proof: &[u8]) -> bool {
        self.proof_for(vault_id, !self.initiator)
            .verify_slice(proof)
            .is_ok()
    }

    /// Encrypt a message into Noise messages to send in order.
    pub fn seal(&mut self, plaintext: &[u8]) -> Result<Vec<Vec<u8>>> {
        if plaintext.len() > super::protocol::MAX_MESSAGE {
            return Err(Error::InvalidInput("sync message too large".into()));
        }
        let len = u32::try_from(plaintext.len())
            .map_err(|_| Error::InvalidInput("sync message too large".into()))?;
        let mut framed = Vec::with_capacity(plaintext.len() + 4);
        framed.extend_from_slice(&len.to_be_bytes());
        framed.extend_from_slice(plaintext);
        let mut out = Vec::new();
        for chunk in framed.chunks(CHUNK) {
            let mut buf = vec![0u8; chunk.len() + TAG_LEN];
            let written = self
                .transport
                .write_message(chunk, &mut buf)
                .map_err(noise_err)?;
            buf.truncate(written);
            out.push(buf);
        }
        Ok(out)
    }

    /// Take one Noise message; a whole application message once complete.
    pub fn open(&mut self, message: &[u8]) -> Result<Option<Vec<u8>>> {
        let mut buf = vec![0u8; message.len()];
        let read = self
            .transport
            .read_message(message, &mut buf)
            .map_err(noise_err)?;
        buf.truncate(read);
        self.pending.extend_from_slice(&buf);
        if self.expected.is_none() && self.pending.len() >= 4 {
            let len = u32::from_be_bytes([
                self.pending[0],
                self.pending[1],
                self.pending[2],
                self.pending[3],
            ]) as usize;
            if len > super::protocol::MAX_MESSAGE {
                return Err(Error::InvalidInput("sync message too large".into()));
            }
            self.pending.drain(..4);
            self.expected = Some(len);
        }
        match self.expected {
            Some(len) if self.pending.len() >= len => {
                let rest = self.pending.split_off(len);
                let message = std::mem::replace(&mut self.pending, rest);
                self.expected = None;
                Ok(Some(message))
            }
            Some(len) if self.pending.len() > len => Err(Error::InvalidInput("bad frame".into())),
            _ => Ok(None),
        }
    }
}

/// The tag a device advertises for a vault on a given day (Unix days).
pub fn discovery_tag(vault_id: &str, day: u64) -> String {
    let digest = mac(vault_id.as_bytes(), &[b"mwt-disc", &day.to_be_bytes()])
        .finalize()
        .into_bytes();
    hex(&digest[..8])
}

/// Unix day of a Unix millisecond time.
pub fn day_of(ms: u64) -> u64 {
    ms / (24 * 3600 * 1000)
}

// -- clocks -------------------------------------------------------------------

/// Up to this difference the clocks are taken as equal.
pub const SKEW_FINE_MS: u64 = 5 * 60 * 1000;
/// Beyond this the devices refuse to sync.
pub const SKEW_REFUSE_MS: u64 = super::stamp::MAX_FUTURE_MS;
/// A drift of the measured offset beyond this within a session is a jump.
pub const DRIFT_MS: u64 = 2 * 60 * 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skew {
    Fine,
    /// Sync, but tell the user the clocks differ by this much (ms).
    Warn(u64),
    Refuse(u64),
}

pub fn skew_of(offset_ms: i64) -> Skew {
    let size = offset_ms.unsigned_abs();
    if size <= SKEW_FINE_MS {
        Skew::Fine
    } else if size <= SKEW_REFUSE_MS {
        Skew::Warn(size)
    } else {
        Skew::Refuse(size)
    }
}

/// Watches the other device's clock during a session.
#[derive(Debug, Clone)]
pub struct ClockWatch {
    /// Remote minus local at the handshake.
    offset: i64,
    last_remote: u64,
}

/// Why the other device's time is not trusted any more.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Jump {
    Drift,
    Backwards,
    Future,
}

impl ClockWatch {
    pub fn new(remote_now: u64, local_now: u64) -> Self {
        Self {
            offset: remote_now as i64 - local_now as i64,
            last_remote: remote_now,
        }
    }

    pub fn offset(&self) -> i64 {
        self.offset
    }

    /// A time report from the other device.
    pub fn observe(&mut self, remote_now: u64, local_now: u64) -> Option<Jump> {
        if remote_now < self.last_remote {
            return Some(Jump::Backwards);
        }
        self.last_remote = remote_now;
        let offset = remote_now as i64 - local_now as i64;
        ((offset - self.offset).unsigned_abs() > DRIFT_MS).then_some(Jump::Drift)
    }

    /// A stamp from the other device further ahead than its clock can be.
    pub fn check_stamp(&self, wall: u64, local_now: u64) -> Option<Jump> {
        let limit = local_now as i64 + self.offset + SKEW_FINE_MS as i64;
        (wall as i64 > limit).then_some(Jump::Future)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handshake(left: &DeviceKey, right: &DeviceKey) -> (Channel, Channel) {
        let mut init = Handshake::initiator(left).unwrap();
        let mut resp = Handshake::responder(right).unwrap();
        while !(init.is_finished() && resp.is_finished()) {
            if init.my_turn() {
                let msg = init.write().unwrap();
                resp.read(&msg).unwrap();
            } else {
                let msg = resp.write().unwrap();
                init.read(&msg).unwrap();
            }
        }
        (init.finish().unwrap(), resp.finish().unwrap())
    }

    #[test]
    fn both_sides_see_the_same_code_and_each_others_key() {
        let left = DeviceKey::generate().unwrap();
        let right = DeviceKey::generate().unwrap();
        let (one, two) = handshake(&left, &right);
        let (dialer, listener) = (new_nonce(), new_nonce());
        assert_eq!(
            one.pairing_code(&dialer, &listener),
            two.pairing_code(&dialer, &listener)
        );
        assert_eq!(one.remote_static, right.public);
        assert_eq!(two.remote_static, left.public);
    }

    /// Protocol 4: the code binds the dialer's committed nonce and the
    /// listener's nonce, so a device in the middle cannot grind its key.
    #[test]
    fn pairing_code_depends_on_both_nonces() {
        let (one, two) = handshake(
            &DeviceKey::generate().unwrap(),
            &DeviceKey::generate().unwrap(),
        );
        let dialer = [1u8; NONCE_LEN];
        let listener = [2u8; NONCE_LEN];
        let code = one.pairing_code(&dialer, &listener);
        assert_eq!(code, two.pairing_code(&dialer, &listener));
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|digit| digit.is_ascii_digit()));
        // Several nonces on either side: one fixed pair could collide (1 in 10^6).
        let by_dialer: std::collections::BTreeSet<String> = (0..8u8)
            .map(|seed| one.pairing_code(&[seed; NONCE_LEN], &listener))
            .collect();
        let by_listener: std::collections::BTreeSet<String> = (0..8u8)
            .map(|seed| one.pairing_code(&dialer, &[seed; NONCE_LEN]))
            .collect();
        assert!(by_dialer.len() > 1, "the dialer's nonce changes the code");
        assert!(
            by_listener.len() > 1,
            "the listener's nonce changes the code"
        );
        // The order matters: swapping the roles gives another code.
        let swapped: std::collections::BTreeSet<String> = (0..8u8)
            .map(|seed| {
                let mine = [seed; NONCE_LEN];
                let other = [seed.wrapping_add(100); NONCE_LEN];
                one.pairing_code(&mine, &other) + &one.pairing_code(&other, &mine)
            })
            .filter(|both| both[..6] != both[6..])
            .collect();
        assert!(!swapped.is_empty());
    }

    #[test]
    fn commit_mismatch_is_rejected() {
        let nonce = new_nonce();
        assert_eq!(nonce.len(), NONCE_LEN);
        assert_ne!(nonce, new_nonce());
        let commit = commit_to(&nonce);
        assert_eq!(commit.len(), 32);
        assert!(check_commit(&commit, &nonce).is_ok());
        let channel = |result: Result<()>| {
            matches!(
                result,
                Err(Error::PeerSync(crate::error::PeerSyncFailure::Channel))
            )
        };
        assert!(channel(check_commit(&commit, &new_nonce())));
        assert!(channel(check_commit(&[], &nonce)));
        assert!(channel(check_commit(&commit_to(&[]), &[])));
        assert!(channel(check_commit(
            &commit_to(&nonce[..16]),
            &nonce[..16]
        )));
    }

    #[test]
    fn the_public_key_follows_from_the_private_one() {
        let key = DeviceKey::generate().unwrap();
        assert_eq!(DeviceKey::from_private(&key.private).unwrap(), key);
        assert_eq!(unhex(&key.public_hex()).unwrap(), key.public);
    }

    #[test]
    fn the_vault_proof_needs_the_vault_and_the_role() {
        let (one, two) = handshake(
            &DeviceKey::generate().unwrap(),
            &DeviceKey::generate().unwrap(),
        );
        let proof = one.proof("vault-1");
        assert!(two.verify_proof("vault-1", &proof));
        assert!(!two.verify_proof("vault-2", &proof));
        // A proof sent back to its author does not pass.
        assert!(!one.verify_proof("vault-1", &proof));
    }

    #[test]
    fn large_messages_cross_in_pieces_and_tampering_fails() {
        let (mut one, mut two) = handshake(
            &DeviceKey::generate().unwrap(),
            &DeviceKey::generate().unwrap(),
        );
        let big: Vec<u8> = (0..200_000u32).map(|index| (index % 251) as u8).collect();
        let frames = one.seal(&big).unwrap();
        assert!(frames.len() > 1);
        let mut got = None;
        for frame in &frames {
            got = two.open(frame).unwrap().or(got);
        }
        assert_eq!(got.unwrap(), big);

        let mut frames = one.seal(b"hello").unwrap();
        frames[0][3] ^= 1;
        assert!(two.open(&frames[0]).is_err());
    }

    #[test]
    fn discovery_tags_differ_by_vault_and_day() {
        assert_eq!(discovery_tag("v", 1), discovery_tag("v", 1));
        assert_ne!(discovery_tag("v", 1), discovery_tag("v", 2));
        assert_ne!(discovery_tag("v", 1), discovery_tag("w", 1));
        assert_eq!(discovery_tag("v", 1).len(), 16);
    }

    #[test]
    fn clock_skew_steps() {
        assert_eq!(skew_of(60_000), Skew::Fine);
        assert_eq!(skew_of(-600_000), Skew::Warn(600_000));
        assert!(matches!(
            skew_of(SKEW_REFUSE_MS as i64 + 1),
            Skew::Refuse(_)
        ));
    }

    #[test]
    fn clock_jumps_are_noticed() {
        let mut watch = ClockWatch::new(1_000_000, 1_000_000);
        assert_eq!(watch.observe(1_010_000, 1_010_000), None);
        assert_eq!(watch.observe(1_000_000, 1_020_000), Some(Jump::Backwards));
        assert_eq!(watch.observe(1_500_000, 1_030_000), Some(Jump::Drift));
        assert_eq!(
            watch.check_stamp(1_030_000 + SKEW_FINE_MS + 10, 1_030_000),
            Some(Jump::Future)
        );
        assert_eq!(watch.check_stamp(1_030_000, 1_030_000), None);
    }
}
