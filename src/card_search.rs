use anyhow::{Result, bail};
use clap::{Args, ValueEnum};
use rusqlite::types::Value;

const COLOR_ORDER: &str = "WUBRG";

#[derive(Debug, Clone, Default, Args)]
pub struct CardFilter {
    #[arg(short, long)]
    pub name: Option<String>,

    #[arg(short = 'o', long)]
    pub text: Option<String>,

    #[arg(short = 't', long = "type")]
    pub type_line: Option<String>,

    #[arg(short, long)]
    pub color: Option<String>,

    #[arg(long)]
    pub exact_color: bool,

    #[arg(long)]
    pub cmc: Option<f64>,

    #[arg(long)]
    pub cmc_min: Option<f64>,

    #[arg(long)]
    pub cmc_max: Option<f64>,

    #[arg(short, long)]
    pub rarity: Option<Rarity>,

    #[arg(short, long)]
    pub set: Option<String>,

    #[arg(short, long)]
    pub format: Option<String>,

    #[arg(long)]
    pub owned: bool,

    #[arg(long, default_value_t = 50)]
    pub limit: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Mythic,
}

impl Rarity {
    pub fn as_str(self) -> &'static str {
        match self {
            Rarity::Common => "common",
            Rarity::Uncommon => "uncommon",
            Rarity::Rare => "rare",
            Rarity::Mythic => "mythic",
        }
    }
}

impl CardFilter {
    pub(crate) fn where_clause(&self) -> Result<(String, Vec<Value>)> {
        let mut conditions: Vec<&str> = Vec::new();
        let mut values: Vec<Value> = Vec::new();

        let mut add = |condition, value| {
            conditions.push(condition);
            values.push(value);
        };

        if let Some(name) = &self.name {
            add("name LIKE ?", contains(name));
        }
        if let Some(text) = &self.text {
            add("oracle_text LIKE ?", contains(text));
        }
        if let Some(type_line) = &self.type_line {
            add("type_line LIKE ?", contains(type_line));
        }
        if let Some(color) = &self.color {
            let colors = normalize_colors(color)?;
            if self.exact_color || colors.is_empty() {
                add("colors = ?", Value::Text(colors));
            } else {
                for letter in colors.chars() {
                    add("colors LIKE ?", contains(&letter.to_string()));
                }
            }
        }
        if let Some(cmc) = self.cmc {
            add("cmc = ?", Value::Real(cmc));
        }
        if let Some(cmc_min) = self.cmc_min {
            add("cmc >= ?", Value::Real(cmc_min));
        }
        if let Some(cmc_max) = self.cmc_max {
            add("cmc <= ?", Value::Real(cmc_max));
        }
        if let Some(rarity) = self.rarity {
            add("rarity = ?", Value::Text(rarity.as_str().to_string()));
        }
        if let Some(set) = &self.set {
            add("set_code = ?", Value::Text(set.to_uppercase()));
        }
        if let Some(format) = &self.format {
            add(
                "json_extract(legalities, ?) = 'legal'",
                Value::Text(format!("$.{}", format.to_lowercase())),
            );
        }

        let clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };
        Ok((clause, values))
    }
}

fn contains(text: &str) -> Value {
    Value::Text(format!("%{text}%"))
}

fn normalize_colors(input: &str) -> Result<String> {
    let input = input.to_uppercase();

    if let Some(invalid) = input.chars().find(|letter| !"WUBRGC".contains(*letter)) {
        bail!("cor inválida: {invalid} (use W, U, B, R, G ou C para incolor)");
    }
    if input.contains('C') {
        if input.len() > 1 {
            bail!("C (incolor) não pode ser combinado com outras cores");
        }
        return Ok(String::new());
    }

    Ok(in_color_order(&input))
}

pub(crate) fn in_color_order(letters: &str) -> String {
    COLOR_ORDER
        .chars()
        .filter(|letter| letters.contains(*letter))
        .collect()
}
