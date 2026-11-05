//! One vault transfer over one TCP connection, from the Noise handshake to
//! the last byte (docs/sync.md, "Vault transfer"). The rules and the
//! crypto are in `trade_core::sync::transfer`; the app side (discovery, the
//! screens, saving the file) is in `handover`.
//!
//! Both sessions take the user's answers through channels and report what
//! happens through a callback, so the tests drive them over a real socket
//! without an app.

use std::time::Duration;

use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::mpsc;
use trade_core::error::{Error, PeerSyncFailure, Result};
use trade_core::sync::session::{Channel, DeviceKey, Handshake};
use trade_core::sync::transfer::{
    Assembler, CODE_ATTEMPTS, CodeAttempt, Offer, TransferMessage, chunks, confirmation, decode,
    encode, new_code, verify_confirmation,
};

use super::link::{read_frame, write_frame};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
/// A step the other device takes by itself (no person in between).
const STEP_TIMEOUT: Duration = Duration::from_secs(60);
/// A step a person takes (accept the request, type the code): bounded like the
/// receive window, so a stalled sender cannot hold the receiver for ever.
const PERSON_TIMEOUT: Duration = Duration::from_secs(300);

fn failure(kind: PeerSyncFailure) -> Error {
    Error::PeerSync(kind)
}

/// The encrypted connection, one message at a time.
struct Pipe {
    channel: Channel,
    reader: OwnedReadHalf,
    writer: OwnedWriteHalf,
}

impl Pipe {
    async fn open(stream: TcpStream, key: &DeviceKey, initiator: bool) -> Result<Self> {
        stream.set_nodelay(true).ok();
        let (mut reader, mut writer) = stream.into_split();
        let channel = tokio::time::timeout(HANDSHAKE_TIMEOUT, async {
            let mut handshake = if initiator {
                Handshake::initiator(key)?
            } else {
                Handshake::responder(key)?
            };
            while !handshake.is_finished() {
                if handshake.my_turn() {
                    write_frame(&mut writer, &handshake.write()?).await?;
                } else {
                    handshake.read(&read_frame(&mut reader).await?)?;
                }
            }
            handshake.finish()
        })
        .await
        .map_err(|_| failure(PeerSyncFailure::Channel))??;
        Ok(Self {
            channel,
            reader,
            writer,
        })
    }

    async fn send(&mut self, message: &TransferMessage) -> Result<()> {
        for frame in self.channel.seal(&encode(message)?)? {
            write_frame(&mut self.writer, &frame).await?;
        }
        Ok(())
    }

    async fn next(&mut self) -> Result<TransferMessage> {
        loop {
            let frame = read_frame(&mut self.reader).await?;
            if let Some(bytes) = self.channel.open(&frame)? {
                return decode(&bytes);
            }
        }
    }

    async fn next_within(&mut self, limit: Duration) -> Result<TransferMessage> {
        tokio::time::timeout(limit, self.next())
            .await
            .map_err(|_| failure(PeerSyncFailure::Channel))?
    }
}

/// Who this device is, as the other one will see it.
#[derive(Debug, Clone)]
pub(crate) struct Me {
    pub device_id: String,
    pub device_name: String,
}

// -- receiving -----------------------------------------------------------------

/// What the receiving session reports as it goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Incoming {
    /// A device asks to hand its vault over: accept or decline.
    Request {
        sender_name: String,
    },
    /// Accepted: show this code.
    Code(String),
    /// The code typed there was wrong; a new try may come.
    WrongCode {
        attempts_left: u32,
    },
    Progress {
        done: u64,
        total: u64,
    },
}

/// The vault that arrived, checked as a whole.
#[derive(Debug)]
pub(crate) struct Received {
    pub offer: Offer,
    pub bytes: Vec<u8>,
    /// The sending device's static key, hex (to trust it for sync).
    pub sender_key: String,
}

/// Serve one request. `decisions` carries "Accept" (`true`) or "Decline".
pub(crate) async fn receive(
    stream: TcpStream,
    key: &DeviceKey,
    me: &Me,
    decisions: &mut mpsc::UnboundedReceiver<bool>,
    report: impl Fn(Incoming),
) -> Result<Received> {
    let mut pipe = Pipe::open(stream, key, false).await?;
    let offer = match pipe.next_within(STEP_TIMEOUT).await? {
        TransferMessage::Offer(offer) => offer,
        _ => return Err(failure(PeerSyncFailure::Channel)),
    };
    let mut assembler = Assembler::new(offer.clone())?;
    report(Incoming::Request {
        sender_name: offer.device_name.clone(),
    });
    let decision = tokio::time::timeout(PERSON_TIMEOUT, decisions.recv())
        .await
        .ok()
        .flatten();
    if decision != Some(true) {
        let _ = pipe.send(&TransferMessage::Declined).await;
        return Err(failure(PeerSyncFailure::TransferDeclined));
    }
    let code = new_code();
    pipe.send(&TransferMessage::Accepted {
        device_id: me.device_id.clone(),
        device_name: me.device_name.clone(),
    })
    .await?;
    report(Incoming::Code(code.clone()));

    let mut attempts_left = CODE_ATTEMPTS;
    loop {
        let attempt = CodeAttempt::start(&code);
        pipe.send(&TransferMessage::Pake {
            message: attempt.message().to_vec(),
        })
        .await?;
        // A person types the code there: a longer limit than a step.
        let TransferMessage::Pake { message } = pipe.next_within(PERSON_TIMEOUT).await? else {
            return Err(failure(PeerSyncFailure::Channel));
        };
        let shared = attempt.finish(&message, pipe.channel.handshake_hash())?;
        let TransferMessage::Confirm { mac } = pipe.next_within(STEP_TIMEOUT).await? else {
            return Err(failure(PeerSyncFailure::Channel));
        };
        if verify_confirmation(&shared, true, &mac) {
            pipe.send(&TransferMessage::Confirm {
                mac: confirmation(&shared, false),
            })
            .await?;
            break;
        }
        attempts_left -= 1;
        if attempts_left == 0 {
            let _ = pipe.send(&TransferMessage::Declined).await;
            return Err(failure(PeerSyncFailure::BadCode));
        }
        pipe.send(&TransferMessage::CodeWrong { attempts_left })
            .await?;
        report(Incoming::WrongCode { attempts_left });
    }

    loop {
        match pipe.next_within(STEP_TIMEOUT).await? {
            TransferMessage::Chunk { index, bytes } => {
                assembler.push(index, &bytes)?;
                let (done, total) = assembler.progress();
                report(Incoming::Progress { done, total });
            }
            TransferMessage::Done => break,
            _ => return Err(failure(PeerSyncFailure::Channel)),
        }
    }
    let bytes = assembler.finish()?;
    pipe.send(&TransferMessage::Received).await?;
    Ok(Received {
        offer,
        bytes,
        sender_key: pipe.channel.remote_hex(),
    })
}

