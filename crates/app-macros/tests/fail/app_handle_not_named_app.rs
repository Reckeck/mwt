use mwt_app_macros::ipc;

#[ipc(locked_ok)]
fn status(state: &AppState, handle: &AppHandle) -> Result<(), ()> {
    Ok(())
}

fn main() {}
