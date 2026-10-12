use clap::ValueEnum;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Language {
    #[default]
    EnUs,
    PtBr,
    EsEs,
    FrFr,
    DeDe,
    ItIt,
    JaJp,
    KoKr,
}

impl Language {
    pub fn label(self) -> &'static str {
        match self {
            Language::EnUs => "Inglês",
            Language::PtBr => "Português",
            Language::EsEs => "Espanhol",
            Language::FrFr => "Francês",
            Language::DeDe => "Alemão",
            Language::ItIt => "Italiano",
            Language::JaJp => "Japonês",
            Language::KoKr => "Coreano",
        }
    }

    pub fn arena_code(self) -> &'static str {
        match self {
            Language::EnUs => "enUS",
            Language::PtBr => "ptBR",
            Language::EsEs => "esES",
            Language::FrFr => "frFR",
            Language::DeDe => "deDE",
            Language::ItIt => "itIT",
            Language::JaJp => "jaJP",
            Language::KoKr => "koKR",
        }
    }
}
