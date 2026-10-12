use crate::{config, storage};
use anyhow::{Context, Result};
use serde_json::json;
use std::env;
use std::path::{Path, PathBuf};

const SERVER: &str = "rust-mtg";
#[cfg(target_os = "linux")]
const SYSTEM_PROMPT: &str = "You are helping the user with their Magic: The Gathering Arena \
                             collection and decks. Use the rust-mtg tools to read their real \
                             data instead of guessing. Reply in the user's language.";

pub fn open() -> Result<()> {
    let dir = config::project_dirs()?.data_dir().join("claude");
    let mcp_config = write_mcp_config(&dir)?;
    let claude =
        find_in_path("claude").context("não encontrei o comando `claude` (Claude Code) no PATH")?;
    launch_in_terminal(&dir, &claude, &mcp_config)
}

fn write_mcp_config(dir: &Path) -> Result<PathBuf> {
    let exe = env::current_exe().context("não consegui descobrir onde está o rust-mtg")?;
    let exe = exe.to_str().context("o caminho do rust-mtg não é UTF-8")?;

    let mut servers = serde_json::Map::new();
    servers.insert(SERVER.into(), json!({ "command": exe, "args": ["mcp"] }));
    let config = json!({ "mcpServers": servers });

    let file = dir.join("mcp.json");
    storage::write_creating_dirs(&file, &serde_json::to_string_pretty(&config)?)?;
    Ok(file)
}

fn find_in_path(program: &str) -> Option<PathBuf> {
    let paths = env::var_os("PATH")?;
    env::split_paths(&paths)
        .map(|dir| dir.join(program))
        .find(|path| path.is_file())
}

#[cfg(target_os = "linux")]
fn launch_in_terminal(dir: &Path, claude: &Path, mcp_config: &Path) -> Result<()> {
    let mut terminal = std::process::Command::new("xdg-terminal-exec")
        .current_dir(dir)
        .arg(claude)
        .arg("--mcp-config")
        .arg(mcp_config)
        .arg("--strict-mcp-config")
        .args(["--allowedTools", &format!("mcp__{SERVER}")])
        .args(["--append-system-prompt", SYSTEM_PROMPT])
        .spawn()
        .context("não consegui abrir um terminal (xdg-terminal-exec)")?;

    std::thread::spawn(move || terminal.wait());
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn launch_in_terminal(_dir: &Path, _claude: &Path, _mcp_config: &Path) -> Result<()> {
    anyhow::bail!("abrir o Claude pelo app ainda só funciona no Linux")
}
