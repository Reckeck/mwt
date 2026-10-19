//! Phone-only window behaviour, as commands that do nothing on a desktop:
//! the colour of the system bar icons and «minimise» on the system Back.

use tauri::AppHandle;

/// Light icons on a dark theme, dark ones on a light theme.
#[tauri::command]
pub fn set_system_bars(app: AppHandle, dark: bool) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        use tauri_plugin_insets::InsetsExt;
        app.insets().set_bar_style(dark)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, dark);
        Ok(())
    }
}

/// Back on the home screen: the app goes to the background, it does not quit.
#[tauri::command]
pub fn move_to_back(app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        use tauri_plugin_insets::InsetsExt;
        app.insets().move_to_back()
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Ok(())
    }
}
