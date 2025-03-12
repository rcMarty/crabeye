use crate::db::model::pr_event::{FileActivity, PullRequestStatus};
use crate::db::Database;
use crate::git::git::Repo;
use crate::git::github::GitHubApi;
use git2::Oid;
use octocrab::params::State::All;
use secrecy::SecretString;
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
            Path::new("./test_repos"),
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
        "https://github.com/".to_owned() + repository_name.as_str() + "/" + owner.as_str()
    }

    pub async fn analyze(&self) -> anyhow::Result<()> {
        let prs = self.github.get_all_pull_requests(All).await?;
        for pr in prs {
            log::debug!("{:#?}", pr);
            if let Err(res) = self.database.insert_pr_event(&pr).await {
                log::error!("Error: {:?}", res);
            }

            let sha = match pr.state {
                PullRequestStatus::Merged { merge_sha } => merge_sha,
                _ => {
                    log::warn!("PR #{} is not merged", pr.pr_number);
                    continue;
                }
            };

            let files = self.repo.modified_files(Oid::from_str(sha.as_str())?);

            if files.is_err() {
                log::error!("Error while getting modified files: {:#?}", files);
            };

            let files = files?;

            if files.is_none() {
                log::warn!("No modified files found");
                continue;
            }

            let files = files.unwrap();
            for file in files {
                let activity = FileActivity {
                    pr: pr.pr_number,
                    file_path: file,
                    user_login: "reee".to_string(),
                    timestamp: pr.timestamp,
                };
                if let Err(res) = self.database.insert_file_activity(&activity).await {
                    log::error!("Error: {:?}", res);
                }
            }
        }

        Ok(())
    }
}
