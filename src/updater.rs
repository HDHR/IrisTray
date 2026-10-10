use std::ffi::c_void;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::ptr;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Networking::WinHttp::{
    WinHttpCloseHandle, WinHttpConnect, WinHttpCrackUrl, WinHttpOpen, WinHttpOpenRequest,
    WinHttpQueryDataAvailable, WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse,
    WinHttpSendRequest, WinHttpSetOption, URL_COMPONENTS, WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
    WINHTTP_FLAG_SECURE, WINHTTP_OPTION_REDIRECT_POLICY, WINHTTP_OPTION_REDIRECT_POLICY_ALWAYS,
    WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE,
};
use windows_sys::Win32::UI::Controls::{
    TaskDialogIndirect, TASKDIALOGCONFIG, TASKDIALOG_BUTTON, TDCBF_CLOSE_BUTTON, TD_ERROR_ICON,
    TD_INFORMATION_ICON,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    MessageBoxW, MB_ICONERROR, MB_ICONINFORMATION, MB_OK,
};

pub const REPO_URL: &str = "https://github.com/HDHR/IrisTray";
pub const RELEASES_URL: &str = "https://github.com/HDHR/IrisTray/releases";
const GITHUB_API_HOST: &str = "api.github.com";
const GITHUB_API_PATH: &str = "/repos/HDHR/IrisTray/releases/latest";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub release_url: String,
    pub exe_download_url: Option<String>,
    pub release_notes: Option<String>,
}

pub fn open_url_in_browser(url: &str) {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let operation: Vec<u16> = "open\0".encode_utf16().collect();
    let url_wide: Vec<u16> = url.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        ShellExecuteW(
            0,
            operation.as_ptr(),
            url_wide.as_ptr(),
            ptr::null(),
            ptr::null(),
            SW_SHOWNORMAL,
        );
    }
}

pub fn parse_semver(v: &str) -> Option<(u64, u64, u64)> {
    let cleaned = v.trim().trim_start_matches('v').trim_start_matches('V');
    let mut parts = cleaned.split('.');
    let major = parts.next()?.parse::<u64>().ok()?;
    let minor = parts.next()?.parse::<u64>().ok()?;
    let patch = parts
        .next()
        .unwrap_or("0")
        .split('-')
        .next()?
        .parse::<u64>()
        .ok()?;
    Some((major, minor, patch))
}

pub fn is_newer_version(latest: &str, current: &str) -> bool {
    match (parse_semver(latest), parse_semver(current)) {
        (Some((l_maj, l_min, l_pat)), Some((c_maj, c_min, c_pat))) => {
            (l_maj, l_min, l_pat) > (c_maj, c_min, c_pat)
        }
        _ => latest.trim() != current.trim(),
    }
}

pub fn check_for_updates() -> Result<UpdateInfo, String> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let body = fetch_github_latest_release_json()?;

    let json: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| format!("Failed to parse release metadata: {e}"))?;

    let tag_name = json["tag_name"]
        .as_str()
        .ok_or_else(|| "Missing tag_name in GitHub release".to_string())?
        .to_string();

    let release_url = json["html_url"]
        .as_str()
        .unwrap_or(RELEASES_URL)
        .to_string();

    let mut exe_download_url = None;
    if let Some(assets) = json["assets"].as_array() {
        for asset in assets {
            if let (Some(name), Some(url)) = (
                asset["name"].as_str(),
                asset["browser_download_url"].as_str(),
            ) {
                if name.eq_ignore_ascii_case("IrisTray.exe") {
                    exe_download_url = Some(url.to_string());
                    break;
                }
            }
        }
    }

    let release_notes = json["body"].as_str().map(|s| s.to_string());
    let has_update = is_newer_version(&tag_name, &current_version);

    Ok(UpdateInfo {
        current_version,
        latest_version: tag_name,
        has_update,
        release_url,
        exe_download_url,
        release_notes,
    })
}

