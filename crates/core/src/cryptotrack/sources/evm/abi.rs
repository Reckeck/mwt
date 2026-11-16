//! The little ABI this source needs: `balanceOf`, Multicall3 `aggregate3` and
//! `getEthBalance`. Hand-written and bounds-checked, because the node's answer
//! is untrusted and a decoder must not index past what it was given.

use super::uint::Uint256;
use crate::error::{Error, Result};

/// Multicall3 is deployed at this address on every network of the table
/// (<https://github.com/mds1/multicall>, deployments list).
pub const MULTICALL3: &str = "0xcA11bde05977b3631167028862bE2a173976CA11";

const SEL_BALANCE_OF: &str = "70a08231";
const SEL_GET_ETH_BALANCE: &str = "4d2301cc";
const SEL_AGGREGATE3: &str = "82ad56cb";
const WORD: usize = 32;
/// More than this is not an answer to a few hundred calls.
const MAX_RESULT_BYTES: usize = 1 << 20;

/// One Multicall3 `Call3` with `allowFailure = true`.
pub struct Call3 {
    /// Lowercase `0x` address.
    pub target: String,
    pub data: Vec<u8>,
}

/// `balanceOf(address)` calldata, `0x`-prefixed hex.
pub fn balance_of_calldata(owner: &str) -> Result<String> {
    Ok(format!("0x{SEL_BALANCE_OF}{}", hex(&address_word(owner)?)))
}

/// `getEthBalance(address)` calldata (a Multicall3 helper) as bytes.
pub fn get_eth_balance_data(owner: &str) -> Result<Vec<u8>> {
    unhex(&format!(
        "{SEL_GET_ETH_BALANCE}{}",
        hex(&address_word(owner)?)
    ))
}

pub fn balance_of_data(owner: &str) -> Result<Vec<u8>> {
    unhex(&balance_of_calldata(owner)?[2..])
}

/// `aggregate3((address,bool,bytes)[])` calldata, `0x`-prefixed hex.
pub fn aggregate3_calldata(calls: &[Call3]) -> Result<String> {
    let mut elements = Vec::with_capacity(calls.len());
    for call in calls {
        let mut item = address_word(&call.target)?.to_vec();
        item.extend_from_slice(&word_of(1)); // allowFailure
        item.extend_from_slice(&word_of(3 * WORD)); // offset of `callData` inside the tuple
        item.extend_from_slice(&word_of(call.data.len()));
        item.extend_from_slice(&call.data);
        item.resize(item.len() + (WORD - call.data.len() % WORD) % WORD, 0);
        elements.push(item);
    }
    let mut out = unhex(SEL_AGGREGATE3)?;
    out.extend_from_slice(&word_of(WORD)); // offset of the array
    out.extend_from_slice(&word_of(elements.len()));
    let mut offset = elements.len() * WORD;
    for item in &elements {
        out.extend_from_slice(&word_of(offset));
        offset += item.len();
    }
    for item in &elements {
        out.extend_from_slice(item);
    }
    Ok(format!("0x{}", hex(&out)))
}

/// What one call of an `aggregate3` answered: `Some(returnData)` on success.
pub fn decode_aggregate3(result: &str, expected: usize) -> Result<Vec<Option<Vec<u8>>>> {
    let bytes = unhex(
        result
            .strip_prefix("0x")
            .ok_or_else(|| bad("no 0x prefix"))?,
    )?;
    if bytes.len() > MAX_RESULT_BYTES {
        return Err(bad("answer too large"));
    }
    let array_at = word_usize(&bytes, 0)?;
    let count = word_usize(&bytes, array_at)?;
    if count != expected {
        return Err(bad("wrong number of results"));
    }
    let heads = array_at.checked_add(WORD).ok_or_else(|| bad("offset"))?;
    let mut out = Vec::with_capacity(count);
    for index in 0..count {
        let offset = word_usize(&bytes, heads + index * WORD)?;
        let tuple = heads.checked_add(offset).ok_or_else(|| bad("offset"))?;
        let success = word_usize(&bytes, tuple)?;
        let data_at = tuple
            .checked_add(word_usize(&bytes, tuple + WORD)?)
            .ok_or_else(|| bad("offset"))?;
        let len = word_usize(&bytes, data_at)?;
        let start = data_at + WORD;
        let end = start.checked_add(len).ok_or_else(|| bad("length"))?;
        let data = bytes
            .get(start..end)
            .ok_or_else(|| bad("data out of range"))?;
        out.push((success == 1).then(|| data.to_vec()));
    }
    Ok(out)
}

/// One 32-byte answer (`balanceOf`, `getEthBalance`) as an integer.
pub fn decode_uint_word(data: &[u8]) -> Result<Uint256> {
    if data.len() != WORD {
        return Err(bad("expected one 32-byte word"));
    }
    Uint256::from_hex(&format!("0x{}", hex(data)))
}

