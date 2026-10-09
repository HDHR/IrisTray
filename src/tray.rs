use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_INFO, NIM_ADD, NIM_DELETE,
    NIM_MODIFY, NIM_SETVERSION, NIN_SELECT, NOTIFYICONDATAW, NOTIFYICON_VERSION_4,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    DispatchMessageW, GetCursorPos, GetMessageW, GetSystemMetrics, GetWindowLongPtrW, LoadIconW,
    MessageBoxW, PostMessageW, PostQuitMessage, RegisterClassExW, RegisterWindowMessageW,
    SetForegroundWindow, SetWindowLongPtrW, TrackPopupMenuEx, TranslateMessage, CREATESTRUCTW,
    GWLP_USERDATA, HICON, MB_ICONERROR, MB_OK, MF_CHECKED, MF_GRAYED, MF_POPUP, MF_SEPARATOR,
    MF_STRING, MF_UNCHECKED, SM_CXSMICON, TPM_BOTTOMALIGN, TPM_RETURNCMD, TPM_RIGHTALIGN, WM_CLOSE,
    WM_COMMAND, WM_CONTEXTMENU, WM_DESTROY, WM_DPICHANGED, WM_LBUTTONUP, WM_NCCREATE, WM_NCDESTROY,
    WM_RBUTTONUP, WM_SETTINGCHANGE, WM_USER, WNDCLASSEXW,
};

use crate::config::ConfigManager;
use crate::dark_mode;
use crate::icon::IconRenderer;
use crate::mouse::{IrisMouseDetector, MouseState};

// Shell notification message constants
const NIN_KEYSELECT: u32 = WM_USER + 1;

// Application-specific window messages (offset from WM_USER to prevent shell event collision)
const WM_TRAYICON: u32 = WM_USER + 10;
const WM_STATE_UPDATED: u32 = WM_USER + 11;

// Menu Command IDs
const CMD_STATUS_HEADER: usize = 1001;
const CMD_REFRESH: usize = 1004;
const CMD_INTERVAL_15: usize = 1201;
const CMD_INTERVAL_30: usize = 1202;
const CMD_INTERVAL_60: usize = 1203;
const CMD_INTERVAL_120: usize = 1204;
const CMD_INTERVAL_300: usize = 1205;
const CMD_NOTIFY_LOW: usize = 1301;
const CMD_NOTIFY_CRITICAL: usize = 1302;
const CMD_NOTIFY_CHARGING: usize = 1303;
const CMD_NOTIFY_FULL: usize = 1304;
const CMD_STARTUP: usize = 1401;
const CMD_ABOUT: usize = 1501;
const CMD_EXIT: usize = 1999;

static WM_TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);

pub struct TrayApp {
    pub hwnd: HWND,
    pub cm: ConfigManager,
    pub current_state: MouseState,
    pub last_visual_state: Option<(u8, bool, bool, bool, usize)>,
    pub current_icon: HICON,
    pub nid: NOTIFYICONDATAW,
    pub refresh_tx: Sender<()>,
    pub state_rx: Receiver<MouseState>,
    pub running: Arc<AtomicBool>,
    pub polling_interval: Arc<AtomicU64>,
    pub in_menu: bool,
    // Notification deduplication state
    pub notified_low: bool,
    pub notified_critical: bool,
    pub notified_full: bool,
    pub last_charge_state: Option<bool>,
}

impl TrayApp {
    pub fn new(
        refresh_tx: Sender<()>,
        state_rx: Receiver<MouseState>,
        running: Arc<AtomicBool>,
    ) -> Self {
        let cm = ConfigManager::new();
        let initial_interval = cm.config.polling_interval_sec.max(5);
        let mut initial_state = MouseState::default();
        if let Some(bat) = cm.config.last_known_battery {
            if bat > 0 {
                initial_state.battery = bat;
            }
        }
        Self {
            hwnd: 0,
            cm,
            current_state: initial_state,
            last_visual_state: None,
            current_icon: 0,
            nid: unsafe { std::mem::zeroed() },
            refresh_tx,
            state_rx,
            running,
            polling_interval: Arc::new(AtomicU64::new(initial_interval)),
            in_menu: false,
            notified_low: false,
            notified_critical: false,
            notified_full: false,
            last_charge_state: None,
        }
    }

