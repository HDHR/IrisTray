use std::ffi::CString;

pub const VENDOR_ID: u16 = 0x363C;
pub const PID_WIRED: u16 = 0xEC05;
pub const PID_DONGLE: u16 = 0xEC06;

pub const USAGE_PAGE: u16 = 0xFFA0;
pub const USAGE_ID: u16 = 0x0002;

pub const REPORT_ID_OUT: u8 = 11; // 0x0B
pub const CMD_GET_INFO: u8 = 19; // 0x13
pub const PAYLOAD_SIZE: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MouseState {
    pub connected: bool,
    pub battery: u8,
    pub is_charging: bool,
    pub is_wireless: bool,
    pub wireless_connected: bool,
    pub mouse_version: String,
    pub dongle_version: String,
    #[allow(dead_code)]
    pub mode_raw: u8,
    pub status_text: String,
}

impl Default for MouseState {
    fn default() -> Self {
        Self {
            connected: false,
            battery: 0,
            is_charging: false,
            is_wireless: true,
            wireless_connected: false,
            mouse_version: "Unknown".to_string(),
            dongle_version: "Unknown".to_string(),
            mode_raw: 0,
            status_text: "Disconnected".to_string(),
        }
    }
}

/// Parses a 64-byte raw HID response payload into a strongly-typed `MouseState`.
pub fn parse_mouse_response(res: &[u8]) -> Option<MouseState> {
    if res.len() < 20 {
        return None;
    }

    // Find response slice starting with CMD_GET_INFO
    let slice = if res[0] == 10 && res[1] == CMD_GET_INFO {
        &res[1..]
    } else if res[0] == CMD_GET_INFO {
        res
    } else {
        let start = res.iter().take(5).position(|&b| b == CMD_GET_INFO);
        start.map(|s| &res[s..])?
    };

    if slice.len() < 20 {
        return None;
    }

    let mode = slice[3];
    let is_wired = matches!(mode, 2 | 3 | 6 | 7);
    let is_wireless = !is_wired;

    let dongle_ver = if slice.len() > 11 {
        format!("{}.{}.{}", slice[11], (slice[10] >> 4) & 15, slice[10] & 15)
    } else {
        "Unknown".to_string()
    };

    let mouse_ver = if slice.len() > 13 {
        format!("{}.{}.{}", slice[13], (slice[12] >> 4) & 15, slice[12] & 15)
    } else {
        "Unknown".to_string()
    };

    let power = if slice.len() > 17 {
        slice[17].min(100)
    } else {
        0
    };
    let wireless_connected = if slice.len() > 19 {
        slice[19] == 1
    } else {
        true
    };

    let is_charging = if slice.len() > 16 {
        slice[16] == 1
    } else {
        false
    };

    let status_text = if is_wireless {
        if wireless_connected {
            if is_charging {
                "Charging (Wireless NearLink)".to_string()
            } else {
                "Connected (Wireless NearLink)".to_string()
            }
        } else {
            "Mouse Sleeping / Offline".to_string()
        }
    } else if is_charging {
        if power >= 100 {
            "Fully Charged (Plugged In)".to_string()
        } else {
            "Charging (Wired USB)".to_string()
        }
    } else {
        "Connected (Wired USB)".to_string()
    };

    Some(MouseState {
        connected: true,
        battery: power,
        is_charging,
        is_wireless,
        wireless_connected,
        mouse_version: mouse_ver,
        dongle_version: dongle_ver,
        mode_raw: mode,
        status_text,
    })
}

pub struct IrisMouseDetector {
    api: Option<hidapi::HidApi>,
    pub last_state: MouseState,
}

impl IrisMouseDetector {
    pub fn new() -> Self {
        Self {
            api: hidapi::HidApi::new().ok(),
            last_state: MouseState::default(),
        }
    }

    pub fn with_initial_battery(initial_battery: u8) -> Self {
        let mut detector = Self::new();
        if initial_battery > 0 {
            detector.last_state.battery = initial_battery;
        }
        detector
    }

    /// Reconciles a parsed response with the previous mouse state.
    /// When the mouse is wireless and sleeping/offline, the dongle cannot communicate
    /// with the mouse over RF. Any battery reading returned by the dongle while disconnected
    /// is inherently stale (e.g. pre-charge levels from before a wired charging session).
    /// If we already have a known valid battery level, preserve it.
    pub fn reconcile_state(&self, mut parsed: MouseState) -> MouseState {
        if parsed.is_wireless && !parsed.wireless_connected {
            if self.last_state.battery > 0 {
                parsed.battery = self.last_state.battery;
            }
            if self.last_state.mouse_version != "Unknown"
                && (parsed.mouse_version == "Unknown" || parsed.mouse_version == "0.0.0")
            {
                parsed.mouse_version = self.last_state.mouse_version.clone();
            }
            parsed.is_charging = false;
        }
        parsed
    }

