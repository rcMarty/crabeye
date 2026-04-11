#![allow(unused)]

pub(crate) mod rate_limit;
mod fetch;
mod parse;

use chrono::NaiveDateTime;
use octocrab::issues::ListIssuesBuilder;
use octocrab::pulls::ListPullRequestsBuilder;
use octocrab::{params, Octocrab};
use secrecy::SecretString;
use std::io::Write;

pub struct GitHubApi {
    repository_identifier: String,
    owner: String,
    repository: String,
    octocrab: Octocrab,
    database: crate::db::Database,
}

/// basic functions and helpers
impl GitHubApi {
    /// Create a new GitHubApi instance
    /// * token - GitHub personal access token
    pub fn new(
        repository_identifier: String,
        owner: String,
        repository: String,
        token: SecretString,
        database: crate::db::Database,
    ) -> anyhow::Result<Self> {
        let octocrab = Octocrab::builder().personal_token(token).build()?;
        Ok(Self {
            repository_identifier,
            owner,
            repository,
            octocrab,
            database,
        })
    }

    fn append_to_file(content: &str) {
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open("ranal_octocrab_errors.log")
            .unwrap();

        file.write_all(content.as_bytes()).unwrap();
    }

    async fn issue_request<F, R>(&self, state: params::State, page: u32, f: F) -> R
    where
        F: AsyncFnOnce(ListIssuesBuilder<'_, '_, '_, '_>) -> R,
    {
        let issues = self
            .octocrab
            .issues(self.owner.clone(), self.repository.clone());
        let req = issues.list().per_page(100).page(page).state(state);

        f(req).await
    }

    async fn pr_request<F, R>(&self, state: params::State, page: u32, f: F) -> R
    where
        F: AsyncFnOnce(ListPullRequestsBuilder<'_, '_>) -> R,
    {
        let prs = self
            .octocrab
            .pulls(self.owner.clone(), self.repository.clone());
        let req = prs.list().per_page(100).page(page).state(state);

        f(req).await
    }
}

#[derive(Clone)]
pub enum SyncMode {
    /// Synchronize from the date
    Since(NaiveDateTime),
    /// Synchronize last N pages
    Last(u32),
}

