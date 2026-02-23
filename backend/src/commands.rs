use crate::db::model::pr_event::PullRequestStatus;
use anyhow::Context;
use chrono::{NaiveDate, NaiveDateTime};
use clap::{Parser, Subcommand};
use std::option::Option;

#[derive(Parser)]
#[command(name = "ranal", version, about = "Rust Analyzer CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub(crate) command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Analyze the repository
    SyncAll {
        #[arg(
            short,
            long,
            value_parser=parse_sync_mode,
            help = "From date (YYYY-MM-DD) or number of N pages"
        )]
        sync: Option<crate::git::github::SyncMode>,
        #[arg(
            short,
            long,
            help = "Whether to fetch the full history of each issue and pull request. This will be much slower but will give you more data for analysis."
        )]
        full_history: Option<bool>,
    },
    /// Download history of issues and pull requests which doesnt have history in the database and update the database with new data.
    Backfill,
    /// Serve the REST API
    Serve,
}

fn parse_sync_mode(mode: &str) -> Result<crate::git::github::SyncMode, anyhow::Error> {
    if let Ok(duration) = mode.parse::<u32>() {
        log::debug!("Sync mode: SyncMode::Last({})", duration);
        Ok(crate::git::github::SyncMode::Last(duration))
    } else if let Ok(date) = parse_date(mode) {
        let datetime = NaiveDateTime::new(date, chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap());
        log::debug!("Sync mode: SyncMode::Since({})", datetime);
        Ok(crate::git::github::SyncMode::Since(datetime))
    } else {
        Err(anyhow::anyhow!(
            "Invalid mode: {}. Use either YYYY-MM-DD or a non-negative integer for days.",
            mode
        ))
    }
}

fn parse_date(date: &str) -> Result<NaiveDate, anyhow::Error> {
    NaiveDate::parse_from_str(date, "%Y-%m-%d").context("Format (YYYY-MM-DD)")
}
fn parse_duration(duration: &str) -> Result<chrono::Duration, anyhow::Error> {
    let duration = duration
        .trim()
        .parse::<u32>()
        .context("Duration should be non negative in days from now")?;
    Ok(chrono::Duration::days(duration as i64))
}

fn parse_event(event: &str) -> Result<PullRequestStatus, String> {
    // TODO utc now timestamp i feel is wrong
    match PullRequestStatus::from_parts(event, chrono::Utc::now(), None) {
        Some(status) => Ok(status),
        None => Err(format!("Invalid event: {}", event)),
    }
}
