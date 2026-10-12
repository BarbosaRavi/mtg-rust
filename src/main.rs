mod commands;
mod gui;
mod mcp;
mod output;

use anyhow::Result;
use clap::{Parser, Subcommand};
use commands::App;
use rust_mtg::card_search::CardFilter;
use rust_mtg::config::Config;
use rust_mtg::language::Language;
use rust_mtg::{locate, player_log};
use std::path::PathBuf;

#[derive(Parser)]
#[command(version)]
struct Cli {
    #[arg(long, global = true)]
    log: Option<PathBuf>,

    #[arg(long, global = true)]
    raw_dir: Option<PathBuf>,

    #[arg(long, global = true)]
    lang: Option<Language>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    Gui,
    #[command(flatten)]
    Cli(Box<CliCommand>),
}

#[derive(Subcommand)]
enum CliCommand {
    Decks,
    Deck { name: String },
    Card { id: u32 },
    Search(CardFilter),
    Watch,
    UpdateCards,
    SyncCollection,
    Events,
    Paths,
    Mcp,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut config = Config::load()?;
    if cli.lang.is_some() {
        config.language = cli.lang;
    }

    match cli.command.unwrap_or(Command::Gui) {
        Command::Gui => gui::run(config, cli.log, cli.raw_dir),
        Command::Cli(command) => run_command(*command, cli.log, cli.raw_dir, config),
    }
}

fn run_command(
    command: CliCommand,
    log: Option<PathBuf>,
    raw_dir: Option<PathBuf>,
    mut config: Config,
) -> Result<()> {
    let log_path = locate::log_path(log, &mut config)?;
    let log_text = player_log::read_log(&log_path)?;
    let card_db = locate::card_db_path(raw_dir, &mut config, &log_path, &log_text)?;
    config.save()?;

    let app = App {
        log_path,
        card_db,
        language: config.language.unwrap_or_default(),
    };

    match command {
        CliCommand::Decks => commands::list_decks(&app),
        CliCommand::Deck { name } => commands::show_deck(&app, &name),
        CliCommand::Card { id } => commands::show_card(&app, id),
        CliCommand::Search(filter) => commands::search(&app, &filter),
        CliCommand::Watch => commands::watch(&app),
        CliCommand::UpdateCards => commands::update_cards(),
        CliCommand::SyncCollection => commands::sync_collection(&app),
        CliCommand::Events => {
            println!(
                "{}",
                output::event_summary(&player_log::summarize_events(&log_text))
            );
            Ok(())
        }
        CliCommand::Paths => commands::print_paths(&app),
        CliCommand::Mcp => mcp::run(app.log_path, app.card_db),
    }
}
