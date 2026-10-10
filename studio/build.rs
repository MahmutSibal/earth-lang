//! Embeds the icon, version information and manifest into
//! `earth-studio.exe`; see `windows/resources.rs`.

include!("../windows/resources.rs");

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    embed_windows_resources(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../windows"),
        Details {
            product: "Earth Studio",
            description: "Earth Studio - Earth programlama dili editoru",
            file_name: "earth-studio.exe",
        },
    );
}
