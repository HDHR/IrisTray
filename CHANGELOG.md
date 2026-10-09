# Changelog

All notable changes to IrisTray will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
