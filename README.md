# IrisTray (Press Play IRIS v2 Battery Tracker)

> **Disclaimer**: This is an independent, unofficial personal open-source project. It is **not** created, maintained, or endorsed by Press Play.

A lightweight Windows system tray utility for monitoring battery and connection status on the Press Play IRIS v2 NearLink gaming mouse. Built in Rust with zero runtime dependencies.

## Features

- **System Tray Icon**: Displays the current battery percentage as a dynamic, color-coded number rendered directly in memory without external image assets.
  - **Green**: Battery above 40%
  - **Amber**: Battery between 21% and 40%
  - **Red**: Battery at or below 20%
  - **Cyan**: Mouse is charging
  - **"X"**: Mouse or dongle disconnected
  - **"Z"**: Mouse in sleep/hibernation mode
  - **High-DPI Support**: Uses Per-Monitor V2 DPI awareness with automatic display scaling tracking across 100%, 125%, 150%, and 200% displays.
- **Status Tooltip**: Hovering over the icon displays battery percentage, connection type (NearLink Wireless or Wired USB), and charging state.
- **Context Menu**:
  - Device overview: Battery level, charging state, connection mode, and firmware versions for mouse and dongle.
  - Manual refresh.
  - Configurable polling interval: 15 seconds, 30 seconds (default), 1 minute, 2 minutes, or 5 minutes.
  - Optional Windows notifications: Low battery (configurable threshold, default 20%), critical battery (default 10%), charging state changes, and full charge (100%).
  - Windows autostart toggle: Configures user startup without requiring administrator privileges.

## Installation and Usage

### Pre-built Binaries

1. Download the latest release from the [GitHub Releases](https://github.com/HDHR/IrisTray/releases) page:
   - `IrisTray.exe`: Standalone binary with embedded icon and metadata.
   - `IrisTray-v<version>-windows-x64.zip`: Zip archive containing the executable, `config.example.json`, README, and LICENSE.
2. Run `IrisTray.exe`. The application minimizes to the notification area (system tray).
3. Right-click the tray icon to access settings, refresh status, or enable autostart.

### Windows SmartScreen & Security Verification

Because IrisTray is an independent, non-commercial open-source project without a costly enterprise Authenticode code-signing certificate, Windows Defender SmartScreen may display an alert on first launch (*"Windows protected your PC. Microsoft Defender SmartScreen prevented an unrecognized app from starting"*).

To launch IrisTray:
1. Click **More info**.
2. Click **Run anyway**.

#### Verifying Checksums

Every release includes a cryptographically signed `checksums.txt` file containing the SHA-256 hashes of the release files. You can verify the integrity of your download in PowerShell:

```powershell
Get-FileHash .\IrisTray.exe -Algorithm SHA256
```

Compare the resulting hash against the entry in `checksums.txt`.

### Building from Source

Prerequisites:
- Windows 10 (version 1809 or newer, 64-bit) or Windows 11
- Rust toolchain (1.78 or newer)

Steps:
1. Clone the repository:
   ```sh
   git clone https://github.com/HDHR/IrisTray.git
   cd IrisTray
   ```
2. Build the release binary:
   ```sh
   cargo build --release --locked
   ```
   The compiled executable is placed in `target/release/IrisTray.exe`.

## Configuration

Settings are saved in JSON format:
- **Standard location**: `%APPDATA%\IrisTray\config.json`
- **Portable mode**: If a `config.json` file exists in the same folder as `IrisTray.exe`, the application reads from and writes to that local file instead. A template `config.example.json` is provided in the repository and release archives.

Available options:
- `polling_interval_sec`: Refresh interval in seconds (minimum 5, default 30).
- `notify_low_battery`: Enable or disable low battery notifications (default `true`).
- `low_battery_threshold`: Percentage threshold for low battery alert (default `20`).
- `notify_critical`: Enable or disable critical battery notifications (default `true`).
- `critical_threshold`: Percentage threshold for critical battery alert (default `10`).
- `notify_charging`: Notify on charger connect/disconnect (default `true`).
- `notify_fully_charged`: Notify when battery reaches 100% while charging (default `true`).
- `start_with_windows`: Launch on user logon via `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.

## Privacy & Security

- **100% Offline**: IrisTray has **zero** network dependencies, makes no HTTP/TCP/UDP requests, and includes no analytics, telemetry, crash reporting, or auto-updaters.
- **Local Storage Only**: The application only stores user configuration preferences in `%APPDATA%\IrisTray\config.json` (or locally in portable mode). No personal data or mouse activity logs are recorded.

## Hardware Protocol Reference

The application interfaces directly with the device over HID:

- **Vendor ID (VID)**: `0x363C`
- **Product IDs (PID)**:
  - `0xEC06`: NearLink 2.4GHz Wireless Dongle
  - `0xEC05`: Direct Wired USB
- **HID Collection**: Usage Page `0xFFA0`, Usage `0x0002`
- **Communication**:
  - Request: 64-byte packet containing Report ID `11` (`0x0B`) and Command `19` (`0x13`).
  - Response: 64-byte payload containing battery percentage (byte 17), charging state (byte 16), wireless link state (byte 19), and firmware revisions (bytes 10–13).

## Troubleshooting & FAQ

- **The tray icon displays "Z" (Sleeping / Offline)**:
  - When the mouse is connected via the 2.4GHz NearLink wireless dongle, it enters low-power sleep mode after several minutes of inactivity. The dongle remains plugged in, but the mouse RF transmitter sleeps. Moving or clicking the mouse instantly wakes it up, and IrisTray will refresh the battery percentage on the next polling interval (or immediately if you click "Refresh Battery Now").
- **The tray icon displays "X" (Disconnected)**:
  - Verify that the NearLink wireless dongle or USB-C cable is firmly plugged into a functional USB port.
  - If using a USB hub, ensure the hub provides adequate power to the dongle.
- **Can I run this alongside official Press Play configuration software?**:
  - Yes. IrisTray communicates using non-exclusive HID feature reports. You can configure DPI stages, LOD, etc in the official software and run IrisTray simultaneously for lightweight battery monitoring.

## Limitations and Disclaimer

- **Windows only**: Relies on Win32 APIs (`windows-sys`) for system tray integration and DPI scaling. Requires Windows 10 (1809+) or Windows 11.
- **Read-only battery monitoring**: This utility monitors status only. Sensor settings (DPI stages, polling rate, debounce time, LOD) must be configured using the official manufacturer software to save them to the mouse's onboard memory.
- **Trademark Disclaimer**: Press Play is a trademark of its respective owner. Press Play IRIS v2 Battery Tracker is an independent open-source utility and is not affiliated with, endorsed by, or associated with Press Play.

## License

This project is licensed under the [MIT License](LICENSE).
