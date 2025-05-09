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
    /// Get various information about the repository
    Request {
        #[command(subcommand)]
        subcommand: RequestSubcommands,
    },
}

#[derive(Subcommand)]
pub enum RequestSubcommands {
    /// Get the state of a PR at a specific timestamp
    PrStateAt {
        #[arg(short, long)]
        pr_number: i64,
        #[arg(short, long, value_parser = parse_date , default_value = "2025-03-21")]
        date: NaiveDate,
    },
    /// Get the count of PRs in a specific state at a given timestamp
    PrCountInState {
        #[arg(short, long)]
        state: String,
        #[arg(short, long, value_parser = parse_date)]
        date: NaiveDate,
    },
    /// Get the top N files modified or reviewed by a user in a specific PR
    TopNFiles {
        #[arg(short, long)]
        user_id: i64,
        #[arg(short, long, value_parser = parse_duration, default_value = "30")]
        days: chrono::Duration,
        #[arg(short, long, default_value = "10")]
        n: i64,
    },
    /// Get users who modified or reviewed a specific file
    UsersWhoModifiedFile {
        #[arg(short, long)]
        file_path: String,
        #[arg(short, long, value_parser = parse_date, default_value = "2025-03-21")]
        date: NaiveDate,
    },
    /// Get PRs waiting the longest for review
    PrsWaitingForReview {
        #[arg(short, long, value_parser = parse_date, default_value = "2025-03-21")]
        date: NaiveDate,
    },
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
