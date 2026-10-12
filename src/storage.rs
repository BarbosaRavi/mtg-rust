use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

pub fn read_if_exists(file: &Path) -> Result<Option<String>> {
    if !file.exists() {
        return Ok(None);
    }

    fs::read_to_string(file)
        .map(Some)
        .with_context(|| format!("não consegui ler {}", file.display()))
}

pub fn write_creating_dirs(file: &Path, contents: &str) -> Result<()> {
    ensure_parent_dir(file)?;
    fs::write(file, contents).with_context(|| format!("não consegui salvar {}", file.display()))
}

pub fn ensure_parent_dir(file: &Path) -> Result<()> {
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).with_context(|| format!("não consegui criar {}", dir.display()))?;
    }
    Ok(())
}
