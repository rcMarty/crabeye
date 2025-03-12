#![allow(unused)]
use crate::db::model::pr_event::{PrEvent, PullRequestStatus};
use anyhow::Context;
use chrono::{DateTime, Utc};
use log::log;
use octocrab::models::issues::Issue;
use octocrab::models::IssueState;
use octocrab::{params, Octocrab};
use secrecy::SecretString;

pub struct GitHubApi {
    owner: String,
    repository: String,
    octocrab: Octocrab,
}

impl GitHubApi {
    /// Create a new GitHubApi instance
    /// * token - GitHub personal access token
    pub fn new(owner: String, repository: String, token: SecretString) -> anyhow::Result<Self> {
        let octocrab = Octocrab::builder().personal_token(token).build()?;
        // let _ = octocrab::initialise(octocrab); // for whatever reason this doesnt return octocrab with auth
        // let octocrab = octocrab::instance(); //this is needed to return right octocrab with auth

        Ok(Self {
            owner,
            repository,
            octocrab,
        })
    }

    pub async fn get_issues(&self, name: String, state: params::State) -> anyhow::Result<Vec<Issue>> {
        let page = self
            .octocrab
            .issues(self.owner.clone(), self.repository.clone())
            .list()
            .creator(name)
            .state(state)
            .per_page(100)
            .send()
            .await?;

        println!(
            "Found {} issues, no of pages: {:?}, total count: {:?}",
            page.items.len(),
            page.number_of_pages(),
            page.total_count
        );

        let result = self.octocrab.get_page::<Issue>(&page.next).await?;
        println!(
            "result: {:?}",
            result.clone().unwrap().items.first().unwrap().url
        );
        let issues = result.unwrap().items;

        for issue in issues.iter() {
            println!(
                "#{}: {} {}\nauthor(s): {:?}\nlabels: {:?}\n",
                issue.number,
                issue.title,
                issue.body_text.clone().unwrap_or("No body".to_string()),
                issue.user.login,
                issue.labels,
            );
        }
        Ok(issues)
    }

    pub async fn get_all_pull_requests(&self, state: params::State) -> anyhow::Result<Vec<PrEvent>> {
        let pr = self
            .octocrab
            .pulls(self.owner.clone(), self.repository.clone())
            .list()
            .state(state)
            .per_page(100)
            .send()
            .await
            .context(format!(
                "Failed to get pull requests for {}/{}",
                self.owner, self.repository
            ))?;

        log::info!(
            "Found less than {} pull requests, no of pages: {:?}",
            pr.items.len() * pr.number_of_pages().unwrap_or(0) as usize,
            pr.number_of_pages()
        );

        log::debug!(
            "Requesting all {:?} pages of pull requests",
            pr.number_of_pages()
        );
        let pr = self
            .octocrab
            .all_pages::<octocrab::models::pulls::PullRequest>(pr)
            .await
            .context("Failed to request all pull requests")?;
        log::debug!("Received all pull requests ({})", pr.len());

        let mut parsed_prs: Vec<PrEvent> = Vec::new();
        for pr in pr {
            let parsed = PrEvent {
                pr_number: pr.id.0 as i64,
                state: match (pr.state, pr.merged_at) {
                    (Some(IssueState::Open), _) => PullRequestStatus::Open,
                    (Some(IssueState::Closed), None) => PullRequestStatus::Closed,
                    (Some(IssueState::Closed), Some(_)) => PullRequestStatus::Merged {
                        merge_sha: pr.merge_commit_sha.expect("Missing merge commit SHA"),
                    },

                    (s, merged_at) => {
                        panic!("Invalid PR #{} state: {s:?}, {merged_at:?}", pr.number)
                    }
                },
                timestamp: DateTime::from(pr.created_at.unwrap()),
            };
            parsed_prs.push(parsed);
        }

        Ok(parsed_prs)
    }
}
