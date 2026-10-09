use std::sync::OnceLock;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::System::LibraryLoader::{
    GetModuleHandleW, GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_SYSTEM32,
};
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY_CURRENT_USER, KEY_READ,
};

type FnSetPreferredAppMode = unsafe extern "system" fn(i32) -> i32;
type FnFlushMenuThemes = unsafe extern "system" fn();
type FnAllowDarkModeForWindow = unsafe extern "system" fn(HWND, i32) -> i32;

struct DarkModeApi {
    set_preferred_app_mode: Option<FnSetPreferredAppMode>,
    flush_menu_themes: Option<FnFlushMenuThemes>,
    allow_dark_window: Option<FnAllowDarkModeForWindow>,
}

// Safely loads a system DLL strictly from %SystemRoot%\System32 to prevent DLL hijacking.
fn load_system_module(name: &str) -> isize {
    unsafe {
        let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let handle = GetModuleHandleW(wide.as_ptr());
        if handle != 0 {
            return handle;
        }
        LoadLibraryExW(wide.as_ptr(), 0, LOAD_LIBRARY_SEARCH_SYSTEM32)
    }
}

fn get_dark_mode_api() -> &'static DarkModeApi {
    static API: OnceLock<DarkModeApi> = OnceLock::new();
    API.get_or_init(|| unsafe {
        let handle = load_system_module("uxtheme.dll");
        if handle == 0 {
            return DarkModeApi {
                set_preferred_app_mode: None,
                flush_menu_themes: None,
                allow_dark_window: None,
            };
        }

        let proc_135 = GetProcAddress(handle, 135 as _);
        let set_preferred_app_mode = proc_135.map(|p| std::mem::transmute(p));

        let proc_136 = GetProcAddress(handle, 136 as _);
        let flush_menu_themes = proc_136.map(|p| std::mem::transmute(p));

        let proc_133 = GetProcAddress(handle, 133 as _);
        let allow_dark_window = proc_133.map(|p| std::mem::transmute(p));

        DarkModeApi {
            set_preferred_app_mode,
            flush_menu_themes,
            allow_dark_window,
        }
    })
}

/// Checks if Windows is currently in Dark Mode (AppsUseLightTheme == 0).
pub fn is_system_dark_mode() -> bool {
    unsafe {
        let key_wide: Vec<u16> =
            "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
        let mut hkey = 0;
        if RegOpenKeyExW(HKEY_CURRENT_USER, key_wide.as_ptr(), 0, KEY_READ, &mut hkey) == 0 {
            let val_wide: Vec<u16> = "AppsUseLightTheme"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let mut typ = 0;
            let mut val: u32 = 1;
            let mut size = std::mem::size_of::<u32>() as u32;
            let status = RegQueryValueExW(
                hkey,
                val_wide.as_ptr(),
                std::ptr::null_mut(),
                &mut typ,
                &mut val as *mut u32 as *mut u8,
                &mut size,
            );
            RegCloseKey(hkey);
            if status == 0 {
                return val == 0;
            }
        }
    }
    // Default to dark mode on modern Windows if query fails
    true
}

/// Enables native Windows 10/11 Dark Mode for the application and Win32 popup menus.
pub fn init_dark_mode() {
    let api = get_dark_mode_api();
    if let Some(set_preferred_app_mode) = api.set_preferred_app_mode {
        // Ordinal 135: SetPreferredAppMode (Windows 10 1903+ / Windows 11) / AllowDarkModeForApp (1809)
        // 2 = ForceDark in 1903+ (TRUE in 1809); 0 = Default/Light in 1903+ (FALSE in 1809)
        let mode = if is_system_dark_mode() { 2 } else { 0 };
        unsafe {
            set_preferred_app_mode(mode);
            if let Some(flush_menu_themes) = api.flush_menu_themes {
                flush_menu_themes();
            }
        }
    }
}

/// Applies dark mode attributes to a specific window handle upon creation.
pub fn enable_dark_mode_for_window(hwnd: HWND) {
    if hwnd == 0 {
        return;
    }

    let api = get_dark_mode_api();
    if let Some(allow_dark_window) = api.allow_dark_window {
        let is_dark = is_system_dark_mode();
        unsafe {
            allow_dark_window(hwnd, if is_dark { 1 } else { 0 });
        }
    }
}
