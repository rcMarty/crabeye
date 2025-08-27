pub mod api;
pub mod commands;
pub mod config;
pub mod db;
pub mod git;
pub mod monitoring;
mod misc;

use std::sync::Arc;
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
use axum::extract::{Query, State};
use axum::{Extension, Json};
use reqwest::StatusCode;
use aide::{axum::{ApiRouter, IntoApiResponse, routing::{get, post}}};
use schemars::JsonSchema;
use crate::api::app_state::AppState;

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
    let state = AppState { db: db.clone() };

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

            log::info!("Press enter to exit...");
            let mut input = String::new();
            std::io::stdin()
                .read_line(&mut input)
                .expect("Failed to read line");
            log::info!("Exiting...");
        }
        Commands::Serve => {
            let mut api = aide::openapi::OpenApi::default();
            let router = ApiRouter::new()
                .nest_api_service("/api", api::review::review_routes(state.clone()))
                .nest_api_service("/docs", api::docs::docs_routes(state.clone()))
                .finish_api_with(&mut api, api::docs::api_docs)
                .layer(Extension(Arc::new(api)))
                .with_state(state);

            let listener = tokio::net::TcpListener::bind("0.0.0.0:7878").await?;
            log::info!("serving API on listener: {listener:?}");
            axum::serve(listener, router).await?;
        }
    }

    Ok(())
}


