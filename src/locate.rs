use crate::config::Config;
use crate::{card_db, paths};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub fn log_path(from_cli: Option<PathBuf>, config: &mut Config) -> Result<PathBuf> {
    if let Some(found) = find_log_path(from_cli, config) {
        return Ok(found);
    }

    let picked = paths::pick_log_file().context("nenhum Player.log selecionado")?;
    config.log_path = Some(picked.clone());
    Ok(picked)
}

pub fn find_log_path(from_cli: Option<PathBuf>, config: &mut Config) -> Option<PathBuf> {
    if from_cli.is_some() {
        return from_cli;
    }

    if let Some(saved) = config.log_path.clone().filter(|path| path.is_file()) {
        return Some(saved);
    }

    let found = detect_single_log()?;
    config.log_path = Some(found.clone());
    Some(found)
}

pub fn card_db_path(
    from_cli: Option<PathBuf>,
    config: &mut Config,
    log_path: &Path,
    log_text: &str,
) -> Result<PathBuf> {
    if let Some(dir) = &from_cli {
        return card_db::newest_in(dir)
            .with_context(|| format!("nenhum Raw_CardDatabase_*.mtga em {}", dir.display()));
    }

    if let Some(found) = find_card_db(None, config, log_path, log_text) {
        return Ok(found);
    }

    let picked = card_db::pick_card_db().context("nenhum banco de cartas selecionado")?;
    config.raw_dir = picked.parent().map(Path::to_path_buf);
    Ok(picked)
}

pub fn find_card_db(
    from_cli: Option<PathBuf>,
    config: &mut Config,
    log_path: &Path,
    log_text: &str,
) -> Option<PathBuf> {
    if let Some(dir) = from_cli {
        return card_db::newest_in(&dir);
    }

    if let Some(saved) = config.raw_dir.as_deref().and_then(card_db::newest_in) {
        return Some(saved);
    }

    let found =
        card_db::raw_dir_from_log(log_path, log_text).and_then(|dir| card_db::newest_in(&dir))?;
    config.raw_dir = found.parent().map(Path::to_path_buf);
    Some(found)
}

fn detect_single_log() -> Option<PathBuf> {
    let mut detected = paths::detect_log_files();

    if detected.len() == 1 {
        detected.pop()
    } else {
        None
    }
}
