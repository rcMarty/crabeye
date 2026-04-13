pub mod api;
pub mod commands;
pub mod config;
pub mod db;
pub mod git;
mod misc;
pub mod monitoring;

use crate::api::app_state::AppState;
use crate::commands::{Cli, Commands};
use crate::config::Config;
use crate::db::Database;
use crate::git::SyncHandler;
use crate::monitoring::state_tracker::StateMonitor;
use aide::axum::ApiRouter;
use axum::Extension;
use clap::Parser;
use indicatif::MultiProgress;
use indicatif_log_bridge::LogWrapper;
use log::LevelFilter;
use std::sync::Arc;

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
        Commands::SyncAll { sync, full_history } => {
            let handler =
                SyncHandler::init(config.repo_name, config.repo_owner, config.github_token, db);

            let sync = sync.unwrap_or(git::github::SyncMode::Last(10));
            if full_history.unwrap_or(false) {
                log::info!("Syncing with full history. This will take much longer but will give you more data for analysis.");
                handler.sync_with_full_info(sync).await?;
            } else {
                log::info!("Syncing without full history. This will be faster but will give you less data for analysis.");
                handler.sync_without_history(sync).await?;
            }

            log::info!("Sync is completed");

            log::info!("Press enter to exit...");
            let mut input = String::new();
            std::io::stdin()
                .read_line(&mut input)
                .expect("Failed to read line");
            log::info!("Exiting...");
        }
        Commands::Serve => {
            // spawn the task to get new data every minute
            let handler =
                SyncHandler::init(config.repo_name, config.repo_owner, config.github_token, db);
            let state_tracker = StateMonitor::new(std::time::Duration::from_secs(config.check_interval_secs));

            // set up and run the API server
            let mut api = aide::openapi::OpenApi::default();

            let cors_layer = tower_http::cors::CorsLayer::new()
                .allow_origin(tower_http::cors::Any)
                .allow_methods(tower_http::cors::Any)
                .allow_headers(tower_http::cors::Any);

            let router = ApiRouter::new()
                .nest_api_service("/api/pr", api::review::pr_routes(state.clone()))
                .nest_api_service("/api/issue", api::issues::issues_routes(state.clone()))
                .nest_api_service("/api/teams", api::teams::teams_routes(state.clone()))
                .nest_api_service("/docs", api::docs::docs_routes(state.clone()))
                .route("/health", axum::routing::get(|| async { "OK" }))
                .finish_api_with(&mut api, api::docs::api_docs)
                .layer(cors_layer)
                .layer(Extension(Arc::new(api)))
                .with_state(state);

            let listener = tokio::net::TcpListener::bind("0.0.0.0:7878").await?;
            log::info!("serving API on URL: http://localhost:7878/docs");

            // run both the state tracker and the API server
            tokio::select! {
                _ = state_tracker.run(&handler,"rust-lang/rust") => {
                    log::info!("State tracker task ended");
            }
                res = axum::serve(listener, router) => {
                    if let Err(e) = res {
                        log::error!("API server error: {:?}", e);
                    }
                }
            }
        }
        Commands::Backfill => {
            let handler =
                SyncHandler::init(config.repo_name, config.repo_owner, config.github_token, db);
            handler.backfill_missing_history().await?;
            log::info!("Backfill is completed");
        }
    }

    Ok(())
}
