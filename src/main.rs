mod api;
mod commands;
mod config;
mod db;
mod git;
mod monitoring;

use crate::commands::{Cli, Commands, RequestSubcommands};
use crate::config::Config;
use crate::db::model::pr_event::PullRequestStatus;
use crate::db::Database;
use crate::git::Analyze;
use chrono::DateTime;
use clap::Parser;
use dotenvy::dotenv;
use indicatif::MultiProgress;
use indicatif_log_bridge::LogWrapper;
use log::LevelFilter;
use std::env;

lazy_static::lazy_static! {static ref MULTI_PROGRESS_BAR: MultiProgress = MultiProgress::new();}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // dotenv().ok();
    let config = Config::from_env()?;
    let logger = env_logger::builder()
        .format_timestamp_millis()
        .filter_level(config.log_level.parse().unwrap_or(LevelFilter::Info))
        .build();

    LogWrapper::new(MULTI_PROGRESS_BAR.clone(), logger).try_init()?;

    log::info!("Hello, world!");
    log::debug!("Config is: {:#?}", config);

    let db = Database::new(config.db_url.as_str()).await?;

    let cli = Cli::parse();
    match cli.command {
        Commands::Analyze => {
            let analyze = Analyze::init(
                config.repo_name,
                config.repo_owner,
                config.github_token,
                db.clone(),
            );
            analyze.analyze().await?;
            log::info!("Analyze is completed");
        }
        Commands::Request { subcommand } => match subcommand {
            RequestSubcommands::PrStateAt { pr_number, date } => {
                let timestamp = date.and_hms_opt(0, 0, 0).unwrap().and_utc();
                let result = db.get_pr_state_at(pr_number, timestamp).await?;
                log::info!("PR State At: {:?}", result);
            }
            RequestSubcommands::PrCountInState { state, date } => {
                let timestamp = date.and_hms_opt(0, 0, 0).unwrap().and_utc();
                let pr_state = PullRequestStatus::from_str(&state, timestamp, None)
                    .ok_or_else(|| anyhow::anyhow!("Invalid state"))?;
                let count = db.get_pr_count_in_state(timestamp, pr_state).await?;
                log::info!("PR Count In State: {}", count);
            }
            RequestSubcommands::TopNFiles {
                user_id,
                days,
                n,
            } => {
                log::debug!("last: {} days", days);
                let files = db.get_top_n_files(user_id, days, n).await?;
                log::info!("Top N Files: {:#?}", files);
            }
            RequestSubcommands::UsersWhoModifiedFile { file_path, date } => {
                let timestamp = date.and_hms_opt(0, 0, 0).unwrap().and_utc();
                let users = db.get_users_who_modified_file(file_path, timestamp).await?;
                log::info!("Users Who Modified File: {:?}", users);
            }
            RequestSubcommands::PrsWaitingForReview { date } => {
                let timestamp = date.and_hms_opt(0, 0, 0).unwrap().and_utc();
                let prs = db.get_prs_waiting_for_review(timestamp).await?;
                log::info!("PRs Waiting For Review: {:?}", prs);
            }
        },
    }

    // wait for user input
    log::info!("Press enter to exit...");
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .expect("Failed to read line");
    log::info!("Exiting...");

    Ok(())
}
