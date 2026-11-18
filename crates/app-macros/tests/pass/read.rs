#![allow(dead_code)]
use mwt_app_macros::ipc;

include!("../stub/ipc_stub.rs");

#[ipc(read)]
fn list(_vault: &UnlockedVault, id: String) -> Result<String, ipc::IpcError> {
    Ok(id)
}

fn main() {
    assert_eq!(__IPC_META_list.name, "list");
}
