use mwt_app_macros::ipc;

#[ipc(read)]
fn list(vault: &Session, app: String) -> Result<(), ()> {
    Ok(())
}

fn main() {}
