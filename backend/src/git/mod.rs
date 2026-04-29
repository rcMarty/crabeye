use crate::db::model::pr_event::{FileActivity, PullRequestStatus};
use crate::db::Database;
use crate::git::git::Repo;
use crate::git::github::GitHubApi;
use crate::progress::{multi_progress_bar as shared_multi_progress_bar, with_progress_bar_async};
use chrono::{DateTime, NaiveDateTime, Utc};
use git2::Oid;
use indicatif::MultiProgress;
use octocrab::params::State;
use secrecy::SecretString;
use std::path::Path;
use std::sync::Mutex;

pub(crate) mod git;
mod github;

pub use github::SyncMode;

pub struct SyncHandler {
    repo: Repo,
    github: GitHubApi,
    database: Database,
    log_messages: Mutex<Vec<String>>,
}

/// Public functions for SyncHanler struct
impl SyncHandler {
    pub fn init(
        repository_name: String,
        owner: String,
        token: SecretString,
        database: Database,
    ) -> Self {
        let repo = Repo::init(
            Self::repository_identifier(repository_name.clone(), owner.clone()),
            Self::url(repository_name.clone(), owner.clone()).as_str(),
            Path::new(&format!("./test_repos/{}", repository_name.as_str())),
        )
        .unwrap();
        let github = GitHubApi::new(
            Self::repository_identifier(repository_name.clone(), owner.clone()),
            owner,
            repository_name,
            token,
            database.clone(),
        )
        .unwrap();
        Self {
            repo,
            github,
            database,
            log_messages: Mutex::new(vec![]),
        }
    }

    pub async fn sync_with_full_info(&self, sync_mode: SyncMode) -> anyhow::Result<()> {
        let overall_time = Utc::now();
        //users section
        log::info!("Getting users from rust teams");
        let (teams, users) = self
            .github
            .get_authorized_users()
            .await
            .expect("Failed to get users from rust teams");

        log::info!("number of found rust teams users: {}", users.len());

        let timestamp_start = Utc::now();
        if let Err(res) = self.database.upsert_team_members(&teams, &users).await {
            log::error!("Error: {:?}", res);
        }
        self.log_duration(
            timestamp_start,
            Utc::now(),
            "Upserting users from rust teams: ",
        );

        self.sync_pull_requests(sync_mode.clone(), true).await?;
        self.sync_issues(sync_mode, true).await?;

        self.log_messages
            .lock()
            .unwrap()
            .iter()
            .for_each(|msg| log::info!("{}", msg));
        self.log_duration(overall_time, Utc::now(), "Overall getting resources: ");
        self.log_messages.lock().unwrap().clear();
        Ok(())
    }

    pub async fn sync_without_history(&self, sync_mode: SyncMode) -> anyhow::Result<()> {
        let overall_time = Utc::now();
        //users section
        log::info!("Getting users from rust teams");
        let (teams, users) = self
            .github
            .get_authorized_users()
            .await
            .expect("Failed to get users from rust teams");

        log::info!("number of found rust teams users: {}", users.len());

        let timestamp_start = Utc::now();
        if let Err(res) = self.database.upsert_team_members(&teams, &users).await {
            log::error!("Error: {:?}", res);
        }
        self.log_duration(
            timestamp_start,
            Utc::now(),
            "Upserting users from rust teams: ",
        );

        self.sync_pull_requests(sync_mode.clone(), false).await?;
        self.sync_issues(sync_mode, false).await?;

        self.log_messages
            .lock()
            .unwrap()
            .iter()
            .for_each(|msg| log::info!("{}", msg));
        self.log_duration(overall_time, Utc::now(), "Overall getting resources: ");
        Ok(())
    }

    pub async fn backfill_missing_history(&self) -> anyhow::Result<()> {
        let mut must_be_backfilled = self.database.get_issues_without_history().await?;
        log::info!(
            "Found {} PR events without history, starting backfilling",
            must_be_backfilled.len()
        );

        let timestamp_start = Utc::now();
        let mut duration_backfill = chrono::Duration::zero();
        let mut duration_db = chrono::Duration::zero();

        for issue_chunk in must_be_backfilled.chunks_mut(100) {
            let backfill_start = Utc::now();
            self.github.process_backfill(issue_chunk).await;
            duration_backfill += Utc::now().signed_duration_since(backfill_start);

            let db_start = Utc::now();
            self.database.insert_history(issue_chunk).await?;
            duration_db += Utc::now().signed_duration_since(db_start);
        }

        let timestamp_end = Utc::now();

        self.log_duration(
            timestamp_end - duration_backfill,
            timestamp_end,
            "Backfill duration (fetching history from github and processing it)",
        );
        self.log_duration(
            timestamp_end - duration_db,
            timestamp_end,
            "Backfill duration (inserting history to database)",
        );
        self.log_duration(
            timestamp_start,
            timestamp_end,
            "Backfilling history for issues: ",
        );

        Ok(())
    }

    pub async fn timestamp_of_last_event(
        &self,
        repo: &str,
    ) -> anyhow::Result<Option<NaiveDateTime>> {
        self.database.get_last_issue_event_timestamp(repo).await
    }

    pub fn update_repository(&self) -> anyhow::Result<()> {
        self.repo.update()
    }
}

pub fn multi_progress_bar() -> &'static MultiProgress {
    shared_multi_progress_bar()
}

/// Private functions and helper methods for Analyze struct
impl SyncHandler {
    fn url(repository_name: String, owner: String) -> String {
        "https://github.com/".to_owned() + owner.as_str() + "/" + repository_name.as_str()
    }

