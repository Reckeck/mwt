//! The system tray: show or hide the window, open another one, jump to a
//! screen, add an operation, refresh prices, lock, quit. Navigation goes through the
//! `tray-navigate` event so the UI changes route without reloading the page.

use tauri::menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

/// `(menu id, route name, Russian label, English label)` of the screens the tray opens.
const SCREENS: [(&str, &str, &str, &str); 6] = [
    ("nav_dashboard", "dashboard", "Дашборд", "Dashboard"),
    ("nav_stores", "stores", "Кошельки", "Wallets"),
    (
        "nav_transactions",
        "transactions",
        "Операции",
        "Transactions",
    ),
    (
        "nav_calendar",
        "transactions-calendar",
        "Календарь",
        "Calendar",
    ),
    (
        "nav_analytics",
        "analytics-cashflow",
        "Аналитика",
        "Analytics",
    ),
    (
        "nav_planning",
        "planning-automations",
        "Планирование",
        "Planning",
    ),
];

fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        window.show().ok();
        window.unminimize().ok();
        window.set_focus().ok();
    }
}

fn toggle_main(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if window.is_visible().unwrap_or(false) && !window.is_minimized().unwrap_or(false) {
        window.hide().ok();
    } else {
        show_main(app);
    }
}

/// Start another instance: a second window to put beside this one (another
/// vault, or another screen of the same).
fn open_new_window() {
    if let Ok(exe) = std::env::current_exe() {
        std::process::Command::new(exe).spawn().ok();
    }
}

pub fn create(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle().clone();
    let ru = crate::scheduler::russian(&handle);
    let label = |ru_text: &str, en_text: &str| {
        if ru {
            ru_text.to_string()
        } else {
            en_text.to_string()
        }
    };

    let toggle = MenuItemBuilder::new(label("Показать / скрыть", "Show / hide"))
        .id("toggle")
        .build(&handle)?;
    let quick_add = MenuItemBuilder::new(label("+ Операция", "+ Operation"))
        .id("quick_add")
        .build(&handle)?;
    let new_window = MenuItemBuilder::new(label("Новое окно", "New window"))
        .id("new_window")
        .build(&handle)?;
    let screens = SCREENS
        .iter()
        .map(|(id, _, ru_text, en_text)| {
            MenuItemBuilder::new(label(ru_text, en_text))
                .id(*id)
                .build(&handle)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let settings = MenuItemBuilder::new(label("Настройки", "Settings"))
        .id("settings")
        .build(&handle)?;
    let refresh = MenuItemBuilder::new(label("Обновить цены", "Refresh prices"))
        .id("refresh_prices")
        .build(&handle)?;
    let lock = MenuItemBuilder::new(label("Заблокировать", "Lock"))
        .id("lock")
        .build(&handle)?;
    let quit = MenuItemBuilder::new(label("Закрыть полностью", "Quit completely"))
        .id("quit")
        .build(&handle)?;

    let mut menu = MenuBuilder::new(&handle)
        .item(&toggle)
        .item(&new_window)
        .item(&quick_add)
        .item(&PredefinedMenuItem::separator(&handle)?);
    for item in &screens {
        menu = menu.item(item);
    }
    let menu = menu
        .item(&settings)
        .item(&PredefinedMenuItem::separator(&handle)?)
        .item(&refresh)
        .item(&lock)
        .item(&PredefinedMenuItem::separator(&handle)?)
        .item(&quit)
        .build()?;

    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or("the app has no window icon for the tray")?;
    TrayIconBuilder::with_id("main")
        .icon(icon)
        .tooltip("Multi Wallet Tracker")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_main(tray.app_handle());
            }
        })
        .on_menu_event(|app, event| {
            let id = event.id.as_ref();
            match id {
                "toggle" => toggle_main(app),
                "quick_add" => crate::shortcut::show_quick_add(app),
                "new_window" => open_new_window(),
                "settings" => {
                    show_main(app);
                    app.emit("tray-navigate", "settings").ok();
                }
                "refresh_prices" => {
                    app.emit("refresh-prices", ()).ok();
                }
                "lock" => {
                    show_main(app);
                    app.emit("toggle-lock", ()).ok();
                }
                "quit" => app.exit(0),
                _ => {
                    if let Some((_, route, _, _)) =
                        SCREENS.iter().find(|(menu_id, ..)| *menu_id == id)
                    {
                        show_main(app);
                        app.emit("tray-navigate", *route).ok();
                    }
                }
            }
        })
        .build(app)?;
    Ok(())
}
