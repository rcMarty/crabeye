#![allow(unused)]

use crate::db::model::pr_event::{PrEvent, PullRequestStatus};
use crate::MULTI_PROGRESS_BAR;
use anyhow::Context;
use chrono::{DateTime, Utc};
use log::log;
use octocrab::models::issues::Issue;
use octocrab::models::pulls::PullRequest;
use octocrab::models::IssueState;
use octocrab::params::pulls::Sort;
use octocrab::{params, Octocrab};
use rust_team_data::v1::{PermissionPerson, Team};
use secrecy::SecretString;
use std::fmt::format;
use indicatif::ProgressBar;
use crate::db::model::team_member::{Contributor, TeamMember};
use crate::misc::with_progress_bar_async;

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

    pub async fn get_authorized_users(&self) -> Result<Vec<TeamMember>, String> {
        let url = format!("{}/teams.json", ::rust_team_data::v1::BASE_URL);
        let client = reqwest::Client::new();
        let teams = client
            .get(&url)
            .send()
            .await
            .map_err(|err| format!("failed to fetch authorized users: {}", err))?
            .error_for_status()
            .map_err(|err| format!("failed to fetch authorized users: {}", err))?
            .json::<rust_team_data::v1::Teams>()
            .await
            .map_err(|err| format!("failed to fetch authorized users: {}", err))
            .map(|teams| teams.teams)
            .map(|indextype| indextype.into_iter().map(|(_k, v)| v).collect::<Vec<_>>())
            .map_err(|err| format!("failed to fetch authorized users: {}", err))?;

        let authorized_users = teams.iter()
            .filter(|team| { team.name != "all" })
            .flat_map(|team| {
                team.members.iter()
                    .map(|member| TeamMember {
                        github_id: member.github_id,
                        github_name: member.github.clone(),
                        name: member.name.clone(),
                        team: team.name.clone(),
                        subteam_of: team.subteam_of.clone(),
                        kind: team.kind,
                    })
                    .collect::<Vec<TeamMember>>()
            })
            .collect::<Vec<TeamMember>>();
        Ok(authorized_users)
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

        log::info!(
            "Found {} issues, no of pages: {:?}, total count: {:?}",
            page.items.len(),
            page.number_of_pages(),
            page.total_count
        );

        let result = self.octocrab.get_page::<Issue>(&page.next).await?;
        log::info!(
            "result: {:?}",
            result.clone().unwrap().items.first().unwrap().url
        );
        let issues = result.unwrap().items;

        for issue in issues.iter() {
            log::info!(
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
        sync_mode: SyncMode,
    ) -> anyhow::Result<(Vec<PrEvent>, Vec<Contributor>)> {
        let pr = self
            .octocrab
            .pulls(self.owner.clone(), self.repository.clone())
            .list()
            .state(state)
            .send()
            .await
            .context(format!(
                "Failed to get pull requests for {}/{}",
                self.owner, self.repository
            ))?;

        log::info!("Found less than {} pull requests, no of pages: {}",pr.items.len() * pr.number_of_pages().unwrap_or(0) as usize,pr.number_of_pages().unwrap_or(0));

        let no_of_pages: u32 = match sync_mode {
            SyncMode::Since(since) => {
                log::info!("Synchronizing pull requests since {since}");
                0
            }
            SyncMode::Last(n) => {
                log::info!("Synchronizing last {n} pages of pull requests");
                n
            }
        };

        let real_no_of_pages = if pr.number_of_pages() < Some(no_of_pages) {
            log::warn!("Found less than requested ({no_of_pages}) pull requests");
            log::warn!(
                "Number of pages will be limited to {}",
                pr.number_of_pages().unwrap_or(0)
            );
            pr.number_of_pages().unwrap_or(0)
        } else {
            no_of_pages
        };

        let mut parsed_prs: Vec<PrEvent> = Vec::new();
        let mut parsed_users: Vec<Contributor> = Vec::new();

        with_progress_bar_async(real_no_of_pages as usize, "Processing".parse()?, async |bar: &ProgressBar| {
            for page in 1..real_no_of_pages {
                bar.inc(1);
                bar.set_message(format!("Processing page {}/{}", page, real_no_of_pages));
                let pr = self
                    .octocrab
                    .pulls(self.owner.clone(), self.repository.clone())
                    .list()
                    .sort(Sort::Updated)
                    .state(state)
                    .per_page(100)
                    .page(page)
                    .direction(params::Direction::Descending)
                    .send()
                    .await
                    .context(format!(
                        "Failed to get pull requests for {}/{}",
                        self.owner, self.repository
                    ))?;

                log::debug!("Requesting {page}/{no_of_pages} page of pull requests");
                log::debug!("Found {} pull requests", pr.items.len());

                // check if there are no more PRs
                if pr.items.is_empty() {
                    break;
                }

                for pr in pr.items {
                    // log::info!("issue state: {:?}", pr.state);
                    // log::info!("labels: {:#?}", pr.labels);
                    //TODO add labels for pr events
                    let pr_copy = pr.clone();
                    parsed_users.push(Contributor::from(*pr_copy.user.expect("No user in Contributor")));

                    let parsed = PrEvent {
                        pr_number: pr.number as i64,
                        author_id: pr.user.expect("No author in PrEvent").id,
                        state: match (pr.state, pr.merged_at, pr.labels) {
                            (Some(IssueState::Open), _, Some(labels)) => {
                                PullRequestStatus::find_status(
                                    labels
                                        .into_iter()
                                        .map(|label| label.name.to_string())
                                        .collect::<Vec<String>>(),
                                    pr.created_at.expect("Missing created time"),
                                    None,
                                )
                                    .unwrap_or(PullRequestStatus::Open {
                                        time: pr.created_at.expect("Missing created time"),
                                    })
                            }
                            (Some(IssueState::Open), _, None) => PullRequestStatus::Open {
                                time: pr.created_at.expect("Missing created time"),
                            },
                            (Some(IssueState::Closed), None, _) => PullRequestStatus::Closed {
                                time: pr.closed_at.expect("Missing closed time"),
                            },
                            (Some(IssueState::Closed), Some(_), _) => PullRequestStatus::Merged {
                                merge_sha: pr
                                    .merge_commit_sha
                                    .and_then(|s| s.is_empty().then_some(None).or(Some(Some(s))))
                                    .unwrap_or_else(|| {
                                        //panic!("Missing merge commit SHA {:#?} ", debug_pr)
                                        Some("NONE".to_string())
                                    })
                                    .unwrap_or_else(|| "NONE".to_string()), //panic!("SHA is empty {:#?} ", debug_pr)),
                                time: pr.merged_at.expect("Missing merge time"),
                            },
                            (s, merged_at, labels) => {
                                panic!("Invalid PR #{} state: {s:?}, {merged_at:?}", pr.number)
                            }
                        },
                    };
                    parsed_prs.push(parsed);
                }
            }
            Ok(())
        }).await?;

        Ok((parsed_prs, parsed_users))
    }
}

pub enum SyncMode {
    /// Synchronize from the date
    Since(DateTime<Utc>),
    /// Synchronize last N pages
    Last(u32),
}