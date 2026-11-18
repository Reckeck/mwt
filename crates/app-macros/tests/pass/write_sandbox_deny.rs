#![allow(dead_code)]
use mwt_app_macros::ipc;

include!("../stub/ipc_stub.rs");

#[ipc(write, sandbox = deny)]
fn change(_vault: &mut UnlockedVault) -> Result<(), ipc::IpcError> {
    Ok(())
}

fn main() {
    assert_eq!(__IPC_META_change.name, "change");
}
