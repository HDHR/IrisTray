fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("FileDescription", "IRIS v2 NearLink Battery Tracker");
        res.set("ProductName", "IRIS v2 Battery Tracker");
        res.set("OriginalFilename", "IrisTray.exe");
        res.set("LegalCopyright", "Copyright (c) 2026 IrisTray Contributors");
        res.set("ProductVersion", "1.0.0.0");
        res.set("FileVersion", "1.0.0.0");
        res.set_version_info(
            winresource::VersionInfo::PRODUCTVERSION,
            0x0001_0000_0000_0000,
        );
        res.set_version_info(winresource::VersionInfo::FILEVERSION, 0x0001_0000_0000_0000);
        if let Err(e) = res.compile() {
            eprintln!("cargo:warning=Failed to compile Windows resource: {e}");
        }
    }
}
