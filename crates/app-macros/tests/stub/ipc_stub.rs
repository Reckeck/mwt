
mod tauri {
    pub use mwt_app_macros::passthrough as command;
    #[derive(Clone)]
    pub struct AppHandle;
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
        }
        pub struct CommandMeta {
            pub name: &'static str,
            pub args: &'static [ArgMeta],
        }
        pub async fn read<H: VaultHost, R>(
            _host: H,
            _meta: &CommandMeta,
            _gate: Gate,
            _body: impl FnOnce(&crate::Session) -> Result<R, crate::ipc::IpcError>,
        ) -> Result<R, crate::ipc::IpcError> {
            unimplemented!()
        }
        pub async fn write<H: VaultHost, R>(
            _host: H,
            _meta: &CommandMeta,
            _gate: Gate,
            _body: impl FnOnce(&mut crate::Session) -> Result<R, crate::ipc::IpcError>,
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
pub struct Session;
pub struct AppState;
