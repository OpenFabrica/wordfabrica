//! Windows only: embed the app icon and version info (VERSIONINFO) into `wordfabrica.exe`, so it
//! shows in Explorer, the taskbar, the Start menu and Alt-Tab.
//!
//! On every other target this does nothing. A missing resource compiler is a warning, so a
//! cross-compile from macOS or Linux still links, unless `WORDFABRICA_REQUIRE_WINRES=1` turns it
//! into an error (for release builds).

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../assets/app-icon/wordfabrica.ico");
    println!("cargo:rerun-if-env-changed=WORDFABRICA_REQUIRE_WINRES");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../assets/app-icon/wordfabrica.ico")
        .set("ProductName", "WordFabrica")
        .set("FileDescription", "WordFabrica word processor")
        .set("CompanyName", "OpenFabrica")
        .set("LegalCopyright", "Copyright (c) the WordFabrica authors. MIT OR Apache-2.0.")
        .set("OriginalFilename", "wordfabrica.exe")
        .set("InternalName", "wordfabrica");
    if let Err(e) = res.compile() {
        if std::env::var_os("WORDFABRICA_REQUIRE_WINRES").is_some() {
            eprintln!("embedding Windows resources failed: {e}");
            std::process::exit(1);
        }
        println!("cargo:warning=wordfabrica.exe built without icon/version resources: {e}");
    }
}