// -- sending -------------------------------------------------------------------

/// What the sending session reports as it goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Outgoing {
    /// The other device accepted: ask for its code.
    Accepted,
    WrongCode {
        attempts_left: u32,
    },
    Progress {
        done: u64,
        total: u64,
    },
}

/// The device the vault went to.
#[derive(Debug, Clone)]
pub(crate) struct Delivered {
    pub device_id: String,
    pub device_name: String,
    pub device_key: String,
}

/// Hand `vault` (the sealed file) to the device behind `stream`. `fingerprint`
/// is what it announced (its key's); `codes` carries what the user types.
pub(crate) async fn send(
    stream: TcpStream,
    key: &DeviceKey,
    offer: Offer,
    vault: &[u8],
    fingerprint: &str,
    codes: &mut mpsc::UnboundedReceiver<String>,
    report: impl Fn(Outgoing),
) -> Result<Delivered> {
    let mut pipe = Pipe::open(stream, key, true).await?;
    // The key behind the address must be the one the device announced.
    if !fingerprint.is_empty() && fingerprint_of(&pipe.channel.remote_hex()) != fingerprint {
        return Err(failure(PeerSyncFailure::Channel));
    }
    pipe.send(&TransferMessage::Offer(offer)).await?;
    // A person decides there, but not for ever: the receive window closes it,
    // so a peer that goes silent ends the send too.
    let (device_id, device_name) = match pipe.next_within(PERSON_TIMEOUT + STEP_TIMEOUT).await? {
        TransferMessage::Accepted {
            device_id,
            device_name,
        } => (device_id, device_name),
        TransferMessage::Declined => return Err(failure(PeerSyncFailure::TransferDeclined)),
        _ => return Err(failure(PeerSyncFailure::Channel)),
    };
    report(Outgoing::Accepted);

    loop {
        let TransferMessage::Pake { message: theirs } = pipe.next_within(STEP_TIMEOUT).await?
        else {
            return Err(failure(PeerSyncFailure::Channel));
        };
        let Some(code) = codes.recv().await else {
            return Err(failure(PeerSyncFailure::TransferDeclined));
        };
        let attempt = CodeAttempt::start(&code);
        pipe.send(&TransferMessage::Pake {
            message: attempt.message().to_vec(),
        })
        .await?;
        let shared = attempt.finish(&theirs, pipe.channel.handshake_hash())?;
        pipe.send(&TransferMessage::Confirm {
            mac: confirmation(&shared, true),
        })
        .await?;
        match pipe.next_within(STEP_TIMEOUT).await? {
            TransferMessage::Confirm { mac } if verify_confirmation(&shared, false, &mac) => break,
            TransferMessage::CodeWrong { attempts_left } => {
                report(Outgoing::WrongCode { attempts_left });
            }
            TransferMessage::Declined => return Err(failure(PeerSyncFailure::BadCode)),
            _ => return Err(failure(PeerSyncFailure::Channel)),
        }
    }

    let total = vault.len() as u64;
    let mut done = 0u64;
    for message in chunks(vault) {
        if let TransferMessage::Chunk { bytes, .. } = &message {
            done += bytes.len() as u64;
        }
        pipe.send(&message).await?;
        report(Outgoing::Progress { done, total });
    }
    pipe.send(&TransferMessage::Done).await?;
    match pipe.next_within(STEP_TIMEOUT).await? {
        TransferMessage::Received => Ok(Delivered {
            device_id,
            device_name,
            device_key: pipe.channel.remote_hex(),
        }),
        _ => Err(failure(PeerSyncFailure::Channel)),
    }
}

/// What a receiving device announces of its key: the first 8 bytes of its
/// SHA-256, hex.
pub(crate) fn fingerprint_of(public_hex: &str) -> String {
    let digest = trade_core::sync::transfer::sha256(public_hex.as_bytes());
    trade_core::sync::session::hex(&digest[..8])
}