pub(crate) fn address_word(address: &str) -> Result<[u8; WORD]> {
    let raw = address
        .strip_prefix("0x")
        .filter(|text| text.len() == 40)
        .ok_or_else(|| Error::InvalidInput("not an EVM address".into()))?;
    let bytes = unhex(raw)?;
    let mut word = [0u8; WORD];
    word[12..].copy_from_slice(&bytes);
    Ok(word)
}

/// Calldata of a function whose arguments are all one word wide (an address,
/// a `uint256`, a `uint24`, an `int24`): the selector and the words, nothing
/// else. Dynamic arguments are not needed by any reader of this module and
/// are deliberately not supported — a decoder that cannot be fed a length
/// cannot be fooled by one.
pub(crate) fn calldata(selector: &str, words: &[[u8; WORD]]) -> Result<String> {
    if selector.len() != 8 || !selector.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(bad("a selector is four bytes of hex"));
    }
    let mut out = unhex(selector)?;
    for word in words {
        out.extend_from_slice(word);
    }
    Ok(format!("0x{}", hex(&out)))
}

/// A signed, one-word argument in two's complement (`int24`, `int256`).
///
/// Nothing calls it until spec D1 asks a pool for `ticks(int24)`; it is here
/// so the four adapters of wave 1 share one encoder of a negative tick
/// instead of writing four. Pinned by the tests of
/// [`crate::cryptotrack::protocols::node`].
#[allow(dead_code, reason = "spec D1 passes a tick as an argument")]
pub(crate) fn int_word(value: i64) -> [u8; WORD] {
    let mut word = if value < 0 {
        [0xffu8; WORD]
    } else {
        [0u8; WORD]
    };
    word[WORD - 8..].copy_from_slice(&value.to_be_bytes());
    word
}

/// A node's answer split into whole words; an answer that is not a whole
/// number of words is refused rather than padded.
pub(crate) fn split_words(data: &[u8]) -> Result<Vec<[u8; WORD]>> {
    if !data.len().is_multiple_of(WORD) {
        return Err(bad("an answer is whole words"));
    }
    Ok(data
        .chunks(WORD)
        .map(|chunk| {
            let mut word = [0u8; WORD];
            word.copy_from_slice(chunk);
            word
        })
        .collect())
}

/// The address a word holds: the low 20 bytes, lowercase and `0x`-prefixed.
/// The high 12 bytes must be zero — a word that is not an address is refused.
pub(crate) fn word_address(word: &[u8; WORD]) -> Result<String> {
    if word[..12].iter().any(|byte| *byte != 0) {
        return Err(bad("not an address word"));
    }
    Ok(format!("0x{}", hex(&word[12..])))
}

/// A signed integer a word holds, as far as `i64` reaches (`int24`, a tick).
pub(crate) fn word_int(word: &[u8; WORD]) -> Result<i64> {
    let negative = word[0] & 0x80 != 0;
    let filler = if negative { 0xffu8 } else { 0x00 };
    if word[..WORD - 8].iter().any(|byte| *byte != filler) {
        return Err(bad("signed value wider than 64 bits"));
    }
    let mut tail = [0u8; 8];
    tail.copy_from_slice(&word[WORD - 8..]);
    Ok(i64::from_be_bytes(tail))
}

fn word_of(value: usize) -> [u8; WORD] {
    let mut word = [0u8; WORD];
    word[WORD - 8..].copy_from_slice(&(value as u64).to_be_bytes());
    word
}

/// A word that must be a small number (an offset, a length, a bool).
fn word_usize(bytes: &[u8], at: usize) -> Result<usize> {
    let end = at.checked_add(WORD).ok_or_else(|| bad("offset"))?;
    let word = bytes.get(at..end).ok_or_else(|| bad("word out of range"))?;
    if word[..WORD - 8].iter().any(|byte| *byte != 0) {
        return Err(bad("number too large"));
    }
    let mut low = [0u8; 8];
    low.copy_from_slice(&word[WORD - 8..]);
    // Offsets, lengths and counts are all below the size cap, so sums of a
    // few of them cannot overflow.
    usize::try_from(u64::from_be_bytes(low))
        .ok()
        .filter(|value| *value <= MAX_RESULT_BYTES)
        .ok_or_else(|| bad("number too large"))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unhex(text: &str) -> Result<Vec<u8>> {
    if !text.len().is_multiple_of(2) || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(bad("not hex"));
    }
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).map_err(|_| bad("not hex")))
        .collect()
}

fn bad(why: &str) -> Error {
    Error::Network(format!("bad ABI data: {why}"))
}
