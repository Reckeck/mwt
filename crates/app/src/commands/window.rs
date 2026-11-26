//! The window's own background, which shows before the page paints and behind
//! it on a resize. The front end owns the colours (the theme's `--background`);
//! this side only checks the value and paints it, so it keeps no colour table.

use crate::ipc::commands::ipc_commands;
use tauri::AppHandle;
#[cfg(desktop)]
use tauri::Manager;

/// `#rrggbb` as three bytes; anything else is refused.
fn parse_rgb(rgb: &str) -> Result<(u8, u8, u8), String> {
    let digits = rgb
        .strip_prefix('#')
        .filter(|digits| digits.len() == 6 && digits.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or_else(|| format!("invalid colour {rgb:?}, expected #rrggbb"))?;
    let channel =
        |at: usize| u8::from_str_radix(&digits[at..at + 2], 16).map_err(|err| err.to_string());
    Ok((channel(0)?, channel(2)?, channel(4)?))
}

/// Paints the main window's background with the theme's colour; does nothing on a phone.
#[tauri::command]
pub async fn set_window_background(app: AppHandle, rgb: String) -> Result<(), String> {
    let (red, green, blue) = parse_rgb(&rgb)?;
    #[cfg(desktop)]
    if let Some(window) = app.get_webview_window("main") {
        window
            .set_background_color(Some(tauri::window::Color(red, green, blue, 255)))
            .map_err(|err| err.to_string())?;
    }
    #[cfg(not(desktop))]
    let _ = (app, red, green, blue);
    Ok(())
}

ipc_commands!(window_commands, WINDOW_NAMES, commands::window: [set_window_background]);

#[cfg(test)]
mod tests {
    use super::parse_rgb;

    #[test]
    fn parses_a_six_digit_hex_colour() {
        assert_eq!(parse_rgb("#1E1e2E"), Ok((0x1E, 0x1E, 0x2E)));
        assert_eq!(parse_rgb("#ffffff"), Ok((255, 255, 255)));
    }

    #[test]
    fn rejects_anything_else() {
        for bad in [
            "",
            "#fff",
            "ffffff",
            "#gggggg",
            "#1234567",
            "#12345",
            "rgb(1,2,3)",
            "#ffffffff",
            "#é12345",
        ] {
            assert!(parse_rgb(bad).is_err(), "{bad:?} must be refused");
        }
    }
}