struct HandleGuard(*mut c_void);
impl Drop for HandleGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { WinHttpCloseHandle(self.0) };
        }
    }
}

fn fetch_github_latest_release_json() -> Result<String, String> {
    unsafe {
        let user_agent: Vec<u16> = format!("IrisTray-App/v{}\0", env!("CARGO_PKG_VERSION"))
            .encode_utf16()
            .collect();
        let h_session = WinHttpOpen(
            user_agent.as_ptr(),
            WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
            ptr::null(),
            ptr::null(),
            0,
        );
        if h_session.is_null() {
            return Err("Failed to open Windows HTTP session".to_string());
        }
        let _session_guard = HandleGuard(h_session);

        let host_wide: Vec<u16> = GITHUB_API_HOST
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let h_connect = WinHttpConnect(h_session, host_wide.as_ptr(), 443, 0);
        if h_connect.is_null() {
            return Err("Failed to connect to GitHub API".to_string());
        }
        let _connect_guard = HandleGuard(h_connect);

        let verb_wide: Vec<u16> = "GET\0".encode_utf16().collect();
        let path_wide: Vec<u16> = GITHUB_API_PATH
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let h_request = WinHttpOpenRequest(
            h_connect,
            verb_wide.as_ptr(),
            path_wide.as_ptr(),
            ptr::null(),
            ptr::null(),
            ptr::null(),
            WINHTTP_FLAG_SECURE,
        );
        if h_request.is_null() {
            return Err("Failed to open HTTP request".to_string());
        }
        let _request_guard = HandleGuard(h_request);

        let headers: Vec<u16> = "Accept: application/vnd.github.v3+json\r\n\0"
            .encode_utf16()
            .collect();

        let send_res = WinHttpSendRequest(
            h_request,
            headers.as_ptr(),
            (headers.len() - 1) as u32,
            ptr::null_mut(),
            0,
            0,
            0,
        );
        if send_res == 0 {
            return Err("Failed to send HTTP request to GitHub".to_string());
        }

        if WinHttpReceiveResponse(h_request, ptr::null_mut()) == 0 {
            return Err("Failed to receive response from GitHub".to_string());
        }

        let mut status_code: u32 = 0;
        let mut status_size = std::mem::size_of::<u32>() as u32;
        let query_res = WinHttpQueryHeaders(
            h_request,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            ptr::null(),
            &mut status_code as *mut u32 as *mut _,
            &mut status_size,
            ptr::null_mut(),
        );

        if query_res == 0 || status_code != 200 {
            return Err(format!("GitHub returned HTTP status {status_code}"));
        }

        let mut response_bytes = Vec::new();
        loop {
            let mut bytes_avail: u32 = 0;
            if WinHttpQueryDataAvailable(h_request, &mut bytes_avail) == 0 {
                break;
            }
            if bytes_avail == 0 {
                break;
            }

            let mut chunk = vec![0u8; bytes_avail as usize];
            let mut bytes_read: u32 = 0;
            if WinHttpReadData(
                h_request,
                chunk.as_mut_ptr() as *mut _,
                bytes_avail,
                &mut bytes_read,
            ) == 0
            {
                break;
            }

            chunk.truncate(bytes_read as usize);
            response_bytes.extend_from_slice(&chunk);
        }

        String::from_utf8(response_bytes).map_err(|e| format!("Invalid UTF-8 from GitHub: {e}"))
    }
}

