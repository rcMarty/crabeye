use crate::db::model::pr_event::{FileActivity, PullRequestStatus};
use crate::db::Database;
use crate::git::git::Repo;
use crate::git::github::{GitHubApi, SyncMode};
use crate::misc::with_progress_bar_async;
use crate::MULTI_PROGRESS_BAR;
use chrono::{DateTime, Utc};
use git2::Oid;
use indicatif::ProgressBar;
use octocrab::params::State;
use secrecy::SecretString;
use std::collections::HashSet;
use std::path::Path;

pub mod git;
pub mod github;

pub struct Analyze {
    repo: Repo,
    github: GitHubApi,
    database: Database,
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
        let duration_secs = end.signed_duration_since(start).num_seconds();
        let format = match duration_secs {
            0..60 => format!("{} seconds", duration_secs),
            60..3600 => format!(
                "{} minutes {} seconds",
                duration_secs / 60,
                duration_secs % 60
            ),
            _ => format!(
                "{} hours {} minutes {} seconds",
                duration_secs / 3600,
                (duration_secs % 3600) / 60,
                duration_secs % 60
            ),
        };
        log::info!("{} took: {}", message, format);
    }

    pub async fn analyze(&self, sync_mode: SyncMode) -> anyhow::Result<()> {
        let mut timestamp_start = Utc::now();
        let overall_time = timestamp_start;
        //users section
        log::info!("Getting users from github");
        let users = self
            .github
            .get_authorized_users()
            .await
            .expect("Failed to get users");

        log::info!("number of found users: {}", users.len());

        if let Err(res) = self.database.insert_team_members(&users).await {
            log::error!("Error: {:?}", res);
        }

        Self::log_duration(timestamp_start, Utc::now(), "Getting users from github: ");
        timestamp_start = Utc::now();

        // pr section
        // TODO hardcoded number of pages
        let (prs, contributors) = self.github.get_pull_requests(State::All, sync_mode).await?;
        Self::log_duration(timestamp_start, Utc::now(), "Getting pull requests: ");
        timestamp_start = Utc::now();

        // insert non existing contributors
        log::info!(
            "Inserting contributors to database ({} found)",
            contributors.len()
        );
        self.database.upsert_contributors(&contributors).await?;

        with_progress_bar_async(
            prs.len(),
            "Processing".parse()?,
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

                    for file in files {
                        let activity = FileActivity {
                            pr: pr.pr_number,
                            file_path: file,
                            user_id: pr.author_id,
                            timestamp: pr.get_timestamp(),
                        };
                        if let Err(res) = self.database.insert_file_activity(&activity).await {
                            log::error!("Error: {:?}", res);
                        }
                    }
                }
                Ok(())
            },
        )
        .await?;

        Self::log_duration(timestamp_start, Utc::now(), "Inserting to database: ");
        Self::log_duration(overall_time, Utc::now(), "Overall getting resources: ");
        Ok(())
    }
}
