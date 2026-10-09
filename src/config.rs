use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use windows_sys::Win32::Storage::FileSystem::{
    MoveFileExW, ReplaceFileW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
};
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
    HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_SZ,
};

const RUN_REG_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const APP_REG_NAME: &str = "IRIS_V2_Tray";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub polling_interval_sec: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_style: Option<String>,
    pub notify_low_battery: bool,
    pub low_battery_threshold: u8,
    pub notify_critical: bool,
    pub critical_threshold: u8,
    pub notify_charging: bool,
    pub notify_fully_charged: bool,
    pub start_with_windows: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_known_battery: Option<u8>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            polling_interval_sec: 30,
            icon_style: Some("badge".to_string()),
            notify_low_battery: true,
            low_battery_threshold: 20,
            notify_critical: true,
            critical_threshold: 10,
            notify_charging: true,
            notify_fully_charged: true,
            start_with_windows: false,
            last_known_battery: None,
        }
    }
}

impl AppConfig {
    /// Sanitizes configuration values to prevent invalid states or divide-by-zero/overflows.
    pub fn sanitize(&mut self) {
        self.polling_interval_sec = self.polling_interval_sec.clamp(5, 3600);
        self.low_battery_threshold = self.low_battery_threshold.clamp(5, 50);
        let max_crit = self.low_battery_threshold.saturating_sub(1).max(1);
        self.critical_threshold = self.critical_threshold.clamp(1, max_crit);
        if let Some(bat) = self.last_known_battery {
            self.last_known_battery = Some(bat.min(100));
        }
    }
}

