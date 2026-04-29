use crate::git::{SyncHandler, SyncMode};
use chrono::{NaiveDateTime, Utc};
use std::time::Duration;
use tokio::time;

pub struct StateMonitor {
    interval: Duration,
    /// how far back in time to look for events if database is empty
    lookback_period: chrono::Duration,
}

impl StateMonitor {
    fn repository_identifier(repository_name: String, owner: String) -> String {
        owner + "/" + repository_name.as_str()
    }

    pub fn new(interval: Duration, lookback_period: chrono::Duration) -> Self {
        Self {
            interval,
            lookback_period,
        }
    }

    pub async fn run(
        self,
        analyze: &SyncHandler,
        repository_owner: String,
        repository_name: String,
    ) {
        let repo = Self::repository_identifier(repository_name, repository_owner.clone());
        let mut interval = time::interval(self.interval);
        loop {
            interval.tick().await;
            log::info!("_________________________________________________________________");
            log::info!("Starting state tracking iteration");

            // fetch new data from git repository
            if let Err(e) = analyze.update_repository() {
                log::error!("Error updating git repository: {:?}", e);
                continue;
            } else {
                log::info!("Git repository updated successfully");
            }

            // download form github
            let from: NaiveDateTime = match analyze.timestamp_of_last_event(repo.as_str()).await {
                Ok(Some(ts)) => ts,
                Ok(None) => {
                    log::info!("No previous PR events found, starting from the beginning");
                    (Utc::now() - self.lookback_period).naive_utc()
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
