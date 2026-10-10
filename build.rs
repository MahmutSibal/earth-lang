//! Embeds the icon, version information and manifest into `earth.exe` when
//! building for Windows. Other targets are untouched. If no resource compiler
//! is available the build still succeeds, just without these extras.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=windows/earth.ico");
    println!("cargo:rerun-if-changed=windows/earth.manifest");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo sets it"));
    let script = out_dir.join("earth.rc");
    fs::write(&script, resource_script(&root)).expect("could not write earth.rc");

    let compiled = match env::var("CARGO_CFG_TARGET_ENV").as_deref() {
        Ok("gnu") => compile_with_windres(&script, &out_dir),
        Ok("msvc") => compile_with_rc(&script, &out_dir),
        _ => None,
    };
    match compiled {
        Some(resource) => println!("cargo:rustc-link-arg-bins={}", resource.display()),
        None => println!(
            "cargo:warning=No Windows resource compiler found; earth.exe will have no icon or version information"
        ),
    }
}

fn resource_script(root: &Path) -> String {
    let version = env::var("CARGO_PKG_VERSION").expect("cargo sets the version");
    let mut parts: Vec<u16> = version
        .split(['.', '-'])
        .take(3)
        .map(|part| part.parse().unwrap_or(0))
        .collect();
    parts.resize(3, 0);
    let numeric = format!("{},{},{},0", parts[0], parts[1], parts[2]);
    let path = |name: &str| {
        root.join("windows")
            .join(name)
            .display()
            .to_string()
            .replace('\\', "/")
    };

    // Constants are spelled out because windres has no <winver.h> by default.
    format!(
        r#"1 ICON "{icon}"
1 24 "{manifest}"
1 VERSIONINFO
FILEVERSION {numeric}
PRODUCTVERSION {numeric}
FILEFLAGSMASK 0x3f
FILEFLAGS 0x0
FILEOS 0x40004
FILETYPE 0x1
FILESUBTYPE 0x0
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "041f04b0"
    BEGIN
      VALUE "CompanyName", "Mahmut Sibal"
      VALUE "FileDescription", "Earth programlama dili"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "earth"
      VALUE "LegalCopyright", "Copyright (c) 2026 Mahmut Sibal, MIT License"
      VALUE "OriginalFilename", "earth.exe"
      VALUE "ProductName", "Earth"
      VALUE "ProductVersion", "{version}"
      VALUE "Comments", "https://github.com/MahmutSibal/earth-lang"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x041f, 1200
  END
END
"#,
        icon = path("earth.ico"),
        manifest = path("earth.manifest"),
    )
}

fn compile_with_windres(script: &Path, out_dir: &Path) -> Option<PathBuf> {
    let object = out_dir.join("earth-resources.o");
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let candidates = [format!("{arch}-w64-mingw32-windres"), "windres".to_owned()];
    candidates.iter().find_map(|windres| {
        let status = Command::new(windres)
            .arg("--input")
            .arg(script)
            .args(["--output-format", "coff", "--output"])
            .arg(&object)
            .status()
            .ok()?;
        status.success().then(|| object.clone())
    })
}

fn compile_with_rc(script: &Path, out_dir: &Path) -> Option<PathBuf> {
    let resource = out_dir.join("earth.res");
    let mut candidates = vec![PathBuf::from("rc.exe")];
    candidates.extend(windows_sdk_rc());
    candidates.iter().find_map(|rc| {
        let status = Command::new(rc)
            .arg("/nologo")
            .arg("/fo")
            .arg(&resource)
            .arg(script)
            .status()
            .ok()?;
        status.success().then(|| resource.clone())
    })
}

/// `rc.exe` is usually not on PATH outside a Visual Studio prompt, so look
/// in the Windows SDK, newest version first.
fn windows_sdk_rc() -> Vec<PathBuf> {
    let host = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x64"
    };
    let bin = Path::new(r"C:\Program Files (x86)\Windows Kits\10\bin");
    let mut versions: Vec<PathBuf> = fs::read_dir(bin)
        .map(|entries| {
            entries
                .filter_map(|entry| Some(entry.ok()?.path()))
                .collect()
        })
        .unwrap_or_default();
    versions.sort();
    versions
        .into_iter()
        .rev()
        .map(|version| version.join(host).join("rc.exe"))
        .filter(|rc| rc.exists())
        .collect()
}
