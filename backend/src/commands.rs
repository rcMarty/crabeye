use anyhow::Context;
use chrono::{NaiveDate, NaiveDateTime};
use clap::{Parser, Subcommand};
use std::option::Option;

#[derive(Parser)]
#[command(name = "crabeye", version, about = "Crabeye CLI")]
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
        sync: Option<crabeye::git::SyncMode>,
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

fn parse_sync_mode(mode: &str) -> Result<crabeye::git::SyncMode, anyhow::Error> {
    if let Ok(duration) = mode.parse::<u32>() {
        log::debug!("Sync mode: SyncMode::Last({})", duration);
        Ok(crabeye::git::SyncMode::Last(duration))
    } else if let Ok(date) = parse_date(mode) {
        let datetime = NaiveDateTime::new(date, chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap());
        log::debug!("Sync mode: SyncMode::Since({})", datetime);
        Ok(crabeye::git::SyncMode::Since(datetime))
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
