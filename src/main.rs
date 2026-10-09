#![windows_subsystem = "windows"]

mod config;
mod dark_mode;
mod icon;
mod mouse;
mod single_instance;
mod tray;

use single_instance::SingleInstanceGuard;

fn main() {
    // Enable Per-Monitor V2 DPI awareness so Windows gives us native pixel coordinates
    // and doesn't bilinearly blur the system tray icon at 125%, 150%, etc.
    unsafe {
        windows_sys::Win32::UI::HiDpi::SetProcessDpiAwarenessContext(
            windows_sys::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        );
    }

    // Enable native Dark Mode for Win32 popup menus if system dark theme is active
    dark_mode::init_dark_mode();

    // Single instance protection: ensure only one tray instance runs
    let _guard = match SingleInstanceGuard::acquire() {
        Some(g) => g,
        None => return,
    };

    tray::run_tray_app();
}