pub fn download_file_to(url: &str, destination: &Path) -> Result<(), String> {
    unsafe {
        let user_agent: Vec<u16> = format!("IrisTray-App/v{}\0", env!("CARGO_PKG_VERSION"))
            .encode_utf16()
            .collect();
        let h_session = WinHttpOpen(
            user_agent.as_ptr(),
            WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
            ptr::null(),
            ptr::null(),
            0,
        );
        if h_session.is_null() {
            return Err("Failed to open Windows HTTP session".to_string());
        }
        let _session_guard = HandleGuard(h_session);

        let mut redirect_policy = WINHTTP_OPTION_REDIRECT_POLICY_ALWAYS;
        WinHttpSetOption(
            h_session,
            WINHTTP_OPTION_REDIRECT_POLICY,
            &mut redirect_policy as *mut _ as *mut c_void,
            std::mem::size_of::<u32>() as u32,
        );

        let url_w: Vec<u16> = url.encode_utf16().chain(std::iter::once(0)).collect();
        let mut comp: URL_COMPONENTS = std::mem::zeroed();
        comp.dwStructSize = std::mem::size_of::<URL_COMPONENTS>() as u32;
        comp.dwHostNameLength = 1;
        comp.dwUrlPathLength = 1;
        comp.dwExtraInfoLength = 1;

        if WinHttpCrackUrl(url_w.as_ptr(), 0, 0, &mut comp) == 0 {
            return Err("Failed to parse download URL".to_string());
        }

        let host_slice =
            std::slice::from_raw_parts(comp.lpszHostName, comp.dwHostNameLength as usize);
        let mut host_w: Vec<u16> = host_slice.to_vec();
        host_w.push(0);

        let mut path_w: Vec<u16> = Vec::new();
        if !comp.lpszUrlPath.is_null() && comp.dwUrlPathLength > 0 {
            let path_slice =
                std::slice::from_raw_parts(comp.lpszUrlPath, comp.dwUrlPathLength as usize);
            path_w.extend_from_slice(path_slice);
        }
        if !comp.lpszExtraInfo.is_null() && comp.dwExtraInfoLength > 0 {
            let extra_slice =
                std::slice::from_raw_parts(comp.lpszExtraInfo, comp.dwExtraInfoLength as usize);
            path_w.extend_from_slice(extra_slice);
        }
        path_w.push(0);

        let is_https = comp.nScheme == 2; // INTERNET_SCHEME_HTTPS
        let flags = if is_https { WINHTTP_FLAG_SECURE } else { 0 };

        let h_connect = WinHttpConnect(h_session, host_w.as_ptr(), comp.nPort, 0);
        if h_connect.is_null() {
            return Err("Failed to connect to download host".to_string());
        }
        let _connect_guard = HandleGuard(h_connect);

        let verb_wide: Vec<u16> = "GET\0".encode_utf16().collect();
        let h_request = WinHttpOpenRequest(
            h_connect,
            verb_wide.as_ptr(),
            path_w.as_ptr(),
            ptr::null(),
            ptr::null(),
            ptr::null(),
            flags,
        );
        if h_request.is_null() {
            return Err("Failed to create download HTTP request".to_string());
        }
        let _request_guard = HandleGuard(h_request);

        let mut req_redirect_policy = WINHTTP_OPTION_REDIRECT_POLICY_ALWAYS;
        WinHttpSetOption(
            h_request,
            WINHTTP_OPTION_REDIRECT_POLICY,
            &mut req_redirect_policy as *mut _ as *mut c_void,
            std::mem::size_of::<u32>() as u32,
        );

        if WinHttpSendRequest(h_request, ptr::null(), 0, ptr::null_mut(), 0, 0, 0) == 0 {
            return Err("Failed to send download request".to_string());
        }

        if WinHttpReceiveResponse(h_request, ptr::null_mut()) == 0 {
            return Err("Failed to receive download response".to_string());
        }

        let mut status_code: u32 = 0;
        let mut status_size = std::mem::size_of::<u32>() as u32;
        let query_res = WinHttpQueryHeaders(
            h_request,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            ptr::null(),
            &mut status_code as *mut u32 as *mut _,
            &mut status_size,
            ptr::null_mut(),
        );

        if query_res == 0 || status_code != 200 {
            return Err(format!("Download failed with HTTP status {status_code}"));
        }

        let mut file = File::create(destination)
            .map_err(|e| format!("Failed to create destination file: {e}"))?;

        loop {
            let mut bytes_avail: u32 = 0;
            if WinHttpQueryDataAvailable(h_request, &mut bytes_avail) == 0 {
                break;
            }
            if bytes_avail == 0 {
                break;
            }

            let mut chunk = vec![0u8; bytes_avail as usize];
            let mut bytes_read: u32 = 0;
            if WinHttpReadData(
                h_request,
                chunk.as_mut_ptr() as *mut _,
                bytes_avail,
                &mut bytes_read,
            ) == 0
            {
                break;
            }

            file.write_all(&chunk[..bytes_read as usize])
                .map_err(|e| format!("Failed to write downloaded bytes: {e}"))?;
        }

        file.flush()
            .map_err(|e| format!("Failed to flush downloaded file: {e}"))?;

        Ok(())
    }
}

