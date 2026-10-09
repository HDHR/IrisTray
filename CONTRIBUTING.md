# Contributing to IRIS v2 Battery Tracker

Thank you for your interest in contributing to the Press Play IRIS v2 Battery Tracker!

## Development Guidelines

1. **Prerequisites**:
   - Windows 10 (1809+) or Windows 11
   - Rust toolchain (1.78 or newer)
   - Git

2. **Building and Testing**:
   - Format check: `cargo fmt --all -- --check`
   - Lint check: `cargo clippy --all-targets --locked -- -D warnings`
   - Run tests: `cargo test --locked`
   - Build debug: `cargo build`
   - Build release: `cargo build --release`

3. **Code Style**:
   - Run `cargo fmt --all` before submitting any pull request.
   - Ensure all tests pass and `cargo clippy --all-targets --locked -- -D warnings` reports zero warnings.
   - Keep unsafe Win32 blocks localized and documented.
   - Never block the UI thread on I/O or USB HID queries.

4. **Pull Requests**:
   - Describe the purpose and context of the changes.
   - Mention any device testing done with hardware (Dongle 2.4GHz / Wired USB).