    pub fn init_window(&mut self, user_data_ptr: isize) -> Result<(), &'static str> {
        unsafe {
            let hinstance = GetModuleHandleW(std::ptr::null());
            let app_icon = LoadIconW(hinstance, 1 as *const u16);
            let class_name: Vec<u16> = "PressPlay_IrisTray_WindowClass"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();

            let wnd_class = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: 0,
                lpfnWndProc: Some(wnd_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: hinstance,
                hIcon: app_icon,
                hCursor: 0,
                hbrBackground: 0,
                lpszMenuName: std::ptr::null(),
                lpszClassName: class_name.as_ptr(),
                hIconSm: app_icon,
            };

            RegisterClassExW(&wnd_class);

            let hwnd = CreateWindowExW(
                0,
                class_name.as_ptr(),
                class_name.as_ptr(),
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                hinstance,
                user_data_ptr as *const std::ffi::c_void,
            );

            if hwnd == 0 {
                return Err("Failed to create tray hidden window");
            }

            self.hwnd = hwnd;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, user_data_ptr);

            // Register TaskbarCreated message for Windows Explorer crash/restart recovery
            let taskbar_str = to_wide("TaskbarCreated");
            let msg_taskbar = RegisterWindowMessageW(taskbar_str.as_ptr());
            WM_TASKBAR_CREATED.store(msg_taskbar, Ordering::Relaxed);

            // Enable dark mode attributes for the window
            dark_mode::enable_dark_mode_for_window(hwnd);

            // Setup NotifyIcon structure
            self.nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            self.nid.hWnd = hwnd;
            self.nid.uID = 1;
            self.nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
            self.nid.uCallbackMessage = WM_TRAYICON;

            // Generate initial icon (starts in neutral/disconnected state instantly)
            self.update_tray_icon();
            Shell_NotifyIconW(NIM_ADD, &self.nid);

            // Set NOTIFYICON_VERSION_4 for modern notification & keyboard accessibility behavior
            let mut version_nid: NOTIFYICONDATAW = std::mem::zeroed();
            version_nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            version_nid.hWnd = hwnd;
            version_nid.uID = 1;
            version_nid.Anonymous.uVersion = NOTIFYICON_VERSION_4;
            Shell_NotifyIconW(NIM_SETVERSION, &version_nid);