pub fn spawn_updater_and_restart(new_exe_path: &Path) -> Result<(), String> {
    use std::env;
    use std::process::Command;

    let current_exe = env::current_exe()
        .map_err(|e| format!("Failed to retrieve current executable path: {e}"))?;
    let pid = std::process::id();

    // PowerShell updater script:
    // 1. Wait for current PID to terminate cleanly.
    // 2. Overwrite current executable with downloaded executable.
    // 3. Remove downloaded temp executable.
    // 4. Start updated application.
    let script = format!(
        "Start-Sleep -Milliseconds 600; Wait-Process -Id {pid} -ErrorAction SilentlyContinue; Move-Item -Path '{new_path}' -Destination '{cur_path}' -Force; Start-Process -FilePath '{cur_path}'",
        pid = pid,
        new_path = new_exe_path.display(),
        cur_path = current_exe.display()
    );

    Command::new("powershell")
        .args(["-WindowStyle", "Hidden", "-Command", &script])
        .spawn()
        .map_err(|e| format!("Failed to launch updater helper: {e}"))?;

    Ok(())
}

pub fn perform_in_app_update(parent: HWND, info: &UpdateInfo) {
    let download_url = match &info.exe_download_url {
        Some(url) => url.as_str(),
        None => {
            open_url_in_browser(&info.release_url);
            return;
        }
    };

    let temp_dir = std::env::temp_dir();
    let temp_new_exe = temp_dir.join(format!("IrisTray-update-{}.exe", info.latest_version));

    // Perform download
    match download_file_to(download_url, &temp_new_exe) {
        Ok(()) => {
            if let Err(e) = spawn_updater_and_restart(&temp_new_exe) {
                let title = to_wide_null("Update Error - IrisTray");
                let msg = to_wide_null(&format!("Failed to apply update automatically:\n{e}\nOpening release page in browser instead."));
                unsafe {
                    MessageBoxW(parent, msg.as_ptr(), title.as_ptr(), MB_ICONERROR | MB_OK);
                }
                open_url_in_browser(&info.release_url);
            } else {
                // Exit current process cleanly so updater script can overwrite binary
                std::process::exit(0);
            }
        }
        Err(e) => {
            let title = to_wide_null("Download Failed - IrisTray");
            let msg = to_wide_null(&format!("Could not download update:\n{e}\nWould you like to open the release page in your browser?"));
            let res = unsafe {
                MessageBoxW(
                    parent,
                    msg.as_ptr(),
                    title.as_ptr(),
                    windows_sys::Win32::UI::WindowsAndMessaging::MB_ICONERROR
                        | windows_sys::Win32::UI::WindowsAndMessaging::MB_YESNO,
                )
            };
            if res == windows_sys::Win32::UI::WindowsAndMessaging::IDYES {
                open_url_in_browser(&info.release_url);
            }
        }
    }
}

pub const BTN_CHECK_UPDATE: i32 = 1001;
pub const BTN_VISIT_REPO: i32 = 1002;

