//! Embed the font manifest in resources/fonts by default. CRAFT_FONTS_DIR can override
//! the resource root (it must contain fonts/manifest.txt). No separate font project is needed.
//! Web builds embed only the regular Chinese and Japanese UI faces; additional document
//! fonts can be served at runtime. Missing built-in resources fail the build.
use std::fmt::Write as _;
use std::path::PathBuf;

fn main() {
    println!("cargo::rerun-if-env-changed=CRAFT_FONTS_DIR");
    println!("cargo::rerun-if-env-changed=CRAFT_FONTS_REQUIRED");
    let mut src = String::from("pub static CRAFT_FONTS: &[CraftFont] = &[\n");
    let override_dir = std::env::var_os("CRAFT_FONTS_DIR").map(PathBuf::from);
    let required = override_dir.is_none() || std::env::var_os("CRAFT_FONTS_REQUIRED").is_some();
    let dir = override_dir.unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources"));
    match craft_fonts(&dir) {
        Ok(entries) => src.push_str(&entries),
        Err(e) if required => {
            println!("cargo::error=CRAFT_FONTS_DIR={}: {e}", dir.display());
        }
        Err(e) => println!("cargo::warning=building without optional fonts: CRAFT_FONTS_DIR={}: {e}", dir.display()),
    }
    src.push_str("];\n");
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap_or_default()).join("craft_fonts.rs");
    if let Err(e) = std::fs::write(&out, src) {
        println!("cargo::error=writing {}: {e}", out.display());
    }
}

/// One `CraftFont { .. }` initialiser per manifest line.
fn craft_fonts(dir: &std::path::Path) -> Result<String, String> {
    let manifest = dir.join("fonts/manifest.txt");
    println!("cargo::rerun-if-changed={}", manifest.display());
    let text = std::fs::read_to_string(&manifest).map_err(|e| format!("{}: {e}", manifest.display()))?;
    let wasm = std::env::var("CARGO_CFG_TARGET_ARCH").is_ok_and(|a| a == "wasm32");
    let mut out = String::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let f: Vec<&str> = line.split(" | ").map(str::trim).collect();
        let [family, style, file, scripts, ..] = f.as_slice() else {
            return Err(format!("malformed manifest line: {line}"));
        };
        let web_ui = *style == "Regular" && (*family == "BIZ UDPGothic" || scripts.split(',').any(|s| s.trim() == "Hans"));
        if wasm && !web_ui {
            continue;
        }
        let path = dir.join(file).canonicalize().map_err(|e| format!("{file}: {e}"))?;
        println!("cargo::rerun-if-changed={}", path.display());
        let scripts: Vec<String> = scripts.split(',').map(|s| format!("{:?}", s.trim())).collect();
        let _ = writeln!(
            out,
            "    CraftFont {{ family: {family:?}, style: {style:?}, scripts: &[{}], bytes: include_bytes!({:?}) }},",
            scripts.join(", "),
            path.display().to_string(),
        );
    }
    Ok(out)
}