            Ok(())
        }
    }

    /// Restores the tray icon when Windows Explorer restarts.
    pub fn restore_tray_icon(&mut self) {
        self.nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        unsafe {
            Shell_NotifyIconW(NIM_ADD, &self.nid);

            let mut version_nid: NOTIFYICONDATAW = std::mem::zeroed();
            version_nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            version_nid.hWnd = self.hwnd;
            version_nid.uID = 1;
            version_nid.Anonymous.uVersion = NOTIFYICON_VERSION_4;
            Shell_NotifyIconW(NIM_SETVERSION, &version_nid);
        }
        self.last_visual_state = None; // Force redrawing icon on restored taskbar
        self.update_tray_icon();
    }

    pub fn update_tray_icon(&mut self) {
        let s = &self.current_state;
        let sleeping = s.connected && s.is_wireless && !s.wireless_connected;
        let sys_metric = unsafe { GetSystemMetrics(SM_CXSMICON) as usize };
        let icon_size = sys_metric.clamp(16, 64);
        let visual_state = (s.battery, s.is_charging, s.connected, sleeping, icon_size);

        // Update tooltip text buffer
        let tooltip_text = self.get_tooltip_string();
        let tooltip_wide: Vec<u16> = tooltip_text
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let max_tip = self.nid.szTip.len();
        for (i, &ch) in tooltip_wide
            .iter()
            .take(max_tip.saturating_sub(1))
            .enumerate()
        {
            self.nid.szTip[i] = ch;
        }
        self.nid.szTip[max_tip - 1] = 0;

        // Avoid rebuilding HICON and DIB sections if visual state hasn't changed
        if self.last_visual_state == Some(visual_state) {
            unsafe {
                let mut tip_nid = self.nid;
                tip_nid.uFlags = NIF_TIP;
                Shell_NotifyIconW(NIM_MODIFY, &tip_nid);
            }
            return;
        }

        let new_icon = IconRenderer::create_hicon(s.battery, s.is_charging, s.connected, sleeping);

        if new_icon != 0 {
            self.last_visual_state = Some(visual_state);
            let old_icon = self.current_icon;
            self.current_icon = new_icon;
            self.nid.hIcon = new_icon;
            self.nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;

            unsafe {
                Shell_NotifyIconW(NIM_MODIFY, &self.nid);
                if old_icon != 0 {
                    IconRenderer::destroy_hicon(old_icon);
                }
            }
        }
    }

    fn get_tooltip_string(&self) -> String {
        let s = &self.current_state;
        if !s.connected {
            if !s.status_text.is_empty() && s.status_text != "Disconnected" {
                return format!("IRIS v2: {}", s.status_text);
            }
            return "IRIS v2 NearLink: Disconnected".to_string();
        }
        if s.is_wireless && !s.wireless_connected {
            return format!(
                "IRIS v2: {}% (Sleeping / Offline)\nNearLink Wireless",
                s.battery
            );
        }
        let mode_txt = if s.is_wireless {
            "Wireless NearLink"
        } else {
            "Wired USB"
        };
        if s.is_charging {
            if s.battery >= 100 {
                format!("IRIS v2: 100% (Fully Charged - Plugged In)\n{}", mode_txt)
            } else {
                format!("IRIS v2: {}% (Charging)\n{}", s.battery, mode_txt)
            }
        } else {
            format!("IRIS v2: {}%\n{}", s.battery, mode_txt)
        }
    }

    /// Safely drains state updates from the thread-safe channel without raw pointers over Win32 IPC.
    pub fn drain_state_updates(&mut self) {
        let mut latest = None;
        while let Ok(state) = self.state_rx.try_recv() {
            latest = Some(state);
        }
        if let Some(state) = latest {
            self.on_state_updated(state);
        }
    }

    pub fn on_state_updated(&mut self, state: MouseState) {
        if state.connected
            && state.battery > 0
            && self.cm.config.last_known_battery != Some(state.battery)
        {
            self.cm.config.last_known_battery = Some(state.battery);
            self.cm.save();
        }
        self.current_state = state;
        self.check_notifications();
        self.update_tray_icon();
    }

    fn check_notifications(&mut self) {
        if !self.current_state.connected {
            self.last_charge_state = None;
            return;
        }

        // Suppress false low-battery or discharging alerts when mouse is offline / sleeping
        if self.current_state.is_wireless && !self.current_state.wireless_connected {
            if self.last_charge_state == Some(true) && !self.current_state.is_charging {
                self.last_charge_state = Some(false);
            }
            return;
        }

        let battery = self.current_state.battery;
        let is_charging = self.current_state.is_charging;
        let cfg = self.cm.config.clone();

        // 1. Critical battery (check first to avoid simultaneous low battery alert)
        if cfg.notify_critical {
            if battery <= cfg.critical_threshold && !is_charging && !self.notified_critical {
                self.send_notification(
                    "IRIS v2 Battery Critical!",
                    &format!("Mouse battery is critically low ({}%). Recharging immediately recommended.", battery),
                );
                self.notified_critical = true;
                self.notified_low = true; // Suppress redundant low battery notification
            } else if battery > cfg.critical_threshold + 5 {
                self.notified_critical = false;
            }
        }

        // 2. Low battery (notify if below low threshold and either above critical threshold or critical alerts disabled)
        if cfg.notify_low_battery {
            let above_crit = !cfg.notify_critical || battery > cfg.critical_threshold;
            if battery <= cfg.low_battery_threshold
                && above_crit
                && !is_charging
                && !self.notified_low
            {
                self.send_notification(
                    "IRIS v2 Battery Low",
                    &format!("Battery is at {}%. Please connect your charger.", battery),
                );
                self.notified_low = true;
            } else if battery > cfg.low_battery_threshold + 5 {
                self.notified_low = false;
            }
        }

        // 3. Charging status changed
        if cfg.notify_charging {
            if let Some(last_chg) = self.last_charge_state {
                if !last_chg && is_charging {
                    self.send_notification(
                        "IRIS v2 Charging",
                        &format!("Mouse is charging ({}%).", battery),
                    );
                } else if last_chg && !is_charging && battery < 99 {
                    self.send_notification(
                        "IRIS v2 Discharging",
                        &format!("Mouse running on battery ({}%).", battery),
                    );
                }
            }
        }
        self.last_charge_state = Some(is_charging);

        // 4. Fully charged (now correctly fires when plugged in and battery reaches 100%)
        if cfg.notify_fully_charged {
            if is_charging && battery >= 100 && !self.notified_full {
                self.send_notification("IRIS v2 Battery Full", "Mouse is fully charged (100%).");
                self.notified_full = true;
            } else if battery < 95 || !is_charging {
                self.notified_full = false;
            }
        }
    }

    pub fn send_notification(&mut self, title: &str, message: &str) {
        let title_wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
        let msg_wide: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();

        let mut nid = self.nid;
        nid.uFlags |= NIF_INFO;
        nid.dwInfoFlags = NIIF_INFO;

        let max_t = nid.szInfoTitle.len();
        for (i, &ch) in title_wide.iter().take(max_t.saturating_sub(1)).enumerate() {
            nid.szInfoTitle[i] = ch;
        }
        nid.szInfoTitle[max_t - 1] = 0;

        let max_m = nid.szInfo.len();
        for (i, &ch) in msg_wide.iter().take(max_m.saturating_sub(1)).enumerate() {
            nid.szInfo[i] = ch;
        }
        nid.szInfo[max_m - 1] = 0;

        unsafe {
            Shell_NotifyIconW(NIM_MODIFY, &nid);
        }
    }

    pub fn build_context_menu(&mut self) -> (isize, POINT) {
        dark_mode::init_dark_mode();

        unsafe {
            let mut pt = POINT { x: 0, y: 0 };
            GetCursorPos(&mut pt);

            let menu = CreatePopupMenu();

            // Status header
            let header_text = self.get_menu_header_text();
            let header_wide = to_wide(&header_text);
            AppendMenuW(menu, MF_STRING, CMD_STATUS_HEADER, header_wide.as_ptr());

            // Status details
            let status_desc = self.get_menu_desc_text();
            let desc_wide = to_wide(&status_desc);
            AppendMenuW(menu, MF_STRING | MF_GRAYED, 0, desc_wide.as_ptr());

            // Firmware info
            let s = &self.current_state;
            let fw_text = format!(
                "Firmware: Mouse v{} | Dongle v{}",
                s.mouse_version, s.dongle_version
            );
            let fw_wide = to_wide(&fw_text);
            AppendMenuW(menu, MF_STRING | MF_GRAYED, 0, fw_wide.as_ptr());

            AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());

            // Refresh item
            let ref_wide = to_wide("\u{1f504} Refresh Battery Now");
            AppendMenuW(menu, MF_STRING, CMD_REFRESH, ref_wide.as_ptr());

            // Polling interval submenu
            let interval_menu = CreatePopupMenu();
            let cur_int = self.cm.config.polling_interval_sec;
            let chk = |secs: u64| {
                if cur_int == secs {
                    MF_CHECKED
                } else {
                    MF_UNCHECKED
                }
            };

            let i15_w = to_wide("15 seconds");
            let i30_w = to_wide("30 seconds (Default)");
            let i60_w = to_wide("1 minute");
            let i120_w = to_wide("2 minutes");
            let i300_w = to_wide("5 minutes");
            AppendMenuW(
                interval_menu,
                MF_STRING | chk(15),
                CMD_INTERVAL_15,
                i15_w.as_ptr(),
            );
            AppendMenuW(
                interval_menu,
                MF_STRING | chk(30),
                CMD_INTERVAL_30,
                i30_w.as_ptr(),
            );
            AppendMenuW(
                interval_menu,
                MF_STRING | chk(60),
                CMD_INTERVAL_60,
                i60_w.as_ptr(),
            );
            AppendMenuW(
                interval_menu,
                MF_STRING | chk(120),
                CMD_INTERVAL_120,
                i120_w.as_ptr(),
            );
            AppendMenuW(
                interval_menu,
                MF_STRING | chk(300),
                CMD_INTERVAL_300,
                i300_w.as_ptr(),
            );
            let interval_parent_w = to_wide("\u{23f1} Polling Interval");
            AppendMenuW(
                menu,
                MF_POPUP,
                interval_menu as usize,
                interval_parent_w.as_ptr(),
            );

            // Notifications submenu
            let notif_menu = CreatePopupMenu();
            let n_low_flags = if self.cm.config.notify_low_battery {
                MF_CHECKED
            } else {
                MF_UNCHECKED
            };
            let n_crit_flags = if self.cm.config.notify_critical {
                MF_CHECKED
            } else {
                MF_UNCHECKED
            };
            let n_chg_flags = if self.cm.config.notify_charging {
                MF_CHECKED
            } else {
                MF_UNCHECKED
            };
            let n_full_flags = if self.cm.config.notify_fully_charged {
                MF_CHECKED
            } else {
                MF_UNCHECKED
            };

            let n_low_label = format!(
                "Low Battery Alert ({}%)",
                self.cm.config.low_battery_threshold
            );
            let n_crit_label = format!("Critical Alert ({}%)", self.cm.config.critical_threshold);
            let n_low_w = to_wide(&n_low_label);
            let n_crit_w = to_wide(&n_crit_label);
            let n_chg_w = to_wide("Charging Status Alerts");
            let n_full_w = to_wide("Fully Charged Alert (100%)");
            AppendMenuW(
                notif_menu,
                MF_STRING | n_low_flags,
                CMD_NOTIFY_LOW,
                n_low_w.as_ptr(),
            );
            AppendMenuW(
                notif_menu,
                MF_STRING | n_crit_flags,
                CMD_NOTIFY_CRITICAL,
                n_crit_w.as_ptr(),
            );
            AppendMenuW(
                notif_menu,
                MF_STRING | n_chg_flags,
                CMD_NOTIFY_CHARGING,
                n_chg_w.as_ptr(),
            );
            AppendMenuW(
                notif_menu,
                MF_STRING | n_full_flags,
                CMD_NOTIFY_FULL,
                n_full_w.as_ptr(),
            );
            let notif_parent_w = to_wide("\u{1f514} Notifications");
            AppendMenuW(menu, MF_POPUP, notif_menu as usize, notif_parent_w.as_ptr());

            // Start with Windows
            let startup_flags = if self.cm.config.start_with_windows {
                MF_CHECKED
            } else {
                MF_UNCHECKED
            };
            let startup_w = to_wide("\u{1f680} Start with Windows");
            AppendMenuW(
                menu,
                MF_STRING | startup_flags,
                CMD_STARTUP,
                startup_w.as_ptr(),
            );

            AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());

            // About & Exit
            let about_w = to_wide("\u{2139} About IRIS v2 Tracker");
            let exit_w = to_wide("\u{274c} Exit");
            AppendMenuW(menu, MF_STRING, CMD_ABOUT, about_w.as_ptr());
            AppendMenuW(menu, MF_STRING, CMD_EXIT, exit_w.as_ptr());

            (menu, pt)
        }
    }

    fn get_menu_header_text(&self) -> String {
        let s = &self.current_state;
        if !s.connected {
            if !s.status_text.is_empty()
                && s.status_text != "Disconnected"
                && s.status_text != "Dongle/Mouse Disconnected"
            {
                return format!("IRIS v2: {}", s.status_text);
            }
            "IRIS v2: Disconnected".to_string()
        } else if s.is_wireless && !s.wireless_connected {
            format!("IRIS v2: {}% (Sleeping / Offline)", s.battery)
        } else if s.is_charging {
            if s.battery >= 100 {
                "IRIS v2: 100% (Fully Charged)".to_string()
            } else {
                format!("IRIS v2: {}% (Charging)", s.battery)
            }
        } else {
            format!("IRIS v2: {}%", s.battery)
        }
    }

    fn get_menu_desc_text(&self) -> String {
        let s = &self.current_state;
        if !s.connected {
            if !s.status_text.is_empty() && s.status_text != "Disconnected" {
                return format!("Status: {}", s.status_text);
            }
            "Plug in NearLink Dongle or USB Cable".to_string()
        } else if s.is_wireless && !s.wireless_connected {
            "Status: Mouse Sleeping / Offline".to_string()
        } else if s.is_charging {
            if s.battery >= 100 {
                "Status: Fully Charged (Plugged In)".to_string()
            } else {
                "Status: Charging via USB".to_string()
            }
        } else {
            let mode = if s.is_wireless {
                "Wireless NearLink"
            } else {
                "Wired USB"
            };
            format!("Status: Connected ({})", mode)
        }
    }

    pub fn handle_command(&mut self, cmd: usize) {
        match cmd {
            CMD_STATUS_HEADER => {
                let s = &self.current_state;
                if s.connected {
                    let mode = if s.is_wireless {
                        "NearLink Wireless"
                    } else {
                        "Wired USB"
                    };
                    let chg = if s.is_charging {
                        if s.battery >= 100 {
                            "Fully Charged (Plugged In)"
                        } else {
                            "Yes (Charging)"
                        }
                    } else {
                        "No (Discharging)"
                    };
                    self.send_notification(
                        "Press Play IRIS v2 Status",
                        &format!(
                            "Battery: {}%\nCharging: {}\nConnection: {}\nMouse FW: v{}\nDongle FW: v{}",
                            s.battery, chg, mode, s.mouse_version, s.dongle_version
                        ),
                    );
                } else {
                    let msg = if !s.status_text.is_empty() && s.status_text != "Disconnected" {
                        format!("Device disconnected ({})", s.status_text)
                    } else {
                        "Device is currently disconnected.".to_string()
                    };
                    self.send_notification("Press Play IRIS v2", &msg);
                }
            }
            CMD_REFRESH => {
                let _ = self.refresh_tx.send(());
            }
            CMD_INTERVAL_15 => {
                self.cm.config.polling_interval_sec = 15;
                self.polling_interval.store(15, Ordering::SeqCst);
                self.cm.save();
                let _ = self.refresh_tx.send(());
            }
            CMD_INTERVAL_30 => {
                self.cm.config.polling_interval_sec = 30;
                self.polling_interval.store(30, Ordering::SeqCst);
                self.cm.save();
                let _ = self.refresh_tx.send(());
            }
            CMD_INTERVAL_60 => {
                self.cm.config.polling_interval_sec = 60;
                self.polling_interval.store(60, Ordering::SeqCst);
                self.cm.save();
                let _ = self.refresh_tx.send(());
            }
            CMD_INTERVAL_120 => {
                self.cm.config.polling_interval_sec = 120;
                self.polling_interval.store(120, Ordering::SeqCst);
                self.cm.save();
                let _ = self.refresh_tx.send(());
            }
            CMD_INTERVAL_300 => {
                self.cm.config.polling_interval_sec = 300;
                self.polling_interval.store(300, Ordering::SeqCst);
                self.cm.save();
                let _ = self.refresh_tx.send(());
            }
            CMD_NOTIFY_LOW => {
                self.cm.config.notify_low_battery = !self.cm.config.notify_low_battery;
                if self.cm.config.notify_low_battery {
                    self.notified_low = false;
                }
                self.cm.save();
                self.check_notifications();
            }
            CMD_NOTIFY_CRITICAL => {
                self.cm.config.notify_critical = !self.cm.config.notify_critical;
                if self.cm.config.notify_critical {
                    self.notified_critical = false;
                }
                self.cm.save();
                self.check_notifications();
            }
            CMD_NOTIFY_CHARGING => {
                self.cm.config.notify_charging = !self.cm.config.notify_charging;
                self.cm.save();
            }
            CMD_NOTIFY_FULL => {
                self.cm.config.notify_fully_charged = !self.cm.config.notify_fully_charged;
                if self.cm.config.notify_fully_charged {
                    self.notified_full = false;
                }
                self.cm.save();
                self.check_notifications();
            }
            CMD_STARTUP => {
                let new_val = !self.cm.config.start_with_windows;
                if !self.cm.set_startup(new_val) {
                    self.send_notification(
                        "IRIS v2 Settings",
                        "Failed to update Windows startup registry entry.",
                    );
                }
            }
            CMD_ABOUT => {
                self.send_notification(
                    "About IRIS v2 Battery Tracker",
                    &format!(
                        "Press Play IRIS v2 NearLink Battery Tracker v{}\nUltra-lightweight background system tray utility.",
                        env!("CARGO_PKG_VERSION")
                    ),
                );
            }
            CMD_EXIT => {
                self.running.store(false, Ordering::SeqCst);
                let _ = self.refresh_tx.send(());
                unsafe {
                    if self.hwnd != 0 {
                        DestroyWindow(self.hwnd);
                    } else {
                        PostQuitMessage(0);
                    }
                }
            }
            _ => {}
        }
    }
}

