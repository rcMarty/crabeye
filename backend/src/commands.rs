use crate::db::model::pr_event::PullRequestStatus;
use anyhow::Context;
use chrono::{NaiveDate, NaiveDateTime};
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
    Analyze {
        #[arg(short, long,value_parser=parse_sync_mode ,help = "From date (YYYY-MM-DD) or number of N pages")]
        sync: crate::git::github::SyncMode,
    },
    /// Serve the REST API
    Serve,
}

pub fn parse_sync_mode(mode: &str) -> Result<crate::git::github::SyncMode, anyhow::Error> {
    if let Ok(date) = parse_date(mode) {
        let datetime = NaiveDateTime::new(date, chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap());
        Ok(crate::git::github::SyncMode::Since(datetime))
    } else if let Ok(duration) = mode.parse::<u32>() {
        Ok(crate::git::github::SyncMode::Last(duration))
    } else {
        Err(anyhow::anyhow!(
            "Invalid mode: {}. Use either YYYY-MM-DD or a non-negative integer for days.",
            mode
        ))
    }
}

pub fn parse_date(date: &str) -> Result<NaiveDate, anyhow::Error> {
    NaiveDate::parse_from_str(date, "%Y-%m-%d").context("Format (YYYY-MM-DD)")
}
pub fn parse_duration(duration: &str) -> Result<chrono::Duration, anyhow::Error> {
    let duration = duration
        .trim()
        .parse::<u32>()
        .context("Duration should be non negative in days from now")?;
    Ok(chrono::Duration::days(duration as i64))
}

pub fn parse_event(event: &str) -> Result<PullRequestStatus, String> {
    // TODO utc now timestamp i feel is wrong
    match PullRequestStatus::from_str(event, chrono::Utc::now(), None) {
        Some(status) => Ok(status),
        None => Err(format!("Invalid event: {}", event)),
    }
}
