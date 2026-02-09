#![allow(unused)]

use crate::db::model::pr_event::{PrEvent, PullRequestStatus};
use crate::db::model::team_member;
use crate::misc::with_progress_bar_async;
use crate::MULTI_PROGRESS_BAR;
use anyhow::{anyhow, Context};
use chrono::{DateTime, NaiveDateTime, Utc};
use git2::Direction;
use indicatif::ProgressBar;
use log::log;
use octocrab::{models, params, params::pulls, params::issues, Octocrab};
use rust_team_data::v1::{PermissionPerson, Team};
use secrecy::SecretString;
use std::fmt::format;
use octocrab::issues::ListIssuesBuilder;
use octocrab::models::IssueState;
use octocrab::models::timelines::TimelineEvent;
use octocrab::pulls::ListPullRequestsBuilder;
use octocrab::repos::ListPullsBuilder;

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
        Ok(Self {
            owner,
            repository,
            octocrab,
        })
    }

    pub async fn get_authorized_users(&self) -> Result<Vec<team_member::TeamMember>, String> {
        let url = format!("{}/teams.json", ::rust_team_data::v1::BASE_URL);
        let client = reqwest::Client::new();
        let teams = client
            .get(&url)
            .send()
            .await
            .map_err(|err| format!("failed to fetch authorized users 1: {}", err))?
            .error_for_status()
            .map_err(|err| format!("failed to fetch authorized users 2: {}", err))?
            .json::<rust_team_data::v1::Teams>()
            .await
            .map_err(|err| format!("failed to fetch authorized users 3: {}", err))
            .map(|teams| teams.teams.into_iter().map(|(_k, v)| v).collect::<Vec<_>>())
            .map_err(|err| format!("failed to fetch authorized users 4: {}", err))?;

        let authorized_users = teams
            .iter()
            .filter(|team| team.name != "all")
            .flat_map(|team| {
                team.members
                    .iter()
                    .map(|member| team_member::TeamMember {
                        github_id: member.github_id,
                        github_name: member.github.clone(),
                        name: member.name.clone(),
                        teams: vec![team_member::Team {
                            team: team.name.clone(),
                            subteam_of: team.subteam_of.clone(),
                            kind: team.kind,
                        }],
                    })
                    .collect::<Vec<team_member::TeamMember>>()
            })
            .collect::<Vec<team_member::TeamMember>>();
        Ok(authorized_users)
    }

    pub async fn get_issues(
        &self,
        state: params::State,
        sync_mode: SyncMode,
    ) -> anyhow::Result<(Vec<crate::db::model::issue::Issue>, Vec<team_member::Contributor>)> {
        // check how many prs are there in total
        let issues = self.issue_request(state, 1, async |req| { req.send().await })
            .await
            .context(format!("Failed to get issues for {}/{}", self.owner, self.repository))?;

        let mut parsed_issues: Vec<crate::db::model::issue::Issue> = Vec::new();
        let mut parsed_users: Vec<team_member::Contributor> = Vec::new();

        match sync_mode {
            SyncMode::Since(since) => {
                log::info!("Synchronizing issues since {since}");

                let mut page = 1u32;

                'pageLoop: loop {
                    let response = self.issue_request(state, page, async |req| {
                        req
                            .direction(params::Direction::Descending)
                            .sort(issues::Sort::Updated)
                            .send()
                            .await
                    })
                        .await
                        .context("Cannot get issues")?;


                    self
                        .octocrab
                        .issues(self.owner.clone(), self.repository.clone())
                        .list()
                        .direction(params::Direction::Descending)
                        .sort(issues::Sort::Updated)
                        .state(state)
                        .per_page(100)
                        .page(page)
                        .send()
                        .await
                        .context("Cannot get issues")?;

                    log::debug!("Requesting {page} page of pull requests");
                    match response.items.last().unwrap() {
                        issue if issue.updated_at.naive_utc() < since => {
                            log::info!("No more pull requests to process, stopping at page {page}");
                            break 'pageLoop;
                        }
                        issue => {
                            log::debug!("Processing page {page}");
                            log::debug!("Found {} issues", response.items.len());
                            log::debug!("Last PR updated at: {}",issue.updated_at.naive_utc());
                            parse_issues(&mut parsed_issues, &mut parsed_users, response);
                            page += 1;
                        }
                    }
                }

                Ok((parsed_issues, parsed_users))
            }
            SyncMode::Last(no_of_pages) => {
                log::info!("Synchronizing last {no_of_pages} pages of pull requests");
                let pages = if issues.number_of_pages() < Some(no_of_pages) {
                    log::warn!("Found less than requested ({no_of_pages}) pull requests");
                    log::warn!("Number of pages will be limited to {}", issues.number_of_pages().unwrap_or(0));
                    issues.number_of_pages().unwrap_or(0)
                } else {
                    no_of_pages
                };

                with_progress_bar_async(
                    pages as usize,
                    "Processing".parse()?,
                    async |bar: &ProgressBar| {
                        for page in 1..=pages {
                            bar.inc(1);
                            bar.set_message(format!("Processing page {}/{}", page, pages));
                            let pr = self
                                .octocrab
                                .issues(self.owner.clone(), self.repository.clone())
                                .list()
                                .page(page)
                                .direction(params::Direction::Descending)
                                .sort(issues::Sort::Created)
                                .state(state)
                                .per_page(100)
                                .send()
                                .await
                                .context(format!(
                                    "Failed to get pull requests for {}/{}",
                                    self.owner, self.repository
                                ))?;

                            log::debug!("Requesting {page}/{pages} page of pull requests");
                            log::debug!("Found {} pull requests", pr.items.len());

                            // check if there are no more PRs
                            if pr.items.is_empty() {
                                break;
                            }

                            parse_issues(&mut parsed_issues, &mut parsed_users, pr);
                        }
                        bar.finish_with_message("Done");
                        Ok(())
                    },
                )
                    .await?;
                Ok((parsed_issues, parsed_users))
            }
        }
    }

    pub async fn get_pull_requests(
        &self,
        state: params::State,
        sync_mode: SyncMode,
    ) -> anyhow::Result<(Vec<PrEvent>, Vec<team_member::Contributor>)> {
        // check how many prs are there in total
        let pr = self.pr_request(state, 1, async |req| { req.send().await })
            .await
            .context(format!("Failed to get pull requests for {}/{}", self.owner, self.repository))?;

        let mut parsed_prs: Vec<PrEvent> = Vec::new();
        let mut parsed_users: Vec<team_member::Contributor> = Vec::new();

        match sync_mode {
            SyncMode::Since(since) => {
                log::info!("Synchronizing pull requests since {since}");

                let mut page = 1u32;
                'pageLoop: loop {
                    let response = self
                        .octocrab
                        .pulls(self.owner.clone(), self.repository.clone())
                        .list()
                        .direction(params::Direction::Descending)
                        .sort(pulls::Sort::Updated)
                        .state(state)
                        .per_page(100)
                        .page(page)
                        .send()
                        .await
                        .context("Cannot get pull requests")?;

                    log::debug!("Requesting {page} page of pull requests");
                    match response.items.last().unwrap() {
                        pr if pr.updated_at.unwrap_or(pr.created_at.unwrap()).naive_utc() < since => {
                            log::info!("No more pull requests to process, stopping at page {page}");
                            break 'pageLoop;
                        }
                        pr => {
                            log::debug!("Processing page {page}");
                            log::debug!("Found {} pull requests", response.items.len());
                            log::debug!(
                                "Last PR updated at: {}",
                                pr.updated_at.unwrap_or(pr.created_at.unwrap())
                            );
                            parse_prs(&mut parsed_prs, &mut parsed_users, response);
                            page += 1;
                        }
                    }
                }

                Ok((parsed_prs, parsed_users))
            }
            SyncMode::Last(no_of_pages) => {
                log::info!("Synchronizing last {no_of_pages} pages of pull requests");
                let pages = if pr.number_of_pages() < Some(no_of_pages) {
                    log::warn!("Found less than requested ({no_of_pages}) pull requests");
                    log::warn!(
                        "Number of pages will be limited to {}",
                        pr.number_of_pages().unwrap_or(0)
                    );

                    pr.number_of_pages().unwrap_or(0)
                } else {
                    no_of_pages
                };

                with_progress_bar_async(
                    pages as usize,
                    "Processing".parse()?,
                    async |bar: &ProgressBar| {
                        for page in 1..=pages {
                            bar.inc(1);
                            bar.set_message(format!("Processing page {}/{}", page, pages));
                            let pr = self
                                .octocrab
                                .pulls(self.owner.clone(), self.repository.clone())
                                .list()
                                .page(page)
                                .direction(params::Direction::Descending)
                                .sort(pulls::Sort::Created)
                                .state(state)
                                .per_page(100)
                                .send()
                                .await
                                .context(format!(
                                    "Failed to get pull requests for {}/{}",
                                    self.owner, self.repository
                                ))?;

                            log::debug!("Requesting {page}/{pages} page of pull requests");
                            log::debug!("Found {} pull requests", pr.items.len());

                            // check if there are no more PRs
                            if pr.items.is_empty() {
                                break;
                            }

                            parse_prs(&mut parsed_prs, &mut parsed_users, pr);
                        }
                        bar.finish_with_message("Done");
                        Ok(())
                    },
                )
                    .await?;
                Ok((parsed_prs, parsed_users))
            }
        }
    }

    pub async fn get_pull_request_timeline(
        &self,
        pr_number: u64,
    ) -> anyhow::Result<Vec<TimelineEvent>> {
        let mut events: Vec<TimelineEvent> = Vec::new();
        let mut page = 1u32;

        let response = self
            .octocrab
            .issues(self.owner.clone(), self.repository.clone())
            .list_timeline_events(pr_number)
            .per_page(100)
            .send()
            .await
            .context(format!("Failed to get timeline events for PR #{} in {}/{}", pr_number, self.owner, self.repository))?;

        let pages = response.number_of_pages().unwrap_or(0);

        events.extend(response.items);

        if pages < 2 { return Ok(events); }

        with_progress_bar_async(
            pages as usize,
            format!("Processing timeline of issue {pr_number}").parse()?,
            async |bar: &ProgressBar| {
                for page in 2..=pages {
                    bar.inc(1);
                    bar.set_message(format!("Processing page {}/{}", page, pages));
                    let response = self
                        .octocrab
                        .issues(self.owner.clone(), self.repository.clone())
                        .list_timeline_events(pr_number)
                        .per_page(100)
                        .page(page)
                        .send()
                        .await
                        .context(format!(
                            "Failed to get timeline events for PR #{} in {}/{}",
                            pr_number, self.owner, self.repository
                        ))?;

                    log::debug!("Requesting {page}/{pages} page of timeline events");
                    log::debug!("Found {} timeline events", response.items.len());

                    // check if there are no more events
                    if response.items.is_empty() {
                        break;
                    }
                    //TODO events primarily opened closed and so on

                    events.extend(response.items);
                }
                bar.finish_with_message("Done");
                Ok(())
            },
        )
            .await?;


        Ok(events)
    }

    async fn issue_request<F, R>(&self, state: params::State, page: u32, f: F) -> R
    where
        F: AsyncFnOnce(ListIssuesBuilder<'_, '_, '_, '_>) -> R,
    {
        let issues = self.octocrab.issues(self.owner.clone(), self.repository.clone());
        let req = issues
            .list()
            .per_page(100)
            .page(page)
            .state(state);

        f(req).await
    }

    async fn pr_request<F, R>(&self, state: params::State, page: u32, f: F) -> R
    where
        F: AsyncFnOnce(ListPullRequestsBuilder<'_, '_>) -> R,
    {
        let prs = self.octocrab.pulls(self.owner.clone(), self.repository.clone());
        let req = prs
            .list()
            .per_page(100)
            .page(page)
            .state(state);

        f(req).await
    }
}


