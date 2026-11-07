//! Windows: sleep and the lock screen as the session reports them.
//!
//! A message-only window on its own thread receives `WM_POWERBROADCAST`
//! (`PBT_APMSUSPEND`) and `WM_WTSSESSION_CHANGE` (`WTS_SESSION_LOCK`). If the
//! window cannot be made, the sleep fallback of `Watch` (a gap between ticks)
//! still locks after a sleep; a screen lock is then not seen.

use std::sync::OnceLock;

use tauri::AppHandle;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::RemoteDesktop::{
    NOTIFY_FOR_THIS_SESSION, WTSRegisterSessionNotification,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, HWND_MESSAGE, MSG,
    PBT_APMSUSPEND, RegisterClassW, TranslateMessage, WM_POWERBROADCAST, WM_WTSSESSION_CHANGE,
    WNDCLASSW,
};

use super::{LockReason, on_os_event};

/// `WTS_SESSION_LOCK` of `wtsapi32.h`: the workstation was locked.
const WTS_SESSION_LOCK: usize = 7;

static APP: OnceLock<AppHandle> = OnceLock::new();

/// Which reason, if any, a window message stands for.
fn reason_of(message: u32, wparam: usize) -> Option<LockReason> {
    match message {
        WM_POWERBROADCAST if wparam == PBT_APMSUSPEND as usize => Some(LockReason::Sleep),
        WM_WTSSESSION_CHANGE if wparam == WTS_SESSION_LOCK => Some(LockReason::ScreenLock),
        _ => None,
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if let (Some(reason), Some(app)) = (reason_of(message, wparam), APP.get()) {
        on_os_event(app, reason);
    }
    // SAFETY: the arguments are the ones Windows handed to this procedure.
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn listen(app: AppHandle) {
    let _ = APP.set(app);
    std::thread::spawn(|| {
        if !pump() {
            eprintln!("autolock: no session notifications, only the sleep gap is watched");
        }
    });
}

/// Make the window and run its message loop; `false` = could not start.
fn pump() -> bool {
    let class_name = wide("MwtAutolockWindow");
    // SAFETY: plain Win32 calls with a class and a window made right here; the
    // strings outlive the calls and the message loop is on this thread.
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let mut class: WNDCLASSW = std::mem::zeroed();
        class.lpfnWndProc = Some(window_proc);
        class.hInstance = instance;
        class.lpszClassName = class_name.as_ptr();
        if RegisterClassW(&class) == 0 {
            return false;
        }
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            class_name.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            return false;
        }
        let _ = WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION);
        let mut message: MSG = std::mem::zeroed();
        while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suspend_and_lock_are_reasons_and_the_rest_is_not() {
        assert_eq!(
            reason_of(WM_POWERBROADCAST, PBT_APMSUSPEND as usize),
            Some(LockReason::Sleep)
        );
        assert_eq!(
            reason_of(WM_WTSSESSION_CHANGE, WTS_SESSION_LOCK),
            Some(LockReason::ScreenLock)
        );
        assert_eq!(reason_of(WM_WTSSESSION_CHANGE, 8), None);
        assert_eq!(reason_of(0, 0), None);
    }
}
