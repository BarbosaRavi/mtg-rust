mod requests;
mod tools;

use anyhow::Result;
use requests::{CardNames, CheckDeck, DeckName, ListDecks, SearchCards};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{Implementation, ServerCapabilities, ServerConfig};
use rmcp::transport::stdio;
use rmcp::{ServerHandler, ServiceExt, tool, tool_handler, tool_router};
use rust_mtg::catalog::Catalog;
use rust_mtg::decks::{self, Decks};
use rust_mtg::inventory::{self, Wildcards};
use rust_mtg::language::Language;
use std::path::PathBuf;

const INSTRUCTIONS: &str = "Tools that read the MTG Arena collection, decks and wildcards of the \
     player on this computer. Card names are in English. search_cards finds cards (set \
     owned_only=false to include cards the player could craft); card_details gives full rules \
     text and legality; list_decks and show_deck show the player's decks; check_deck validates a \
     decklist against the collection and wildcards, so use it before recommending a deck. The \
     'craft' rarity is the lowest rarity among Arena printings, usually the wildcard needed.";

struct MtgServer {
    log_path: PathBuf,
    card_db: PathBuf,
}

#[tool_router]
impl MtgServer {
    #[tool(
        description = "The player's MTG Arena wildcards (common, uncommon, rare, mythic), \
                       which they can spend to craft cards they don't own."
    )]
    async fn my_wildcards(&self) -> Result<String, String> {
        reply(self.wildcards().and_then(tools::wildcards))
    }

    #[tool(
        description = "Search MTG Arena cards by name, rules text, type, colors, mana value, \
                       rarity, set and format legality. Returns each card's cost, type, rules \
                       text, how many copies the player owns and the wildcard rarity to craft it."
    )]
    async fn search_cards(
        &self,
        Parameters(request): Parameters<SearchCards>,
    ) -> Result<String, String> {
        reply(
            self.catalog()
                .and_then(|catalog| tools::search_cards(&catalog, &request)),
        )
    }

    #[tool(
        description = "Full details of cards by exact English name: cost, type, rules text, \
                       formats where they are legal, copies owned and craft rarity."
    )]
    async fn card_details(
        &self,
        Parameters(request): Parameters<CardNames>,
    ) -> Result<String, String> {
        reply(
            self.catalog()
                .and_then(|catalog| tools::card_details(&catalog, &request.names)),
        )
    }

    #[tool(
        description = "List the player's decks with format, colors, size, whether they built it \
                       or it is a precon, and what is missing to complete it."
    )]
    async fn list_decks(
        &self,
        Parameters(request): Parameters<ListDecks>,
    ) -> Result<String, String> {
        reply(self.catalog().and_then(|catalog| {
            let decks = self.decks(&catalog)?;
            tools::list_decks(&catalog, &decks, &request)
        }))
    }

    #[tool(
        description = "Show one of the player's decks card by card, with what is missing and \
                       the wildcards needed to complete it."
    )]
    async fn show_deck(&self, Parameters(request): Parameters<DeckName>) -> Result<String, String> {
        reply(self.catalog().and_then(|catalog| {
            let decks = self.decks(&catalog)?;
            tools::show_deck(&catalog, &decks, &request.name)
        }))
    }

    #[tool(
        description = "Check a decklist against the player's collection: copies owned and \
                       missing per card, wildcards needed by rarity, whether the player's \
                       wildcards are enough, and optional format legality."
    )]
    async fn check_deck(
        &self,
        Parameters(request): Parameters<CheckDeck>,
    ) -> Result<String, String> {
        reply(
            self.catalog()
                .and_then(|catalog| tools::check_deck(&catalog, &request, self.wildcards()?)),
        )
    }
}

impl MtgServer {
    fn catalog(&self) -> Result<Catalog> {
        Catalog::open(&self.card_db, Language::EnUs)
    }

    fn decks(&self, catalog: &Catalog) -> Result<Decks> {
        catalog.localized_decks(&decks::read_from_logs(&self.log_path)?)
    }

    fn wildcards(&self) -> Result<Option<Wildcards>> {
        inventory::wildcards_from_logs(&self.log_path)
    }
}

#[tool_handler]
impl ServerHandler for MtgServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("rust-mtg", env!("CARGO_PKG_VERSION")))
            .with_instructions(INSTRUCTIONS)
    }
}

fn reply(result: Result<String>) -> Result<String, String> {
    result.map_err(|error| format!("{error:#}"))
}

pub fn run(log_path: PathBuf, card_db: PathBuf) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async {
        let service = MtgServer { log_path, card_db }.serve(stdio()).await?;
        service.waiting().await?;
        Ok(())
    })
}
