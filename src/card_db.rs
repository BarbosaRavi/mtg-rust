use crate::wine;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const DOWNLOADS_MARKER: &str = "Bundle download destination: ";

pub fn raw_dir_from_log(log_path: &Path, log_text: &str) -> Option<PathBuf> {
    let (_, downloads) = log_text
        .lines()
        .find_map(|line| line.split_once(DOWNLOADS_MARKER))?;
    let downloads = downloads.trim();

    let downloads_dir = match wine::prefix_of(log_path) {
        Some(prefix) => wine::windows_path_in_prefix(prefix, downloads)?,
        None => PathBuf::from(downloads),
    };

    dunce::canonicalize(downloads_dir.join("Raw")).ok()
}

pub fn newest_in(raw_dir: &Path) -> Option<PathBuf> {
    newest_raw_file(raw_dir, "Raw_CardDatabase_")
}

pub fn localization_db_in(raw_dir: &Path) -> Option<PathBuf> {
    newest_raw_file(raw_dir, "Raw_ClientLocalization_")
}

pub fn pick_card_db() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Selecione o Raw_CardDatabase_*.mtga (em MTGA_Data/Downloads/Raw)")
        .add_filter("Banco de cartas do MTGA", &["mtga"])
        .pick_file()
}

fn newest_raw_file(raw_dir: &Path, prefix: &str) -> Option<PathBuf> {
    fs::read_dir(raw_dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| has_raw_name(path, prefix))
        .max_by_key(|path| modified_at(path))
}

fn has_raw_name(path: &Path, prefix: &str) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with(prefix) && name.ends_with(".mtga"))
}

fn modified_at(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|meta| meta.modified()).ok()
}
