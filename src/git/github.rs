#![allow(unused)]
use crate::db::model::pr_event::{PrEvent, PullRequestStatus};
use anyhow::Context;
use chrono::{DateTime, Utc};
use log::log;
use octocrab::models::issues::Issue;
use octocrab::models::pulls::PullRequest;
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

    pub async fn get_issues(
        &self,
        name: String,
        state: params::State,
    ) -> anyhow::Result<Vec<Issue>> {
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

    pub async fn get_pull_requests(
        &self,
        state: params::State,
        no_of_pages: u32,
    ) -> anyhow::Result<Vec<PrEvent>> {
        let pr = self
            .octocrab
            .pulls(self.owner.clone(), self.repository.clone())
            .list()
            .state(state)
            .page(2u32)
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

        if pr.number_of_pages() < Some(no_of_pages) {
            log::warn!("Found less than {no_of_pages} pull requests");
        }

        let mut parsed_prs: Vec<PrEvent> = Vec::new();

        for page in 1..no_of_pages {
            let pr = self
                .octocrab
                .pulls(self.owner.clone(), self.repository.clone())
                .list()
                .state(state)
                .page(2u32)
                .per_page(100)
                .page(page)
                .send()
                .await
                .context(format!(
                    "Failed to get pull requests for {}/{}",
                    self.owner, self.repository
                ))?;

            log::debug!("Requesting {page}/{no_of_pages} page of pull requests");

            for pr in pr.items {
                let parsed = PrEvent {
                    pr_number: pr.number as i64,
                    author_id: pr.user.expect("No author in PrEvent").id,
                    state: match (pr.state, pr.merged_at) {
                        (Some(IssueState::Open), _) => PullRequestStatus::Open {
                            time: pr.created_at.expect("Missing created time"),
                        },
                        (Some(IssueState::Closed), None) => PullRequestStatus::Closed {
                            time: pr.closed_at.expect("Missing closed time"),
                        },
                        (Some(IssueState::Closed), Some(_)) => PullRequestStatus::Merged {
                            merge_sha: pr.merge_commit_sha.expect("Missing merge commit SHA"),
                            time: pr.merged_at.expect("Missing merge time"),
                        },

                        (s, merged_at) => {
                            panic!("Invalid PR #{} state: {s:?}, {merged_at:?}", pr.number)
                        }
                    },
                };
                parsed_prs.push(parsed);
            }
        }

        Ok(parsed_prs)
    }
}
