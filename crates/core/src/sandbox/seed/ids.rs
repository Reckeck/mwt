//! Fixed ids: the seed never asks the system for a random one.

use uuid::Uuid;

const VAULT: u128 = 0x5a4d_0000_0000_4000_8000_0000_0000_0000;
const STORE: u128 = 0x5a4d_0001_0000_4000_8000_0000_0000_0000;
const TRANSACTION: u128 = 0x5a4d_0002_0000_4000_8000_0000_0000_0000;

const HOLDING: u128 = 0x5a4d_0003_0000_4000_8000_0000_0000_0000;

pub fn holding(number: u128) -> String {
    Uuid::from_u128(HOLDING + number).to_string()
}

pub fn vault() -> String {
    Uuid::from_u128(VAULT).to_string()
}

pub fn store(number: u128) -> String {
    Uuid::from_u128(STORE + number).to_string()
}

pub fn transaction(number: u128) -> String {
    Uuid::from_u128(TRANSACTION + number).to_string()
}
