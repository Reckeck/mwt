// The Kotlin side is called from Rust only (`run_mobile_plugin`), so no
// command is exposed to the webview.
const COMMANDS: &[&str] = &[];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .build();
}