    fn find_target_device_path(api: &hidapi::HidApi) -> Option<CString> {
        let mut wired_primary = None;
        let mut wired_fallback = None;
        let mut dongle_primary = None;
        let mut dongle_fallback = None;

        for dev in api.device_list() {
            if dev.vendor_id() == VENDOR_ID {
                let is_target_usage = dev.usage_page() == USAGE_PAGE && dev.usage() == USAGE_ID;
                let is_col05 = dev.path().to_string_lossy().contains("Col05");

                if dev.product_id() == PID_WIRED {
                    if is_target_usage && wired_primary.is_none() {
                        wired_primary = Some(dev.path().to_owned());
                    } else if is_col05 && wired_fallback.is_none() {
                        wired_fallback = Some(dev.path().to_owned());
                    }
                } else if dev.product_id() == PID_DONGLE {
                    if is_target_usage && dongle_primary.is_none() {
                        dongle_primary = Some(dev.path().to_owned());
                    } else if is_col05 && dongle_fallback.is_none() {
                        dongle_fallback = Some(dev.path().to_owned());
                    }
                }
            }
        }

        wired_primary
            .or(wired_fallback)
            .or(dongle_primary)
            .or(dongle_fallback)
    }

    pub fn query_state(&mut self) -> MouseState {
        if self.api.is_none() {
            self.api = hidapi::HidApi::new().ok();
        }

        let api = match self.api.as_mut() {
            Some(a) => a,
            None => {
                self.last_state.connected = false;
                self.last_state.status_text = "HID subsystem error".to_string();
                return self.last_state.clone();
            }
        };

        // Refresh HID enumeration so hotplugged wired cables or dongles are detected immediately
        let _ = api.refresh_devices();
        let path = Self::find_target_device_path(api);

        let path = match path {
            Some(p) => p,
            None => {
                let preserved_battery = self.last_state.battery;
                self.last_state = MouseState {
                    connected: false,
                    battery: preserved_battery,
                    status_text: "Dongle/Mouse Disconnected".to_string(),
                    ..Default::default()
                };
                return self.last_state.clone();
            }
        };

        let dev = match api.open_path(&path) {
            Ok(d) => d,
            Err(e) => {
                // Device might have been unplugged; refresh cache immediately so next poll can reconnect
                let _ = api.refresh_devices();
                self.last_state.connected = false;
                self.last_state.status_text = format!("Device open error: {}", e);
                return self.last_state.clone();
            }
        };

        // Build 64-byte command packet
        let mut req = [0u8; PAYLOAD_SIZE];
        req[0] = REPORT_ID_OUT;
        req[1] = CMD_GET_INFO;
        req[2] = 0;
        req[3] = 0;

        if let Err(e) = dev.write(&req) {
            // Write failed; refresh cache immediately so next poll re-enumerates
            let _ = api.refresh_devices();
            self.last_state.connected = false;
            self.last_state.status_text = format!("Write error: {}", e);
            return self.last_state.clone();
        }

        let mut res = [0u8; PAYLOAD_SIZE];
        match dev.read_timeout(&mut res, 600) {
            Ok(n) if n >= 20 => {
                if let Some(parsed) = parse_mouse_response(&res[..n]) {
                    self.last_state = self.reconcile_state(parsed);
                    self.last_state.clone()
                } else {
                    self.last_state.clone()
                }
            }
            Ok(_) => {
                // Device responded but returned 0 or insufficient bytes (timeout while mouse is sleeping or offline)
                let is_wireless = self.last_state.is_wireless;
                self.last_state = MouseState {
                    connected: true,
                    battery: if self.last_state.battery > 0 {
                        self.last_state.battery
                    } else {
                        0
                    },
                    wireless_connected: false,
                    is_charging: false,
                    status_text: if is_wireless {
                        "Mouse Sleeping / Offline".to_string()
                    } else {
                        "Wired USB (No response)".to_string()
                    },
                    ..self.last_state.clone()
                };
                self.last_state.clone()
            }
            Err(e) => {
                // Device I/O failure (unplugged or communication broken); refresh cache immediately
                let _ = api.refresh_devices();
                let preserved_battery = self.last_state.battery;
                self.last_state = MouseState {
                    connected: false,
                    battery: preserved_battery,
                    status_text: format!("Read error: {}", e),
                    ..Default::default()
                };
                self.last_state.clone()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_wireless_active() {
        let mut mock_res = [0u8; 64];
        mock_res[0] = 10;
        mock_res[1] = CMD_GET_INFO;
        mock_res[4] = 0; // wireless mode
        mock_res[17] = 0;
        mock_res[18] = 85; // 85% battery
        mock_res[20] = 1; // wireless connected = true

        let state = parse_mouse_response(&mock_res).expect("should parse valid response");
        assert!(state.connected);
        assert_eq!(state.battery, 85);
        assert!(!state.is_charging);
        assert!(state.is_wireless);
        assert!(state.wireless_connected);
    }

    #[test]
    fn test_parse_wireless_sleeping() {
        let mut mock_res = [0u8; 64];
        mock_res[0] = 10;
        mock_res[1] = CMD_GET_INFO;
        mock_res[4] = 0; // wireless
        mock_res[18] = 42; // 42% battery
        mock_res[20] = 0; // wireless disconnected = sleeping

        let state = parse_mouse_response(&mock_res).expect("should parse valid response");
        assert!(state.connected);
        assert_eq!(state.battery, 42);
        assert!(!state.wireless_connected);
        assert_eq!(state.status_text, "Mouse Sleeping / Offline");
    }

    #[test]
    fn test_parse_charging() {
        let mut mock_res = [0u8; 64];
        mock_res[0] = 10;
        mock_res[1] = CMD_GET_INFO;
        mock_res[4] = 2; // wired mode
        mock_res[17] = 1; // is_charging = 1
        mock_res[18] = 75; // 75%

        let state = parse_mouse_response(&mock_res).expect("should parse valid response");
        assert!(state.connected);
        assert_eq!(state.battery, 75);
        assert!(state.is_charging);
        assert!(!state.is_wireless);
    }

    #[test]
    fn test_parse_charging_at_100() {
        let mut mock_res = [0u8; 64];
        mock_res[0] = 10;
        mock_res[1] = CMD_GET_INFO;
        mock_res[4] = 2; // wired mode
        mock_res[17] = 1; // is_charging = 1
        mock_res[18] = 100; // 100%

        let state = parse_mouse_response(&mock_res).expect("should parse valid response");
        assert!(state.connected);
        assert_eq!(state.battery, 100);
        assert!(state.is_charging);
        assert_eq!(state.status_text, "Fully Charged (Plugged In)");
    }

    #[test]
    fn test_parse_direct_cmd_start() {
        let mut mock_res = [0u8; 64];
        mock_res[0] = CMD_GET_INFO;
        mock_res[3] = 0; // wireless
        mock_res[16] = 0;
        mock_res[17] = 50; // 50%
        mock_res[19] = 1;

        let state =
            parse_mouse_response(&mock_res).expect("should parse response starting with cmd");
        assert!(state.connected);
        assert_eq!(state.battery, 50);
    }

    #[test]
    fn test_parse_invalid_or_short() {
        assert!(parse_mouse_response(&[]).is_none());
        assert!(parse_mouse_response(&[0u8; 10]).is_none());
        assert!(parse_mouse_response(&[0u8; 64]).is_none()); // no CMD_GET_INFO
    }

    #[test]
    fn test_reconcile_state_preserves_battery_after_unplug() {
        let mut detector = IrisMouseDetector::new();
        // Mouse was just fully charged via wired cable
        detector.last_state = MouseState {
            connected: true,
            battery: 100,
            is_charging: true,
            is_wireless: false,
            wireless_connected: true,
            mouse_version: "1.2.3".to_string(),
            dongle_version: "1.0.0".to_string(),
            mode_raw: 2,
            status_text: "Fully Charged (Plugged In)".to_string(),
        };

        // User unplugs cable, mouse wireless is still turned off.
        // Dongle responds with stale 19% and wireless_connected = false.
        let stale_dongle_state = MouseState {
            connected: true,
            battery: 19,
            is_charging: false,
            is_wireless: true,
            wireless_connected: false,
            mouse_version: "Unknown".to_string(),
            dongle_version: "1.0.0".to_string(),
            mode_raw: 0,
            status_text: "Mouse Sleeping / Offline".to_string(),
        };

        let reconciled = detector.reconcile_state(stale_dongle_state);
        assert_eq!(reconciled.battery, 100);
        assert!(!reconciled.is_charging);
        assert_eq!(reconciled.mouse_version, "1.2.3");
        assert!(!reconciled.wireless_connected);
    }

    #[test]
    fn test_reconcile_state_accepts_live_wireless_reading() {
        let detector = IrisMouseDetector::with_initial_battery(100);
        assert_eq!(detector.last_state.battery, 100);

        // When mouse connects wirelessly, live reading must override
        let live_wireless_state = MouseState {
            connected: true,
            battery: 95,
            is_charging: false,
            is_wireless: true,
            wireless_connected: true,
            ..Default::default()
        };

        let reconciled = detector.reconcile_state(live_wireless_state);
        assert_eq!(reconciled.battery, 95);
        assert!(reconciled.wireless_connected);
    }
}
