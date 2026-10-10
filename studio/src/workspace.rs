//! The user's Earth folder and the example programs built into Studio.
//!
//! Studio ships as a single executable: the examples live inside it and are
//! written to `Documents\Earth\Örnekler` when the user wants them.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Example programs, embedded at build time.
pub const EXAMPLES: &[(&str, &str)] = &[
    ("merhaba.earth", include_str!("../../examples/hello.earth")),
    (
        "fizzbuzz.earth",
        include_str!("../../examples/fizzbuzz.earth"),
    ),
    (
        "fibonacci.earth",
        include_str!("../../examples/fibonacci.earth"),
    ),
    ("liste.earth", include_str!("../../examples/liste.earth")),
    ("sozluk.earth", include_str!("../../examples/sozluk.earth")),
    ("selam.earth", include_str!("../../examples/selam.earth")),
    (
        "moduller.earth",
        include_str!("../../examples/moduller.earth"),
    ),
    (
        "araclar.earth",
        include_str!("../../examples/araclar.earth"),
    ),
];

const EXAMPLES_FOLDER: &str = "Örnekler";

/// `Documents\Earth`, where Studio keeps the user's programs.
pub fn earth_folder() -> Option<PathBuf> {
    documents_folder().map(|documents| documents.join("Earth"))
}

fn documents_folder() -> Option<PathBuf> {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from);
    // OneDrive may move Documents; its folder is named after the language.
    let one_drive = std::env::var_os("OneDrive").map(PathBuf::from);
    let candidates = one_drive
        .iter()
        .flat_map(|root| [root.join("Documents"), root.join("Belgeler")])
        .chain(home.iter().map(|home| home.join("Documents")));
    candidates.into_iter().find(|path| path.is_dir()).or(home)
}

/// Creates the Earth folder, with the examples, the first time Studio runs.
/// The examples are also written when the folder is still empty, for
/// example because writing them failed last time. The folder is opened
/// even if the examples cannot be written.
pub fn prepare_earth_folder() -> Option<PathBuf> {
    let folder = earth_folder()?;
    fs::create_dir_all(&folder).ok()?;
    let is_empty = fs::read_dir(&folder).is_ok_and(|mut entries| entries.next().is_none());
    if is_empty {
        let _ = write_examples(&folder);
    }
    Some(folder)
}

/// Writes the examples into `<folder>/Örnekler`. Files the user already has
/// are left untouched. Returns the examples folder.
pub fn write_examples(folder: &Path) -> io::Result<PathBuf> {
    let examples = folder.join(EXAMPLES_FOLDER);
    fs::create_dir_all(&examples)?;
    for (name, source) in EXAMPLES {
        let path = examples.join(name);
        if !path.exists() {
            fs::write(path, source)?;
        }
    }
    Ok(examples)
}

#[cfg(test)]
mod tests {
    use super::{write_examples, EXAMPLES};

    #[test]
    fn writes_examples_without_overwriting() {
        let folder = std::env::temp_dir().join(format!("earth-studio-ws-{}", std::process::id()));
        let examples = write_examples(&folder).unwrap();
        std::fs::write(examples.join("merhaba.earth"), "yaz(1)").unwrap();
        write_examples(&folder).unwrap();

        assert_eq!(
            std::fs::read_dir(&examples).unwrap().count(),
            EXAMPLES.len()
        );
        assert_eq!(
            std::fs::read_to_string(examples.join("merhaba.earth")).unwrap(),
            "yaz(1)"
        );
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn embedded_examples_parse() {
        for (name, source) in EXAMPLES {
            assert!(earth::parse(source).is_ok(), "{name} does not parse");
        }
    }
}
