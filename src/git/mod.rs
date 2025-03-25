use crate::db::model::pr_event::{FileActivity, PullRequestStatus};
use crate::db::Database;
use crate::git::git::Repo;
use crate::git::github::GitHubApi;
use crate::MULTI_PROGRESS_BAR;
use git2::Oid;
use octocrab::params::State;
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

    pub async fn analyze(&self) -> anyhow::Result<()> {
        // TODO hardcoded number of pages
        let prs = self.github.get_pull_requests(State::Closed, 100).await?;

        // proggress bar
        let multi = MULTI_PROGRESS_BAR.clone();
        let bar = multi.add(indicatif::ProgressBar::new(prs.len() as u64));
        bar.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}")?
                .progress_chars("##-"),
        );

        // process prs
        for pr in prs.iter() {
            bar.inc(1);
            bar.set_message(format!("Processing PR #{}", pr.pr_number));
            log::debug!("{}", "_".repeat(69));
            log::debug!("{:#?}", pr);
            if let Err(res) = self.database.insert_pr_event(pr).await {
                log::error!("Error: {:?}", res);
            }

            let sha = match &pr.state {
                PullRequestStatus::Merged { merge_sha, time } => merge_sha,
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
                    user_id: pr.author_id,
                    timestamp: pr.get_timestamp(),
                };
                if let Err(res) = self.database.insert_file_activity(&activity).await {
                    log::error!("Error: {:?}", res);
                }
            }
        }
        bar.finish_with_message("Finished processing PRs");
        multi.remove(&bar);
        Ok(())
    }
}
