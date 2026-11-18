use mwt_app_macros::ipc;

#[ipc(read)]
fn list(vault: &UnlockedVault, app: String) -> Result<(), ()> {
    Ok(())
}

fn main() {}
