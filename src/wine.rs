use crate::paths::join_all;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

const MTGA_STEAM_APPID: &str = "2141910";

pub fn prefixes(home: &Path) -> Vec<PathBuf> {
    let mut candidates = vec![home.join(".wine")];

    for library in steam_libraries(home) {
        candidates.push(join_all(
            &library,
            &["steamapps", "compatdata", MTGA_STEAM_APPID, "pfx"],
        ));
    }

    for parent in prefix_parents(home) {
        candidates.extend(subdirs(&parent));
    }

    candidates.retain(|prefix| prefix.join("drive_c").is_dir());
    candidates
}

pub fn user_dirs(prefix: &Path) -> Vec<PathBuf> {
    subdirs(&join_all(prefix, &["drive_c", "users"]))
}

pub fn prefix_of(path: &Path) -> Option<&Path> {
    path.ancestors()
        .find(|dir| dir.file_name() == Some(OsStr::new("drive_c")))?
        .parent()
}

pub fn windows_path_in_prefix(prefix: &Path, windows_path: &str) -> Option<PathBuf> {
    let (drive, rest) = windows_path.split_once(':')?;
    let mut path = join_all(
        prefix,
        &["dosdevices", &format!("{}:", drive.to_lowercase())],
    );

    for part in rest.split(['/', '\\']).filter(|part| !part.is_empty()) {
        path.push(part);
    }

    Some(path)
}

fn steam_libraries(home: &Path) -> Vec<PathBuf> {
    let mut libraries = Vec::new();

    for root in steam_roots(home) {
        let vdf = join_all(&root, &["steamapps", "libraryfolders.vdf"]);
        if let Ok(text) = fs::read_to_string(vdf) {
            libraries.extend(library_paths(&text));
        }
        libraries.push(root);
    }

    libraries
}

fn library_paths(vdf: &str) -> Vec<PathBuf> {
    vdf.lines()
        .filter_map(|line| line.trim().strip_prefix("\"path\""))
        .map(|value| PathBuf::from(value.trim().trim_matches('"')))
        .collect()
}

fn steam_roots(home: &Path) -> Vec<PathBuf> {
    vec![
        join_all(home, &[".steam", "steam"]),
        join_all(home, &[".local", "share", "Steam"]),
        join_all(
            home,
            &[
                ".var",
                "app",
                "com.valvesoftware.Steam",
                ".local",
                "share",
                "Steam",
            ],
        ),
    ]
}

fn prefix_parents(home: &Path) -> Vec<PathBuf> {
    vec![
        home.join("Games"),
        join_all(home, &[".local", "share", "bottles", "bottles"]),
        join_all(
            home,
            &[
                ".var",
                "app",
                "com.usebottles.bottles",
                "data",
                "bottles",
                "bottles",
            ],
        ),
    ]
}

fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };

    entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect()
}
