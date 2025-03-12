mod api;
mod config;
mod db;
mod git;
mod monitoring;

use crate::db::Database;
use dotenvy::dotenv;

use log::LevelFilter::Debug;
use crate::config::Config;
use crate::git::Analyze;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    let config = Config::from_env()?;
    env_logger::builder()
        .format_timestamp_millis()
        .filter_level(Debug)
        .init();

    log::info!("Hello, world!");

    let db = Database::new(config.db_url.as_str()).await?;

    let analyze = Analyze::init(
        config.repo_name,
        config.repo_owner,
        config.github_token,
        db,
    );

    analyze.analyze().await?;

    Ok(())
}