fn parse_prs(
    parsed_prs: &mut Vec<PrEvent>,
    parsed_users: &mut Vec<team_member::Contributor>,
    pr: octocrab::Page<octocrab::models::pulls::PullRequest>,
) {
    for pr in pr.items {
        parsed_users.push(team_member::Contributor::from(*pr.user.clone().expect("No user in Contributor")));

        let parsed = PrEvent {
            pr_number: pr.number as i64,
            author_id: pr.user.expect("No author in PrEvent").id.0 as i64,
            state: match (pr.state, pr.merged_at, pr.labels) {
                (Some(IssueState::Open), _, Some(labels)) => PullRequestStatus::find_status(
                    labels
                        .into_iter()
                        .map(|label| label.name.to_string())
                        .collect::<Vec<String>>(),
                    pr.created_at.expect("Missing created time"),
                    None,
                )
                    .unwrap_or(PullRequestStatus::Open {
                        time: pr.created_at.expect("Missing created time"),
                    }),
                (Some(IssueState::Open), _, None) => PullRequestStatus::Open {
                    time: pr.created_at.expect("Missing created time"),
                },
                (Some(IssueState::Closed), None, _) => PullRequestStatus::Closed {
                    time: pr.closed_at.expect("Missing closed time"),
                },
                (Some(IssueState::Closed), Some(_), _) => {
                    PullRequestStatus::Merged {
                        merge_sha: pr
                            .merge_commit_sha
                            .and_then(|s| s.is_empty().then_some(None).or(Some(Some(s))))
                            .unwrap_or_else(|| {
                                //panic!("Missing merge commit SHA {:#?} ", debug_pr)
                                Some("NONE".to_string())
                            })
                            .unwrap_or_else(|| "NONE".to_string()), //panic!("SHA is empty {:#?} ", debug_pr)),
                        time: pr.merged_at.expect("Missing merge time"),
                    }
                }
                (s, merged_at, labels) => {
                    panic!("Invalid PR #{} state: {s:?}, {merged_at:?}", pr.number)
                }
            },
        };
        parsed_prs.push(parsed);
    }
}

fn parse_issues(
    parsed_prs: &mut Vec<crate::db::model::issue::Issue>,
    parsed_users: &mut Vec<team_member::Contributor>,
    pr: octocrab::Page<octocrab::models::issues::Issue>,
) {
    for issue in pr.items {
        parsed_users.push(team_member::Contributor::from(issue.user.clone()));

        for label in issue.labels {
            log::debug!("Issue #{} label: {}", issue.number, label.name);
            let parsed = crate::db::model::issue::Issue {
                issue_number: issue.number as i64,
                author_id: issue.user.id.0 as i64,
                timestamp: issue.created_at.naive_utc(),
                label: label.name.to_string(),
                action: crate::db::model::issue::LabelEventAction::Added,
            };
            parsed_prs.push(parsed);
        }
    }
}

#[derive(Clone)]
pub enum SyncMode {
    /// Synchronize from the date
    Since(NaiveDateTime),
    /// Synchronize last N pages
    Last(u32),
}