fn to_wide_path(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn atomic_write_file(target_path: &Path, content: &[u8]) -> std::io::Result<()> {
    let tmp_path = target_path.with_extension("tmp");
    fs::write(&tmp_path, content)?;

    let target_wide = to_wide_path(target_path);
    let tmp_wide = to_wide_path(&tmp_path);

    let res = unsafe {
        // If target file already exists, use ReplaceFileW for a true atomic swap without deletion
        if target_path.exists() {
            let replaced = ReplaceFileW(
                target_wide.as_ptr(),
                tmp_wide.as_ptr(),
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            if replaced != 0 {
                return Ok(());
            }
        }

        // If target file does not exist yet or ReplaceFileW failed, perform an atomic move with replace
        let moved = MoveFileExW(
            tmp_wide.as_ptr(),
            target_wide.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        );
        if moved != 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    };

    // Always ensure temporary file is removed if it still exists
    let _ = fs::remove_file(&tmp_path);
    res
}

pub struct ConfigManager {
    pub config: AppConfig,
    pub config_path: PathBuf,
}

impl ConfigManager {
    pub fn new() -> Self {
        let config_path = Self::determine_config_path();
        let mut cm = Self {
            config: AppConfig::default(),
            config_path,
        };
        cm.load();

        let actual_startup = cm.is_registered_startup();
        if cm.config.start_with_windows != actual_startup {
            cm.config.start_with_windows = actual_startup;
        }

        cm
    }

    fn determine_config_path() -> PathBuf {
        let exe_dir = env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));

        let portable_config = exe_dir.join("config.json");
        if portable_config.exists() {
            // Verify directory is writable (prevents silent write failure in C:\Program Files)
            let test_file = exe_dir.join(".iris_write_test");
            if fs::write(&test_file, b"ok").is_ok() {
                let _ = fs::remove_file(&test_file);
                return portable_config;
            }
        }

        if let Ok(appdata) = env::var("APPDATA") {
            let app_dir = PathBuf::from(appdata).join("IrisTray");
            let _ = fs::create_dir_all(&app_dir);
            return app_dir.join("config.json");
        }

        portable_config
    }

    pub fn load(&mut self) {
        let mut loaded_config = None;

        if self.config_path.exists() {
            match fs::read_to_string(&self.config_path) {
                Ok(content) => match serde_json::from_str::<AppConfig>(&content) {
                    Ok(mut loaded) => {
                        loaded.sanitize();
                        loaded_config = Some(loaded);
                    }
                    Err(e) => {
                        eprintln!(
                            "Warning: failed to parse config at {:?}: {e}",
                            self.config_path
                        );
                        // Preserve the unparseable configuration rather than silently overwriting it
                        let bak_path = self.config_path.with_extension("json.bak");
                        let _ = fs::copy(&self.config_path, &bak_path);
                    }
                },
                Err(e) => {
                    eprintln!(
                        "Warning: failed to read config file at {:?}: {e}",
                        self.config_path
                    );
                }
            }
        }

        // Fallback: if main config failed or was empty, check for config.tmp
        if loaded_config.is_none() {
            let tmp_path = self.config_path.with_extension("tmp");
            if tmp_path.exists() {
                if let Ok(content) = fs::read_to_string(&tmp_path) {
                    if let Ok(mut loaded) = serde_json::from_str::<AppConfig>(&content) {
                        loaded.sanitize();
                        loaded_config = Some(loaded);
                    }
                }
            }
        }

        if let Some(cfg) = loaded_config {
            self.config = cfg;
        }
    }

    pub fn save(&mut self) {
        if let Some(parent) = self.config_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(json_str) = serde_json::to_string_pretty(&self.config) {
            if atomic_write_file(&self.config_path, json_str.as_bytes()).is_err() {
                // Fallback to APPDATA if writing to portable path failed due to permissions
                if let Ok(appdata) = env::var("APPDATA") {
                    let app_dir = PathBuf::from(appdata).join("IrisTray");
                    let _ = fs::create_dir_all(&app_dir);
                    let fallback_path = app_dir.join("config.json");
                    if atomic_write_file(&fallback_path, json_str.as_bytes()).is_ok() {
                        self.config_path = fallback_path;
                    }
                }
            }
        }
    }

    pub fn is_registered_startup(&self) -> bool {
        let exe_path = match env::current_exe() {
            Ok(p) => p.to_string_lossy().to_string(),
            Err(_) => return false,
        };
        let normalized_exe = exe_path.trim_matches('"').replace('/', "\\").to_lowercase();

        unsafe {
            let key_wide: Vec<u16> = RUN_REG_KEY
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let mut hkey = 0;
            if RegOpenKeyExW(HKEY_CURRENT_USER, key_wide.as_ptr(), 0, KEY_READ, &mut hkey) == 0 {
                let name_wide: Vec<u16> = APP_REG_NAME
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .collect();
                let mut typ = 0;
                let mut buf = [0u16; 512];
                let mut size = (buf.len() * std::mem::size_of::<u16>()) as u32;
                let status = RegQueryValueExW(
                    hkey,
                    name_wide.as_ptr(),
                    std::ptr::null_mut(),
                    &mut typ,
                    buf.as_mut_ptr() as *mut u8,
                    &mut size,
                );
                RegCloseKey(hkey);

                if status == 0 && typ == REG_SZ {
                    let raw_len = size as usize / std::mem::size_of::<u16>();
                    // Strip trailing null characters
                    let len = if raw_len > 0 && buf[raw_len - 1] == 0 {
                        raw_len - 1
                    } else {
                        raw_len
                    };
                    let reg_val = String::from_utf16_lossy(&buf[..len]);
                    let normalized_reg =
                        reg_val.trim_matches('"').replace('/', "\\").to_lowercase();
                    return normalized_reg == normalized_exe;
                }
            }
            false
        }
    }

    pub fn set_startup(&mut self, enable: bool) -> bool {
        unsafe {
            let key_wide: Vec<u16> = RUN_REG_KEY
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let mut hkey = 0;
            if RegOpenKeyExW(
                HKEY_CURRENT_USER,
                key_wide.as_ptr(),
                0,
                KEY_SET_VALUE,
                &mut hkey,
            ) != 0
            {
                return false;
            }

            let name_wide: Vec<u16> = APP_REG_NAME
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let mut success = false;

            if enable {
                if let Ok(exe_path) = env::current_exe() {
                    let cmd = format!("\"{}\"", exe_path.to_string_lossy());
                    let val_wide: Vec<u16> = cmd.encode_utf16().chain(std::iter::once(0)).collect();
                    let bytes_len = (val_wide.len() * std::mem::size_of::<u16>()) as u32;

                    let status = RegSetValueExW(
                        hkey,
                        name_wide.as_ptr(),
                        0,
                        REG_SZ,
                        val_wide.as_ptr() as *const u8,
                        bytes_len,
                    );
                    if status == 0 {
                        self.config.start_with_windows = true;
                        self.save();
                        success = true;
                    }
                }
            } else {
                let status = RegDeleteValueW(hkey, name_wide.as_ptr());
                // 0 is ERROR_SUCCESS, 2 is ERROR_FILE_NOT_FOUND (value already absent)
                if status == 0 || status == 2 {
                    self.config.start_with_windows = false;
                    self.save();
                    success = true;
                }
            }

            RegCloseKey(hkey);
            success
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_sanitize() {
        let mut cfg = AppConfig {
            polling_interval_sec: 1, // too low
            icon_style: Some("badge".to_string()),
            notify_low_battery: true,
            low_battery_threshold: 80, // too high
            notify_critical: true,
            critical_threshold: 95, // higher than low
            notify_charging: true,
            notify_fully_charged: true,
            start_with_windows: false,
            last_known_battery: None,
        };
        cfg.sanitize();

        assert_eq!(cfg.polling_interval_sec, 5);
        assert_eq!(cfg.low_battery_threshold, 50);
        assert!(cfg.critical_threshold < cfg.low_battery_threshold);
        assert_eq!(cfg.critical_threshold, 49);
    }

    #[test]
    fn test_config_icon_style_preservation() {
        let json_data = r#"{"polling_interval_sec":60,"icon_style":"custom","notify_low_battery":true,"low_battery_threshold":20,"notify_critical":true,"critical_threshold":10,"notify_charging":true,"notify_fully_charged":true,"start_with_windows":false}"#;
        let parsed: AppConfig = serde_json::from_str(json_data).unwrap();
        assert_eq!(parsed.icon_style.as_deref(), Some("custom"));

        let serialized = serde_json::to_string(&parsed).unwrap();
        assert!(serialized.contains(r#""icon_style":"custom""#));
    }

    #[test]
    fn test_config_default_on_missing_fields() {
        // Empty JSON should parse to default without failing
        let empty_json = "{}";
        let parsed: AppConfig =
            serde_json::from_str(empty_json).expect("should parse empty json with defaults");
        assert_eq!(parsed.polling_interval_sec, 30);
        assert!(parsed.notify_low_battery);
        assert_eq!(parsed.low_battery_threshold, 20);

        // Partial JSON should parse specified fields and keep defaults for omitted ones
        let partial_json = r#"{"polling_interval_sec": 120}"#;
        let partial: AppConfig =
            serde_json::from_str(partial_json).expect("should parse partial json");
        assert_eq!(partial.polling_interval_sec, 120);
        assert_eq!(partial.low_battery_threshold, 20);
    }
}
