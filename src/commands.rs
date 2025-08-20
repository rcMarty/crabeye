use crate::db::model::pr_event::PullRequestStatus;
use anyhow::Context;
use chrono::NaiveDate;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "ranal", version, about = "Rust Analyzer CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub(crate) command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Analyze the repository
    Analyze,
    /// Serve the REST API
    Serve,
}

pub fn parse_date(date: &str) -> Result<NaiveDate, anyhow::Error> {
    NaiveDate::parse_from_str(date, "%Y-%m-%d").context("Format (YYYY-MM-DD)")
}
pub fn parse_duration(duration: &str) -> Result<chrono::Duration, anyhow::Error> {
    let duration = duration.trim().parse::<u32>().context("Duration should be non negative in days from now")?;
    Ok(chrono::Duration::days(duration as i64))
}

pub fn parse_event(event: &str) -> Result<PullRequestStatus, String> {
    // TODO utc now timestamp i feel is wrong
    match PullRequestStatus::from_str(&event.to_string(), chrono::Utc::now(), None) {
        Some(status) => Ok(status),
        None => Err(format!("Invalid event: {}", event)),
    }
}
