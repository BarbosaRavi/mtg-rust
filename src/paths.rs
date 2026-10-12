use crate::wine;
use std::env;
use std::path::{Path, PathBuf};

pub fn detect_log_files() -> Vec<PathBuf> {
    let home = match env::home_dir() {
        Some(home) => home,
        None => return Vec::new(),
    };

    let mut found: Vec<PathBuf> = log_candidates(&home)
        .into_iter()
        .filter(|path| path.is_file())
        .filter_map(|path| dunce::canonicalize(path).ok())
        .collect();
    found.sort();
    found.dedup();
    found
}

pub fn pick_log_file() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Selecione o arquivo Player.log")
        .add_filter("Log do MTGA", &["log"])
        .pick_file()
}

pub(crate) fn join_all(base: &Path, parts: &[&str]) -> PathBuf {
    let mut path = base.to_path_buf();
    for part in parts {
        path.push(part);
    }
    path
}

fn log_candidates(home: &Path) -> Vec<PathBuf> {
    if cfg!(target_os = "windows") {
        vec![log_in_windows_profile(home)]
    } else if cfg!(target_os = "macos") {
        vec![join_all(
            home,
            &[
                "Library",
                "Logs",
                "Wizards Of The Coast",
                "MTGA",
                "Player.log",
            ],
        )]
    } else {
        linux_log_candidates(home)
    }
}

fn linux_log_candidates(home: &Path) -> Vec<PathBuf> {
    wine::prefixes(home)
        .iter()
        .flat_map(|prefix| wine::user_dirs(prefix))
        .map(|user_dir| log_in_windows_profile(&user_dir))
        .collect()
}

fn log_in_windows_profile(user_dir: &Path) -> PathBuf {
    join_all(
        user_dir,
        &[
            "AppData",
            "LocalLow",
            "Wizards Of The Coast",
            "MTGA",
            "Player.log",
        ],
    )
}
