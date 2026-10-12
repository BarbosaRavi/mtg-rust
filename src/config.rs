use crate::language::Language;
use crate::storage;
use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Config {
    pub log_path: Option<PathBuf>,
    pub raw_dir: Option<PathBuf>,
    pub language: Option<Language>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let file = config_file()?;

        match storage::read_if_exists(&file)? {
            Some(text) => toml::from_str(&text)
                .with_context(|| format!("config inválida em {}", file.display())),
            None => Ok(Self::default()),
        }
    }

    pub fn save(&self) -> Result<()> {
        storage::write_creating_dirs(&config_file()?, &toml::to_string(self)?)
    }
}

pub fn config_file() -> Result<PathBuf> {
    Ok(project_dirs()?.config_dir().join("config.toml"))
}

pub(crate) fn project_dirs() -> Result<ProjectDirs> {
    ProjectDirs::from("", "", "rust-mtg").context("não encontrei as pastas do sistema para o app")
}
