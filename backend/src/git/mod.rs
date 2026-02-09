use crate::db::model::pr_event::{FileActivity, PullRequestStatus};
use crate::db::Database;
use crate::git::git::Repo;
use crate::git::github::{GitHubApi, SyncMode};
use crate::misc::with_progress_bar_async;
use chrono::{DateTime, Utc};
use git2::Oid;
use indicatif::ProgressBar;
use octocrab::params::State;
use secrecy::SecretString;
use std::path::Path;

pub mod git;
pub mod github;

pub struct Analyze {
    pub repo: Repo,
    github: GitHubApi,
    pub database: Database,
}

impl Analyze {
    pub fn init(
        repository_name: String,
        owner: String,
        token: SecretString,
        database: Database,
    ) -> Self {
        let repo = Repo::init(
            Analyze::url(repository_name.clone(), owner.clone()).as_str(),
            Path::new(&format!("./test_repos/{}", repository_name.as_str())),
        )
            .unwrap();
        let github = GitHubApi::new(owner, repository_name, token).unwrap();
        Self {
            repo,
            github,
            database,
        }
    }

    fn url(repository_name: String, owner: String) -> String {
        "https://github.com/".to_owned() + owner.as_str() + "/" + repository_name.as_str()
    }

    fn log_duration(start: DateTime<Utc>, end: DateTime<Utc>, message: &str) {
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
    }

    pub async fn analyze(&self, sync_mode: SyncMode) -> anyhow::Result<()> {
        let overall_time = Utc::now();
        //users section
        log::info!("Getting users from github");
        let users = self
            .github
            .get_authorized_users()
            .await
            .expect("Failed to get users");

        log::info!("number of found users: {}", users.len());

        let timestamp_start = Utc::now();
        if let Err(res) = self.database.upsert_team_members(&users).await {
            log::error!("Error: {:?}", res);
        }
        Self::log_duration(timestamp_start, Utc::now(), "Upserting users from github: ");


        self.analyze_prs(sync_mode.clone()).await?;
        // self.analyze_issues(sync_mode).await?;

        Self::log_duration(overall_time, Utc::now(), "Overall getting resources: ");
        Ok(())
    }
    pub async fn analyze_prs(&self, sync_mode: SyncMode) -> anyhow::Result<()> {
        let mut timestamp_start = Utc::now();

        // pr section
        let (prs, contributors) = self.github.get_pull_requests(State::All, sync_mode).await?;
        Self::log_duration(timestamp_start, Utc::now(), "Getting pull requests: ");

        // insert non existing contributors
        log::info!(
            "Inserting contributors to database ({} found)",
            contributors.len()
        );
        timestamp_start = Utc::now();
        self.database.upsert_contributors(&contributors).await?;
        Self::log_duration(timestamp_start, Utc::now(), "Inserting contributors: ");


        timestamp_start = Utc::now();

        // process PRs and its files and contributors
        with_progress_bar_async(
            prs.len(),
            "Processing prs".parse()?,
            async |bar: &ProgressBar| {
                for pr in prs.iter() {
                    bar.inc(1);
                    bar.set_message(format!("Processing PR #{}", pr.pr_number));
                    log::debug!("{}", "_".repeat(69));
                    log::debug!("{:#?}", pr);
                    if let Err(res) = self.database.insert_pr_event(pr).await {
                        log::error!("Error when inserting pr event to database: {:?}", res);
                    }

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

                    let file_activities: Vec<FileActivity> = files
                        .iter()
                        .map(|file| FileActivity {
                            pr: pr.pr_number,
                            file_path: file.clone(),
                            user_id: pr.author_id,
                            timestamp: pr.get_timestamp(),
                        })
                        .collect();
                    if let Err(res) = self.database.insert_file_activities(&file_activities).await {
                        log::error!("Error: {:?}", res);
                    }
                }
                Ok(())
            },
        )
            .await?;

        Self::log_duration(timestamp_start, Utc::now(), "Inserting to database: ");
        Ok(())
    }

    pub async fn analyze_issues(&self, sync_mode: SyncMode) -> anyhow::Result<()> {
        let mut timestamp_start = Utc::now();

        // pr section
        let (issues, contributors) = self.github.get_issues(State::All, sync_mode).await?;
        Self::log_duration(timestamp_start, Utc::now(), "Getting issues: ");

        // insert non existing contributors
        log::info!(
            "Upserting contributors to database ({} found)",
            contributors.len()
        );
        timestamp_start = Utc::now();
        self.database.upsert_contributors(&contributors).await?;
        Self::log_duration(timestamp_start, Utc::now(), "Inserting contributors: ");


        timestamp_start = Utc::now();

        // process PRs and its files and contributors
        with_progress_bar_async(
            issues.len(),
            "Processing".parse()?,
            async |bar: &ProgressBar| {
                for issue in issues.iter() {
                    bar.inc(1);
                    bar.set_message(format!("Processing issue #{}", issue.issue_number));
                    log::debug!("{}", "_".repeat(69));
                    log::debug!("{:#?}", issue);
                    // if let Err(res) = self.database.insert_issue_event(issue).await {
                    //     log::error!("Error when inserting issue event to database: {:?}", res);
                    // }
                }
                Ok(())
            },
        )
            .await?;

        Self::log_duration(timestamp_start, Utc::now(), "Inserting to database: ");
        Ok(())
    }
}
