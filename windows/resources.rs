// Shared by the build scripts of `earth` and `earth-studio` through
// `include!`. Embeds the icon, version information and manifest into the
// Windows executable. Other targets are untouched. If no resource compiler
// is available the build still succeeds, just without these extras.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// What Windows shows in the file's Properties → Details tab.
struct Details {
    product: &'static str,
    description: &'static str,
    file_name: &'static str,
}

fn embed_windows_resources(windows_dir: &Path, details: Details) {
    println!("cargo:rerun-if-changed={}", windows_dir.join("earth.ico").display());
    println!("cargo:rerun-if-changed={}", windows_dir.join("earth.manifest").display());
    println!("cargo:rerun-if-changed={}", windows_dir.join("resources.rs").display());

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    let script = out_dir.join("resources.rc");
    fs::write(&script, resource_script(windows_dir, &details)).expect("could not write resources.rc");

    let compiled = match env::var("CARGO_CFG_TARGET_ENV").as_deref() {
        Ok("gnu") => compile_with_windres(&script, &out_dir),
        Ok("msvc") => compile_with_rc(&script, &out_dir),
        _ => None,
    };
    match compiled {
        Some(resource) => println!("cargo:rustc-link-arg-bins={}", resource.display()),
        None => println!(
            "cargo:warning=No Windows resource compiler found; {} will have no icon or version information",
            details.file_name
        ),
    }
}

fn resource_script(windows_dir: &Path, details: &Details) -> String {
    let version = env::var("CARGO_PKG_VERSION").expect("cargo sets the version");
    let mut parts: Vec<u16> = version
        .split(['.', '-'])
        .take(3)
        .map(|part| part.parse().unwrap_or(0))
        .collect();
    parts.resize(3, 0);
    let numeric = format!("{},{},{},0", parts[0], parts[1], parts[2]);
    let path = |name: &str| {
        windows_dir
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
      VALUE "FileDescription", "{description}"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "{product}"
      VALUE "LegalCopyright", "Copyright (c) 2026 Mahmut Sibal, MIT License"
      VALUE "OriginalFilename", "{file_name}"
      VALUE "ProductName", "{product}"
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
        product = details.product,
        description = details.description,
        file_name = details.file_name,
    )
}

fn compile_with_windres(script: &Path, out_dir: &Path) -> Option<PathBuf> {
    let object = out_dir.join("resources.o");
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
    let resource = out_dir.join("resources.res");
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
