use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE};
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    MessageBoxW, MB_ICONINFORMATION, MB_SETFOREGROUND,
};

const MUTEX_NAME: &str = "PressPlay_IrisTray_SingleInstance_Mutex";

pub struct SingleInstanceGuard {
    handle: HANDLE,
}

impl SingleInstanceGuard {
    pub fn acquire() -> Option<Self> {
        unsafe {
            let name_wide: Vec<u16> = MUTEX_NAME
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let handle = CreateMutexW(std::ptr::null_mut(), 0, name_wide.as_ptr());
            if handle == 0 {
                return None;
            }

            if GetLastError() == ERROR_ALREADY_EXISTS {
                let msg_wide: Vec<u16> = "IRIS v2 Battery Tracker is already running in the system tray.\nCheck your taskbar notification area (click the ^ arrow)."
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .collect();
                let title_wide: Vec<u16> = "IRIS v2 Battery Tracker"
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .collect();

                MessageBoxW(
                    0,
                    msg_wide.as_ptr(),
                    title_wide.as_ptr(),
                    MB_ICONINFORMATION | MB_SETFOREGROUND,
                );
                CloseHandle(handle);
                return None;
            }

            Some(Self { handle })
        }
    }
}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        if self.handle != 0 {
            unsafe {
                CloseHandle(self.handle);
            }
            self.handle = 0;
        }
    }
}
