
#![allow(dead_code)]
use mwt_app_macros::ipc;

mod tauri {
    pub use mwt_app_macros::passthrough as command;
    #[derive(Clone)]
    pub struct AppHandle;
}
mod ts_rs {
    pub struct Config;
    impl Config {
        pub fn new() -> Self {
            Config
        }
    }
    pub trait TS {
        fn name(config: &Config) -> String;
    }
    impl TS for String {
        fn name(_: &Config) -> String {
            "string".into()
        }
    }
    impl TS for () {
        fn name(_: &Config) -> String {
            "null".into()
        }
    }
}
mod ipc {
    pub struct IpcError;
    pub mod run {
        pub trait VaultHost {}
        impl VaultHost for crate::tauri::AppHandle {}
        pub struct Gate {
            pub sandbox_deny: bool,
        }
        pub struct ArgMeta {
            pub name: &'static str,
            pub ts: fn() -> String,
        }
        pub struct CommandMeta {
            pub name: &'static str,
            pub args: &'static [ArgMeta],
            pub ret: fn() -> String,
        }
        pub async fn read<H: VaultHost, R>(
            _host: H,
            _meta: &CommandMeta,
            _gate: Gate,
            _body: impl FnOnce(&crate::UnlockedVault) -> Result<R, crate::ipc::IpcError>,
        ) -> Result<R, crate::ipc::IpcError> {
            unimplemented!()
        }
        pub async fn write<H: VaultHost, R>(
            _host: H,
            _meta: &CommandMeta,
            _gate: Gate,
            _body: impl FnOnce(&mut crate::UnlockedVault) -> Result<R, crate::ipc::IpcError>,
        ) -> Result<R, crate::ipc::IpcError> {
            unimplemented!()
        }
        pub async fn locked_ok<R>(
            _host: crate::tauri::AppHandle,
            _meta: &CommandMeta,
            _gate: Gate,
            _body: impl FnOnce(&crate::AppState) -> Result<R, crate::ipc::IpcError>,
        ) -> Result<R, crate::ipc::IpcError> {
            unimplemented!()
        }
    }
}
pub struct UnlockedVault;
pub struct AppState;

#[ipc(write, sandbox = deny)]
fn change(_vault: &mut UnlockedVault) -> Result<(), ipc::IpcError> {
    Ok(())
}

fn main() {
    assert_eq!(__IPC_META_change.name, "change");
}
