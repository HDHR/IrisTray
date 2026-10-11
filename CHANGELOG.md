# Changelog

All notable changes to IrisTray will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.1.3] - 2026-10-11

### Added
- Adaptive fast-polling on mouse sleep and startup: automatically checks every 3-5 seconds after startup or sleep, smoothly ramping back up and capping at the user's configured polling interval (e.g. 5 minutes). Mouse wake-ups are now detected almost immediately without waiting for a long polling cycle.

## [1.1.2] - 2026-10-10

### Fixed
- Fixed in-app self-updater not overwriting the existing binary due to Windows PowerShell 5.1 Move-Item destination collision error.
- Fixed console window momentarily flashing during self-update by attaching CREATE_NO_WINDOW flag.
- Added retry loop and process exit synchronization to safely wait for Windows file locks before replacing IrisTray.exe.

### Changed
- Updated the Windows application header and notification banner to display IrisTray vX.X.X dynamically based on the package release version.

## [1.1.1] - 2026-10-10

### Fixed
- Fixed TaskDialogIndirect entry point not found crash on startup by dynamically loading comctl32.dll and embedding the Microsoft Common-Controls v6 application manifest.

## [1.1.0] - 2026-10-10

### Added
- Native modern About dialog (TaskDialogIndirect) displaying version, author, and quick action buttons.
- Direct repository link action button opening GitHub in the default browser.
- In-app update checker querying GitHub Releases via native Windows HTTP (WinHttp).
- In-app update download and silent binary replacement with automatic restart.
- Silent background update check on application startup.
- Windows autostart path auto-healing (relocating IrisTray.exe automatically heals the Windows Run registry key).

### Changed
- Refactored tray menu bottom section with clean About and Check for Updates entries.
- Updated documentation and privacy disclosures regarding update checks.
## [1.0.0] - 2026-10-10

### Added
- Initial public release of IrisTray (IRIS v2 Battery Tracker).
- Real-time battery status monitoring in Windows system tray for Press Play IRIS v2 NearLink gaming mouse.
- Dual connection support: 2.4GHz NearLink Wireless Dongle (PID `0xEC06`) and Direct Wired USB (PID `0xEC05`).
- Dynamic in-memory tray icon rendering with custom font masks and Per-Monitor V2 DPI scaling (100%, 125%, 150%, 200%).
- Configurable polling intervals (15s, 30s default, 1m, 2m, 5m).
- Customizable Windows balloon/toast notifications: Low Battery, Critical Battery, Charging, Discharging, and Full Charge (100%).
- Sleep / hibernation detection when mouse is wireless and inactive.
- Windows logon autostart toggle via `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
- Native Windows 10/11 dark mode context menu support via `uxtheme.dll`.
- Portable mode support: auto-detects `config.json` alongside the executable before falling back to `%APPDATA%\IrisTray\config.json`.
- Explorer crash recovery via `TaskbarCreated` message listener.
- Single instance guard via named Win32 mutex.
