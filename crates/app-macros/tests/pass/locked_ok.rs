#![allow(dead_code)]
use mwt_app_macros::ipc;

include!("../stub/ipc_stub.rs");

#[ipc(locked_ok)]
fn status(_state: &AppState, app: &tauri::AppHandle, key: String) -> Result<String, ipc::IpcError> {
    Ok(key)
}

fn main() {
    assert_eq!(__IPC_META_status.args.len(), 1);
}
