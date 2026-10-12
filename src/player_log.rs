use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Request,
    Response,
}

pub struct LogEvent<'a> {
    pub direction: Direction,
    pub name: &'a str,
    pub body: &'a str,
}

#[derive(Default)]
pub struct EventSummary {
    pub count: usize,
    pub keys: BTreeSet<String>,
}

pub fn read_log(path: &Path) -> Result<String> {
    let bytes =
        fs::read(path).with_context(|| format!("não consegui ler o arquivo {}", path.display()))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

pub fn previous_log(log_path: &Path) -> Option<PathBuf> {
    let previous = log_path.with_file_name("Player-prev.log");
    (previous != log_path && previous.is_file()).then_some(previous)
}

pub fn events(text: &str) -> Vec<LogEvent<'_>> {
    let mut events = Vec::new();
    let mut lines = text.lines();

    while let Some(line) = lines.next() {
        if let Some(name) = response_name(line) {
            let body = lines.next().unwrap_or("");
            events.push(LogEvent {
                direction: Direction::Response,
                name,
                body,
            });
        } else if let Some((name, body)) = request(line) {
            events.push(LogEvent {
                direction: Direction::Request,
                name,
                body,
            });
        }
    }

    events
}

pub fn summarize_events(text: &str) -> BTreeMap<String, EventSummary> {
    let mut summary: BTreeMap<String, EventSummary> = BTreeMap::new();

    for event in events(text) {
        if event.direction == Direction::Response {
            let entry = summary.entry(event.name.to_string()).or_default();
            entry.count += 1;
            entry.keys.extend(top_level_keys(event.body));
        }
    }

    summary
}

fn response_name(line: &str) -> Option<&str> {
    let (name, _) = line.strip_prefix("<== ")?.split_once('(')?;
    Some(name)
}

fn request(line: &str) -> Option<(&str, &str)> {
    let (_, rest) = line.split_once("==> ")?;
    rest.split_once(' ')
}

fn top_level_keys(json: &str) -> Vec<String> {
    match serde_json::from_str::<Value>(json) {
        Ok(Value::Object(map)) => map.keys().cloned().collect(),
        _ => Vec::new(),
    }
}
