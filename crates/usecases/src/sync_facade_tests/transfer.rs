//! The transfer test that needs a real vault file (`Session::create`). It moved here
//! from `mwt-sync` with its assertions unchanged (crate-split D6).

use crate::service::Session;
use crate::sync::transfer::{Assembler, Offer, TRANSFER_PROTOCOL, TransferMessage, chunks, sha256};

fn vault_bytes() -> Vec<u8> {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.wallet");
    Session::create(path.clone(), "pw").expect("vault");
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
