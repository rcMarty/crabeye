mod api;
mod config;
mod db;
mod git;
mod monitoring;

use crate::db::Database;
use anyhow::Context;
use git2::{Error, Oid, Repository};
use octocrab::models::issues::Issue;
use octocrab::params::State::Closed;
use octocrab::{models, params, Octocrab};
use secrecy::SecretString;
use sqlx::sqlite::SqlitePoolOptions;
use std::collections::HashSet;
use std::fmt::format;
use dotenvy::dotenv;
use log::LevelFilter;
use log::LevelFilter::Debug;
use crate::config::Config;
use crate::git::Analyze;
use crate::git::github::GitHubApi;

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