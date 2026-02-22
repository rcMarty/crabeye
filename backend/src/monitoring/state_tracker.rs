use crate::git::github::SyncMode;
use crate::git::SyncHandler;
use chrono::{NaiveDateTime, Utc};
use std::time::Duration;
use tokio::time;

pub struct StateMonitor {
    interval: Duration,
}

impl StateMonitor {
    pub fn new(interval: Duration) -> Self {
        Self { interval }
    }

    pub async fn run(self, analyze: &SyncHandler, repo: &str) {
        let mut interval = time::interval(self.interval);
        loop {
            interval.tick().await;
            log::info!("_________________________________________________________________");
            log::info!("Starting state tracking iteration");

            // fetch new data from git repository
            if let Err(e) = analyze.repo.update() {
                log::error!("Error updating git repository: {:?}", e);
                continue;
            } else {
                log::info!("Git repository updated successfully");
            }

            // download form github
            let from: NaiveDateTime = match analyze.timestamp_of_last_event(repo).await {
                Ok(Some(ts)) => ts,
                Ok(None) => {
                    log::info!("No previous PR events found, starting from the beginning");
                    (Utc::now() - chrono::Duration::days(720)).naive_utc()
                }
                Err(e) => {
                    log::error!("Error retrieving last PR event timestamp: {:?}", e);
                    continue;
                }
            };
            if let Err(e) = analyze.sync_with_full_info(SyncMode::Since(from)).await {
                log::error!("Error during state tracking: {:?}", e);
            } else {
                log::info!("State tracking iteration completed successfully");
            }
        }
    }
}