    fn repository_identifier(repository_name: String, owner: String) -> String {
        owner + "/" + repository_name.as_str()
    }

    fn log_duration(&self, start: DateTime<Utc>, end: DateTime<Utc>, message: &str) {
        let duration_ms = end.signed_duration_since(start).num_milliseconds();
        let duration_secs = duration_ms / 1000;
        let ms = duration_ms % 1000;
        let format = match duration_secs {
            0..60 => format!("{}.{:03} seconds", duration_secs, ms),
            60..3600 => format!(
                "{} minutes {}.{:03} seconds",
                duration_secs / 60,
                duration_secs % 60,
                ms
            ),
            _ => format!(
                "{} hours {} minutes {}.{:03} seconds",
                duration_secs / 3600,
                (duration_secs % 3600) / 60,
                duration_secs % 60,
                ms
            ),
        };
        log::info!("{} took: {}", message, format);
        self.log_messages
            .lock()
            .unwrap()
            .push(format!("{} took: {}", message, format));
    }

    async fn sync_pull_requests(
        &self,
        sync_mode: SyncMode,
        with_timeline: bool,
    ) -> anyhow::Result<()> {
        let mut timestamp_start = Utc::now();

        // pr section
        let (prs, contributors) = self
            .github
            .get_pull_requests(State::All, sync_mode, with_timeline)
            .await?;
        self.log_duration(
            timestamp_start,
            Utc::now(),
            "Getting pull requests from github: ",
        );
        log::debug!("found {} prs", prs.len());

        // insert non existing contributors
        log::info!(
            "Inserting contributors to database ({} found)",
            contributors.len()
        );
        timestamp_start = Utc::now();
        self.database.upsert_contributors(&contributors).await?;
        self.log_duration(
            timestamp_start,
            Utc::now(),
            "Inserting contributors from pull requests: ",
        );

        timestamp_start = Utc::now();
        // Collect all file activities up-front so we can issue one bulk insert per
        // table (PRs + file_activity) instead of two queries per PR.
        let all_file_activities: Mutex<Vec<FileActivity>> = Mutex::new(Vec::new());

        with_progress_bar_async(
            prs.len(),
            Some("Processing prs and computing file activity".parse()?),
            async |bar_opt, _multi: &MultiProgress| {
                let bar = bar_opt.unwrap();
                for pr in prs.iter() {
                    bar.inc(1);
                    bar.set_message(format!("Processing PR #{}", pr.pr_number));

                    let sha = match &pr.state {
                        PullRequestStatus::Merged { merge_sha, time: _ } => merge_sha,
                        _ => continue,
                    };

                    //obtain modified files
                    let files = match self
                        .repo
                        .modified_files(Oid::from_str(sha.as_str()).unwrap_or(Oid::zero()))
                    {
                        Ok(Some(files)) => files,
                        Ok(None) => {
                            log::warn!("No modified files found for commit {}", sha);
                            continue;
                        }
                        Err(e) => {
                            log::error!(
                                "Error while getting modified files for commit {}: {:#?}",
                                sha,
                                e
                            );
                            continue;
                        }
                    };

                    let mut buf = all_file_activities.lock().unwrap();
                    buf.extend(files.into_iter().map(|file| FileActivity {
                        repository: self.repo.repository_identifier().to_owned(),
                        pr: pr.pr_number,
                        file_path: file,
                        user_id: pr.author_id,
                        timestamp: pr.get_edited_at(),
                    }));
                }
                Ok(())
            },
        )
        .await?;
        self.log_duration(
            timestamp_start,
            Utc::now(),
            "Processing pull requests and computing file activity: ",
        );

        // Bulk insert PRs (chunked internally by the database layer).
        timestamp_start = Utc::now();
        if let Err(res) = self.database.insert_pr_events(&prs).await {
            log::error!("Error when bulk-inserting PR events to database: {:?}", res);
        }
        self.log_duration(
            timestamp_start,
            Utc::now(),
            "Bulk-inserting pull requests to database: ",
        );

        // Bulk insert file activities (chunked internally by the database layer).
        timestamp_start = Utc::now();
        let file_activities = all_file_activities.into_inner().unwrap();
        log::info!(
            "Bulk-inserting {} file activity rows",
            file_activities.len()
        );
        if let Err(res) = self.database.insert_file_activities(&file_activities).await {
            log::error!("Error when bulk-inserting file activities: {:?}", res);
        }
        self.log_duration(
            timestamp_start,
            Utc::now(),
            "Bulk-inserting file activities to database: ",
        );

        Ok(())
    }

    async fn sync_issues(&self, sync_mode: SyncMode, with_timeline: bool) -> anyhow::Result<()> {
        let mut timestamp_start = Utc::now();

        // pr section
        let (issues, contributors) = self
            .github
            .get_issues(State::All, sync_mode, with_timeline)
            .await?;
        log::debug!("found {} issues", issues.len());
        self.log_duration(timestamp_start, Utc::now(), "Getting issues from github: ");

        // insert non existing contributors
        log::info!(
            "Upserting contributors to database ({} found)",
            contributors.len()
        );
        timestamp_start = Utc::now();
        self.database.upsert_contributors(&contributors).await?;
        self.log_duration(
            timestamp_start,
            Utc::now(),
            "Inserting contributors from issues to database: ",
        );

        timestamp_start = Utc::now();
        if let Err(res) = self.database.insert_issues(issues.as_slice()).await {
            log::error!("Error when inserting issue event to database: {:?}", res);
        }
        self.log_duration(
            timestamp_start,
            Utc::now(),
            "Inserting issues to database: ",
        );

        Ok(())
    }
}
