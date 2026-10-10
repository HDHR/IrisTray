fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let pkg_version = env!("CARGO_PKG_VERSION");
        let app_title = format!("IrisTray v{pkg_version}");

        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("FileDescription", &app_title);
        res.set("ProductName", &app_title);
        res.set("OriginalFilename", "IrisTray.exe");
        res.set("LegalCopyright", "Copyright (c) 2026 IrisTray Contributors");
        res.set("ProductVersion", pkg_version);
        res.set("FileVersion", pkg_version);

        let parts: Vec<u64> = pkg_version
            .split('.')
            .filter_map(|s| s.parse().ok())
            .collect();
        let major = parts.first().copied().unwrap_or(1);
        let minor = parts.get(1).copied().unwrap_or(0);
        let patch = parts.get(2).copied().unwrap_or(0);
        let version_u64 = (major << 48) | (minor << 32) | (patch << 16);

        res.set_version_info(winresource::VersionInfo::PRODUCTVERSION, version_u64);
        res.set_version_info(winresource::VersionInfo::FILEVERSION, version_u64);
        res.set_manifest_file("IrisTray.exe.manifest");
        if let Err(e) = res.compile() {
            eprintln!("cargo:warning=Failed to compile Windows resource: {e}");
        }
    }
}
