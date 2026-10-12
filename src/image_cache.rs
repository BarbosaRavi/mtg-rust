use crate::{config, storage};
use anyhow::{Context, Result};
use reqwest::blocking::Client;
use std::fs;
use std::path::{Path, PathBuf};

pub fn cached_path(url: &str) -> Result<PathBuf> {
    let without_query = url.split('?').next().unwrap_or(url);
    let (_, address) = without_query
        .split_once("://")
        .with_context(|| format!("URL de imagem inválida: {url}"))?;

    let mut path = config::project_dirs()?.cache_dir().join("images");
    for segment in address.split('/').skip(1) {
        if !segment.is_empty() && segment != "." && segment != ".." {
            path.push(segment);
        }
    }
    Ok(path)
}

pub fn download(client: &Client, url: &str, destination: &Path) -> Result<()> {
    storage::ensure_parent_dir(destination)?;

    let bytes = client
        .get(url)
        .send()
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.bytes())
        .with_context(|| format!("não consegui baixar {url}"))?;

    let partial = destination.with_extension("part");
    fs::write(&partial, &bytes)?;
    fs::rename(&partial, destination)?;
    Ok(())
}

pub fn file_uri(path: &Path) -> String {
    if cfg!(target_os = "windows") {
        format!("file:///{}", path.display())
    } else {
        format!("file://{}", path.display())
    }
}
