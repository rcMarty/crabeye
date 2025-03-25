mod api;
mod config;
mod db;
mod git;
mod monitoring;

use crate::config::Config;
use crate::db::model::pr_event::PullRequestStatus;
use crate::db::Database;
use crate::git::Analyze;
use dotenvy::dotenv;
use indicatif::MultiProgress;
use indicatif_log_bridge::LogWrapper;
use log::LevelFilter;

lazy_static::lazy_static! {static ref MULTI_PROGRESS_BAR: MultiProgress = MultiProgress::new();}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    let config = Config::from_env()?;
    let logger = env_logger::builder()
        .format_timestamp_millis()
        .filter_level(config.log_level.parse().unwrap_or(LevelFilter::Info))
        .build();

    LogWrapper::new(MULTI_PROGRESS_BAR.clone(), logger).try_init()?;

    log::info!("Hello, world!");
    log::debug!("Config is: {:#?}", config);

    let db = Database::new(config.db_url.as_str()).await?;

    // get all prs and matching files from repo and insert them into db
    let analyze = Analyze::init(
        config.repo_name,
        config.repo_owner,
        config.github_token,
        db.clone(),
    );
    analyze.analyze().await?;

    // test for get_pr_state_at function
    let timestamp =
        chrono::DateTime::parse_from_rfc3339("2025-03-21T00:00:00Z")?.with_timezone(&chrono::Utc);
    let ret = db.get_pr_state_at(138694, timestamp).await?;
    log::info!("Result: {:?}", ret);

    // test for get count of prs in concrete state and concrete day
    let ret2 = db
        .get_pr_count_in_state(
            timestamp,
            PullRequestStatus::Open {
                time: Default::default(),
            },
        )
        .await?;
    log::info!("Result2: {:?}", ret2);

    Ok(())
}
