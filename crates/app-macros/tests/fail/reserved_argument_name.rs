use mwt_app_macros::ipc;

#[ipc(read)]
fn list(vault: &UnlockedVault, __ipc_host: String) -> Result<(), ()> {
    Ok(())
}

fn main() {}