impl Drop for TrayApp {
    fn drop(&mut self) {
        unsafe {
            if self.hwnd != 0 {
                SetWindowLongPtrW(self.hwnd, GWLP_USERDATA, 0);
            }
            Shell_NotifyIconW(NIM_DELETE, &self.nid);
            if self.current_icon != 0 {
                IconRenderer::destroy_hicon(self.current_icon);
                self.current_icon = 0;
            }
        }
    }
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let mut app_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut RefCell<TrayApp>;

    if msg == WM_NCCREATE {
        let cs = lparam as *const CREATESTRUCTW;
        if !cs.is_null() {
            let create_ptr = (*cs).lpCreateParams as *mut RefCell<TrayApp>;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create_ptr as isize);
            app_ptr = create_ptr;
        }
    } else if msg == WM_NCDESTROY {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }

    if app_ptr.is_null() {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }

    let app_cell = &*app_ptr;

    // Explorer restart recovery
    let taskbar_msg = WM_TASKBAR_CREATED.load(Ordering::Relaxed);
    if taskbar_msg != 0 && msg == taskbar_msg {
        if let Ok(mut app) = app_cell.try_borrow_mut() {
            if !app.in_menu {
                app.restore_tray_icon();
            }
        }
        return 0;
    }

    match msg {
        WM_TRAYICON => {
            let event = (lparam & 0xFFFF) as u32;
            let show_context_menu =
                event == WM_RBUTTONUP || event == WM_CONTEXTMENU || event == NIN_KEYSELECT;

            let handle_default_action = event == WM_LBUTTONUP || event == NIN_SELECT;

            if show_context_menu {
                let menu_info = if let Ok(mut app) = app_cell.try_borrow_mut() {
                    if !app.in_menu {
                        app.in_menu = true;
                        let hwnd = app.hwnd;
                        let (menu, pt) = app.build_context_menu();
                        Some((menu, pt, hwnd))
                    } else {
                        None
                    }
                } else {
                    None
                };

                if let Some((menu, pt, menu_hwnd)) = menu_info {
                    // Modal TrackPopupMenuEx runs WITHOUT holding mutable borrow on app_cell
                    let selected_cmd = {
                        SetForegroundWindow(menu_hwnd);
                        let cmd = TrackPopupMenuEx(
                            menu,
                            TPM_BOTTOMALIGN | TPM_RIGHTALIGN | TPM_RETURNCMD,
                            pt.x,
                            pt.y,
                            menu_hwnd,
                            std::ptr::null(),
                        );
                        // Microsoft KB Q139526: Post WM_NULL to dismiss menu reliably on external click
                        PostMessageW(menu_hwnd, 0, 0, 0);
                        DestroyMenu(menu);
                        cmd as usize
                    };

                    if let Ok(mut app) = app_cell.try_borrow_mut() {
                        app.in_menu = false;
                        app.drain_state_updates();
                        if selected_cmd > 0 {
                            app.handle_command(selected_cmd);
                        }
                    }
                }
            } else if handle_default_action {
                if let Ok(mut app) = app_cell.try_borrow_mut() {
                    if !app.in_menu {
                        app.handle_command(CMD_STATUS_HEADER);
                    }
                }
            }
            0
        }
        WM_DPICHANGED | WM_SETTINGCHANGE => {
            if let Ok(mut app) = app_cell.try_borrow_mut() {
                if !app.in_menu {
                    app.last_visual_state = None;
                    app.update_tray_icon();
                }
            }
            0
        }
        WM_STATE_UPDATED => {
            if let Ok(mut app) = app_cell.try_borrow_mut() {
                if !app.in_menu {
                    app.drain_state_updates();
                }
            }
            0
        }
        WM_COMMAND => {
            let cmd = wparam & 0xFFFF;
            if let Ok(mut app) = app_cell.try_borrow_mut() {
                if !app.in_menu {
                    app.handle_command(cmd);
                }
            }
            0
        }
        WM_CLOSE | WM_DESTROY => {
            if let Ok(app) = app_cell.try_borrow_mut() {
                app.running.store(false, Ordering::SeqCst);
                let _ = app.refresh_tx.send(());
            }
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

pub fn run_tray_app() {
    let running = Arc::new(AtomicBool::new(true));
    let (refresh_tx, refresh_rx) = channel::<()>();
    let (state_tx, state_rx) = channel::<MouseState>();

    let app = Box::new(RefCell::new(TrayApp::new(
        refresh_tx,
        state_rx,
        running.clone(),
    )));
    let app_ptr = Box::into_raw(app);

    let (hwnd, polling_interval, initial_battery) = {
        let mut app_borrow = unsafe { (*app_ptr).borrow_mut() };
        if let Err(e) = app_borrow.init_window(app_ptr as isize) {
            let title_w = to_wide("Press Play IRIS v2 Error");
            let msg_w = to_wide(&format!("Failed to initialize tray application: {}", e));
            unsafe {
                MessageBoxW(0, msg_w.as_ptr(), title_w.as_ptr(), MB_ICONERROR | MB_OK);
                let _ = Box::from_raw(app_ptr);
            }
            return;
        }
        (
            app_borrow.hwnd,
            app_borrow.polling_interval.clone(),
            app_borrow.cm.config.last_known_battery.unwrap_or(0),
        )
    };

    // Background worker thread for polling (all HID queries run asynchronously)
    let running_worker = running.clone();
    thread::spawn(move || {
        let mut detector = IrisMouseDetector::with_initial_battery(initial_battery);

        while running_worker.load(Ordering::SeqCst) {
            let state = detector.query_state();
            if state_tx.send(state).is_err() {
                break;
            }
            unsafe {
                PostMessageW(hwnd, WM_STATE_UPDATED, 0, 0);
            }

            // Wait for refresh channel or timeout using user's configured interval
            let interval = polling_interval.load(Ordering::SeqCst).max(5);
            let _ = refresh_rx.recv_timeout(Duration::from_secs(interval));
        }
    });

    // Win32 Message Loop starts immediately without blocking on USB reads
    unsafe {
        let mut msg = std::mem::zeroed();
        while GetMessageW(&mut msg, 0, 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        let _ = Box::from_raw(app_ptr);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_wide() {
        let wide = to_wide("hello");
        assert_eq!(
            wide,
            vec!['h' as u16, 'e' as u16, 'l' as u16, 'l' as u16, 'o' as u16, 0]
        );
        let empty_wide = to_wide("");
        assert_eq!(empty_wide, vec![0]);
    }

    #[test]
    fn test_tray_text_formatting() {
        let (tx, _) = channel();
        let (_, rx) = channel();
        let running = Arc::new(AtomicBool::new(true));
        let mut app = TrayApp::new(tx, rx, running);

        // Disconnected
        app.current_state.connected = false;
        assert_eq!(app.get_menu_header_text(), "IRIS v2: Disconnected");
        assert_eq!(app.get_tooltip_string(), "IRIS v2 NearLink: Disconnected");

        // Connected Wireless Active
        app.current_state = MouseState {
            connected: true,
            battery: 85,
            is_charging: false,
            is_wireless: true,
            wireless_connected: true,
            ..Default::default()
        };
        assert_eq!(app.get_menu_header_text(), "IRIS v2: 85%");
        assert!(app.get_tooltip_string().contains("85%"));

        // Sleeping
        app.current_state.wireless_connected = false;
        assert_eq!(
            app.get_menu_header_text(),
            "IRIS v2: 85% (Sleeping / Offline)"
        );
        assert!(app.get_tooltip_string().contains("Sleeping / Offline"));

        // Charging
        app.current_state.wireless_connected = true;
        app.current_state.is_charging = true;
        assert_eq!(app.get_menu_header_text(), "IRIS v2: 85% (Charging)");
        assert!(app.get_tooltip_string().contains("Charging"));

        // Full
        app.current_state.battery = 100;
        assert_eq!(app.get_menu_header_text(), "IRIS v2: 100% (Fully Charged)");
        assert!(app.get_tooltip_string().contains("100%"));
    }
}
