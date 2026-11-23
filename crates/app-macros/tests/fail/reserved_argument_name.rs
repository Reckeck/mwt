use mwt_app_macros::ipc;

#[ipc(read)]
fn list(vault: &Session, __ipc_host: String) -> Result<(), ()> {
    Ok(())
}

fn main() {}
