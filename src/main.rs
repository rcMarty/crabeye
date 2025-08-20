pub mod api;
pub mod commands;
pub mod config;
pub mod db;
pub mod git;
pub mod monitoring;
mod misc;

use crate::commands::{Cli, Commands};
use crate::config::Config;
use crate::db::model::pr_event::PullRequestStatus;
use crate::db::Database;
use crate::git::Analyze;
use chrono::{DateTime, NaiveTime};
use clap::Parser;
use indicatif::MultiProgress;
use indicatif_log_bridge::LogWrapper;
use log::LevelFilter;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::extract::{Query, State};
use axum::{Json, Router};
use reqwest::StatusCode;

lazy_static::lazy_static! {static ref MULTI_PROGRESS_BAR: MultiProgress = MultiProgress::new();}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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
                db,
            );
            analyze.analyze().await?;
            log::info!("Analyze is completed");
        }
        Commands::Serve => {
            log::info!("serving API");
            let router = Router::new()
                // root hello world
                .route("/", get(made_review))
                .with_state(db);

            let listener = tokio::net::TcpListener::bind("0.0.0.0:7878").await?;
            axum::serve(listener, router).await?;
        }
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

async fn made_review(State(db): State<Database>, Query(params): Query<ReviewParams>) -> Result<Json<Vec<i64>>, (StatusCode, String)> {
    log::debug!("{:?}", params.clone());
    let res = db.get_users_who_modified_file(params.file, params.from_date, params.last_n_days).await;

    match res {
        Ok(values) => {
            Ok(Json(values))
        }
        Err(err) => {
            Err((StatusCode::INTERNAL_SERVER_ERROR, format!("{:?}", err)))
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct ReviewParams {
    file: String,
    last_n_days: Option<i64>,
    from_date: Option<chrono::NaiveDateTime>,
}

#[derive(serde::Deserialize)]
pub struct Pagination {
    skip: Option<i32>,
    page: Option<i32>,
}