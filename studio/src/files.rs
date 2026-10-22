//! The folder tree shown in the explorer.

use std::fs;
use std::path::{Path, PathBuf};

/// Folders deeper than this are not listed, to keep huge trees fast.
const MAX_DEPTH: usize = 8;

/// Folders that are never interesting to an Earth programmer.
const SKIPPED: &[&str] = &["target", "node_modules", "__pycache__"];

pub struct Node {
    pub path: PathBuf,
    pub name: String,
    pub children: Option<Vec<Node>>,
}

impl Node {
    pub fn is_dir(&self) -> bool {
        self.children.is_some()
    }
}

/// Lists `root` recursively: folders first, then files, each alphabetical.
/// Hidden entries (starting with a dot) are skipped.
pub fn scan(root: &Path) -> Vec<Node> {
    scan_dir(root, 0)
}

fn scan_dir(dir: &Path, depth: usize) -> Vec<Node> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut nodes: Vec<Node> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                return None;
            }
            let path = entry.path();
            let is_dir = entry.file_type().ok()?.is_dir();
            if is_dir && SKIPPED.contains(&name.as_str()) {
                return None;
            }
            let children = match (is_dir, depth < MAX_DEPTH) {
                (true, true) => Some(scan_dir(&path, depth + 1)),
                (true, false) => Some(Vec::new()),
                (false, _) => None,
            };
            Some(Node {
                path,
                name,
                children,
            })
        })
        .collect();
    nodes.sort_by(|a, b| {
        b.is_dir()
            .cmp(&a.is_dir())
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    nodes
}

/// Adds the `.earth` extension to a new file name that has none.
pub fn with_earth_extension(name: &str) -> String {
    let name = name.trim();
    if Path::new(name).extension().is_some() {
        name.to_owned()
    } else {
        format!("{name}.earth")
    }
}

pub fn is_python(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("py"))
}

#[cfg(test)]
mod tests {
    use super::{is_python, scan, with_earth_extension};
    use std::path::Path;

    #[test]
    fn lists_folders_first_and_skips_hidden_entries() {
        let root = std::env::temp_dir().join(format!("earth-studio-scan-{}", std::process::id()));
        std::fs::create_dir_all(root.join("b_klasör")).unwrap();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(root.join("a.earth"), "").unwrap();
        std::fs::write(root.join("b_klasör/iç.earth"), "").unwrap();

        let nodes = scan(&root);
        let names: Vec<&str> = nodes.iter().map(|node| node.name.as_str()).collect();
        assert_eq!(names, ["b_klasör", "a.earth"]);
        assert_eq!(nodes[0].children.as_ref().unwrap()[0].name, "iç.earth");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn adds_the_earth_extension() {
        assert_eq!(with_earth_extension(" oyun "), "oyun.earth");
        assert_eq!(with_earth_extension("notlar.txt"), "notlar.txt");
    }

    #[test]
    fn recognizes_python_files_case_insensitively() {
        assert!(is_python(Path::new("main.py")));
        assert!(is_python(Path::new("main.PY")));
        assert!(!is_python(Path::new("main.earth")));
    }
}