pub enum AboutAction {
    CheckUpdate,
    VisitRepo,
    Close,
}

fn to_wide_null(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn show_about_dialog(parent: HWND, app_version: &str) -> AboutAction {
    let title = to_wide_null(&format!("About IrisTray v{app_version}"));
    let header = to_wide_null(&format!("IrisTray v{app_version}"));
    let content = to_wide_null(
        "Battery monitor for Press Play IRIS v2 NearLink gaming mouse\n\nAuthor: HDHR\nRepository: github.com/HDHR/IrisTray",
    );
    let btn_check_text = to_wide_null("Check for Updates");
    let btn_repo_text = to_wide_null("Visit Repository");

    let buttons = [
        TASKDIALOG_BUTTON {
            nButtonID: BTN_CHECK_UPDATE,
            pszButtonText: btn_check_text.as_ptr(),
        },
        TASKDIALOG_BUTTON {
            nButtonID: BTN_VISIT_REPO,
            pszButtonText: btn_repo_text.as_ptr(),
        },
    ];

    let mut config: TASKDIALOGCONFIG = unsafe { std::mem::zeroed() };
    config.cbSize = std::mem::size_of::<TASKDIALOGCONFIG>() as u32;
    config.hwndParent = parent;
    config.pszWindowTitle = title.as_ptr();
    config.pszMainInstruction = header.as_ptr();
    config.pszContent = content.as_ptr();
    config.Anonymous1.pszMainIcon = TD_INFORMATION_ICON;
    config.dwCommonButtons = TDCBF_CLOSE_BUTTON;
    config.cButtons = buttons.len() as u32;
    config.pButtons = buttons.as_ptr();

    let mut button_pressed = 0;
    let hr = unsafe {
        TaskDialogIndirect(
            &config,
            &mut button_pressed,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };

    if hr == 0 {
        match button_pressed {
            BTN_CHECK_UPDATE => AboutAction::CheckUpdate,
            BTN_VISIT_REPO => AboutAction::VisitRepo,
            _ => AboutAction::Close,
        }
    } else {
        // Fallback to basic MessageBoxW if TaskDialogIndirect fails
        let msg = to_wide_null(&format!(
            "IrisTray v{}\nBattery monitor for Press Play IRIS v2\nAuthor: HDHR\nhttps://github.com/HDHR/IrisTray",
            app_version
        ));
        unsafe {
            MessageBoxW(
                parent,
                msg.as_ptr(),
                title.as_ptr(),
                MB_ICONINFORMATION | MB_OK,
            );
        }
        AboutAction::Close
    }
}

pub fn show_update_result_dialog(parent: HWND, update_info: &Result<UpdateInfo, String>) -> bool {
    match update_info {
        Ok(info) if info.has_update => {
            let title = to_wide_null("Update Available - IrisTray");
            let header = to_wide_null(&format!("IrisTray {} is available!", info.latest_version));
            let body = format!(
                "A new version of IrisTray is available.\n\nCurrent Version: v{}\nLatest Version: {}\n\nWould you like to download and install this update now?",
                info.current_version, info.latest_version
            );
            let content = to_wide_null(&body);
            let btn_update_now = to_wide_null("Update & Restart");
            let btn_view_release = to_wide_null("View Release Notes");

            let buttons = [
                TASKDIALOG_BUTTON {
                    nButtonID: 2001,
                    pszButtonText: btn_update_now.as_ptr(),
                },
                TASKDIALOG_BUTTON {
                    nButtonID: 2002,
                    pszButtonText: btn_view_release.as_ptr(),
                },
            ];

            let mut config: TASKDIALOGCONFIG = unsafe { std::mem::zeroed() };
            config.cbSize = std::mem::size_of::<TASKDIALOGCONFIG>() as u32;
            config.hwndParent = parent;
            config.pszWindowTitle = title.as_ptr();
            config.pszMainInstruction = header.as_ptr();
            config.pszContent = content.as_ptr();
            config.Anonymous1.pszMainIcon = TD_INFORMATION_ICON;
            config.dwCommonButtons = TDCBF_CLOSE_BUTTON;
            config.cButtons = buttons.len() as u32;
            config.pButtons = buttons.as_ptr();

            let mut button_pressed = 0;
            let hr = unsafe {
                TaskDialogIndirect(
                    &config,
                    &mut button_pressed,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            };

            if hr == 0 {
                match button_pressed {
                    2001 => {
                        perform_in_app_update(parent, info);
                        true
                    }
                    2002 => {
                        open_url_in_browser(&info.release_url);
                        true
                    }
                    _ => false,
                }
            } else {
                false
            }
        }
        Ok(info) => {
            let title = to_wide_null("Check for Updates - IrisTray");
            let header = to_wide_null("You are up to date!");
            let content = to_wide_null(&format!(
                "IrisTray v{} is the latest version available.",
                info.current_version
            ));

            let mut config: TASKDIALOGCONFIG = unsafe { std::mem::zeroed() };
            config.cbSize = std::mem::size_of::<TASKDIALOGCONFIG>() as u32;
            config.hwndParent = parent;
            config.pszWindowTitle = title.as_ptr();
            config.pszMainInstruction = header.as_ptr();
            config.pszContent = content.as_ptr();
            config.Anonymous1.pszMainIcon = TD_INFORMATION_ICON;
            config.dwCommonButtons = TDCBF_CLOSE_BUTTON;

            let mut button_pressed = 0;
            let hr = unsafe {
                TaskDialogIndirect(
                    &config,
                    &mut button_pressed,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            };

            if hr != 0 {
                let msg = to_wide_null(&format!(
                    "IrisTray v{} is currently up to date.",
                    info.current_version
                ));
                unsafe {
                    MessageBoxW(
                        parent,
                        msg.as_ptr(),
                        title.as_ptr(),
                        MB_ICONINFORMATION | MB_OK,
                    );
                }
            }
            false
        }
        Err(err) => {
            let title = to_wide_null("Update Check Failed - IrisTray");
            let header = to_wide_null("Unable to check for updates");
            let content = to_wide_null(&format!("Could not connect to GitHub releases:\n\n{err}"));

            let mut config: TASKDIALOGCONFIG = unsafe { std::mem::zeroed() };
            config.cbSize = std::mem::size_of::<TASKDIALOGCONFIG>() as u32;
            config.hwndParent = parent;
            config.pszWindowTitle = title.as_ptr();
            config.pszMainInstruction = header.as_ptr();
            config.pszContent = content.as_ptr();
            config.Anonymous1.pszMainIcon = TD_ERROR_ICON;
            config.dwCommonButtons = TDCBF_CLOSE_BUTTON;

            let mut button_pressed = 0;
            let hr = unsafe {
                TaskDialogIndirect(
                    &config,
                    &mut button_pressed,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            };

            if hr != 0 {
                let msg = to_wide_null(&format!("Unable to check for updates:\n{err}"));
                unsafe {
                    MessageBoxW(parent, msg.as_ptr(), title.as_ptr(), MB_ICONERROR | MB_OK);
                }
            }
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_semver() {
        assert_eq!(parse_semver("1.0.0"), Some((1, 0, 0)));
        assert_eq!(parse_semver("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_semver("V2.0"), Some((2, 0, 0)));
        assert_eq!(parse_semver("v1.0.1-beta.1"), Some((1, 0, 1)));
    }

    #[test]
    fn test_is_newer_version() {
        assert!(is_newer_version("v1.0.1", "1.0.0"));
        assert!(is_newer_version("v1.1.0", "1.0.9"));
        assert!(is_newer_version("v2.0.0", "1.99.99"));
        assert!(!is_newer_version("v1.0.0", "1.0.0"));
        assert!(!is_newer_version("v0.9.9", "1.0.0"));
    }
}
