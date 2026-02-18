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
use octocrab::issues::ListIssuesBuilder;
use octocrab::models::timelines::TimelineEvent;
use octocrab::models::IssueState;
use octocrab::pulls::ListPullRequestsBuilder;
use octocrab::repos::ListPullsBuilder;
use octocrab::{models, params, params::issues, params::pulls, Octocrab};
use rust_team_data::v1::{PermissionPerson, Team};
use secrecy::SecretString;
use serde::Serialize;
use std::fmt::format;

pub struct GitHubApi {
    repository_identifier: String,
    owner: String,
    repository: String,
    octocrab: Octocrab,
}

impl GitHubApi {
    /// Create a new GitHubApi instance
    /// * token - GitHub personal access token
    pub fn new(
        repository_identifier: String,
        owner: String,
        repository: String,
        token: SecretString,
    ) -> anyhow::Result<Self> {
        let octocrab = Octocrab::builder().personal_token(token).build()?;
        Ok(Self {
            repository_identifier,
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
    ) -> anyhow::Result<(
        Vec<crate::db::model::issue::Issue>,
        Vec<team_member::Contributor>,
    )> {
        // check how many prs are there in total
        let issues = self
            .issue_request(state, 1, async |req| req.send().await)
            .await
            .context(format!(
                "Failed to get issues for {}/{}",
                self.owner, self.repository
            ))?;

        let mut parsed_issues: Vec<crate::db::model::issue::Issue> = Vec::new();
        let mut parsed_users: Vec<team_member::Contributor> = Vec::new();

        match sync_mode {
            SyncMode::Since(since) => {
                log::info!("Synchronizing issues since {since}");

                let mut page = 1u32;

                'pageLoop: loop {
                    let response = self
                        .issue_request(state, page, async |req| {
                            req.direction(params::Direction::Descending)
                                .sort(issues::Sort::Updated)
                                .send()
                                .await
                        })
                        .await
                        .context("Cannot get issues")?;

                    self.octocrab
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
                            log::debug!("Last PR updated at: {}", issue.updated_at.naive_utc());
                            self.parse_issues(&mut parsed_issues, &mut parsed_users, response).await;
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
                    log::warn!("Number of pages will be limited to {}", issues.number_of_pages().unwrap_or(10));
                    issues.number_of_pages().unwrap_or(10)
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

                            self.parse_issues(&mut parsed_issues, &mut parsed_users, pr).await;
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
        let pr = self
            .pr_request(state, 1, async |req| req.send().await)
            .await
            .context(format!(
                "Failed to get pull requests for {}/{}",
                self.owner, self.repository
            ))?;

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
                        pr if pr.updated_at.unwrap_or(pr.created_at.unwrap()).naive_utc()
                            < since =>
                            {
                                log::info!("No more pull requests to process, stopping at page {page}");
                                break 'pageLoop;
                            }
                        pr => {
                            log::debug!("Processing page {page}");
                            log::debug!(
                                "Number of pages: {}",
                                response.number_of_pages().unwrap_or(0)
                            );
                            log::debug!("Found {} pull requests", response.items.len());
                            log::debug!(
                                "Last PR updated at: {}",
                                pr.updated_at.unwrap_or(pr.created_at.unwrap())
                            );
                            self.parse_prs(&mut parsed_prs, &mut parsed_users, response).await;
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
                                .pr_request(state, page, async |req| {
                                    req.direction(params::Direction::Descending)
                                        .sort(pulls::Sort::Created)
                                        .send()
                                        .await
                                })
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

                            self.parse_prs(&mut parsed_prs, &mut parsed_users, pr).await;
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

    pub async fn get_event_timeline(
        &self,
        event_number: u64,
    ) -> anyhow::Result<Vec<TimelineEvent>> {
        let mut events: Vec<TimelineEvent> = Vec::new();
        let mut page = 1u32;

        let response = self
            .octocrab
            .issues(self.owner.clone(), self.repository.clone())
            .list_timeline_events(event_number)
            .per_page(100)
            .send()
            .await
            .context(format!(
                "Failed to get timeline events for PR #{} in {}/{}",
                event_number, self.owner, self.repository
            ))?;


        let pages = response.number_of_pages().unwrap_or(0);

        events.extend(response.items);

        if pages < 2 {
            return Ok(events);
        }

        for page in 2..=pages {
            let response = self
                .octocrab
                .issues(self.owner.clone(), self.repository.clone())
                .list_timeline_events(event_number)
                .per_page(100)
                .page(page)
                .send()
                .await
                .context(format!(
                    "Failed to get timeline events for PR #{} in {}/{}",
                    event_number, self.owner, self.repository
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


        Ok(events)
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

    async fn parse_prs(
        &self,
        parsed_prs: &mut Vec<PrEvent>,
        parsed_users: &mut Vec<team_member::Contributor>,
        pr: octocrab::Page<octocrab::models::pulls::PullRequest>,
    ) {
        for pr in pr.items {
            parsed_users.push(team_member::Contributor::from(
                *pr.user.clone().expect("No user in Contributor"),
            ));

            let timeline = self.get_event_timeline(pr.number).await.unwrap();
            let labels = self.get_labels(pr.number, &timeline).unwrap();
            let states = self.get_states(pr.number, &timeline).unwrap();

            let parsed = PrEvent {
                repository: self.repository_identifier.clone(),
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
                states_history: Some(states),
                labels_history: Some(labels),
            };
            parsed_prs.push(parsed);
        }
    }

    async fn parse_issues(
        &self,
        parsed_issues: &mut Vec<crate::db::model::issue::Issue>,
        parsed_users: &mut Vec<team_member::Contributor>,
        pr: octocrab::Page<octocrab::models::issues::Issue>,
    ) {
        for issue in pr.items {
            parsed_users.push(team_member::Contributor::from(issue.user.clone()));

            let timeline = self.get_event_timeline(issue.number).await.unwrap();
            let labels = self.get_labels(issue.number, &timeline).unwrap();
            let states = self.get_states(issue.number, &timeline).unwrap();

            let last_state = match issue.state {
                IssueState::Open => crate::db::model::issue::IssueStatus::Open {
                    time: issue.created_at,
                },
                IssueState::Closed => crate::db::model::issue::IssueStatus::Closed {
                    time: issue.closed_at.expect("Missing closed time"),
                },
                _ => panic!("Invalid issue state: {:?}", issue.state),
            };

            let parsed_issue = crate::db::model::issue::Issue {
                repository: self.repository_identifier.clone(),
                issue_number: issue.number as i64,
                author_id: issue.user.id.0 as i64,
                status: last_state,
                states_history: Some(states),
                labels_history: Some(labels),
            };

            parsed_issues.push(parsed_issue);
        }
    }

    fn get_labels(
        &self,
        issue_number: u64,
        timeline: &[TimelineEvent],
    ) -> anyhow::Result<Vec<crate::db::model::issue::IssueLabel>> {
        let mut vec = Vec::new();
        for event in timeline {
            match event.event {
                models::Event::Labeled => {
                    let label = event
                        .label
                        .clone()
                        .context(format!("cannot get label from Timeline event{:?}", event))?
                        .color;
                    let time = event
                        .created_at
                        .context(format!(
                            "cannot get created_at from Timeline event{:?}",
                            event
                        ))?
                        .naive_utc();
                    let issue_label = crate::db::model::issue::IssueLabel {
                        label,
                        timestamp: time,
                        action: crate::db::model::issue::LabelEventAction::Added,
                    };
                    vec.push(issue_label);
                }
                models::Event::Unlabeled => {
                    let label = event
                        .label
                        .clone()
                        .context(format!("cannot get label from Timeline event{:?}", event))?
                        .color;
                    let time = event
                        .created_at
                        .context(format!("cannot get time from Timeline event{:?}", event))?
                        .naive_utc();

                    let issue_label = crate::db::model::issue::IssueLabel {
                        label,
                        timestamp: time,
                        action: crate::db::model::issue::LabelEventAction::Removed,
                    };
                    vec.push(issue_label);
                }
                _ => {
                    log::trace!("not interesting timeline event: {:#?}", event);
                }
            }
        }
        Ok(vec)
    }

    fn get_states(
        &self,
        issue_number: u64,
        timeline: &[TimelineEvent],
    ) -> anyhow::Result<Vec<crate::db::model::issue::IssueState>> {
        let mut vec = Vec::new();
        for event in timeline {
            match event.event {
                models::Event::Closed => {
                    let time = event
                        .created_at
                        .context(format!("cannot get time from Timeline event{:?}", event))?
                        .naive_utc();
                    vec.push(crate::db::model::issue::IssueState {
                        state: "closed".to_string(),
                        timestamp: time,
                    });
                }
                models::Event::Committed => {
                    let time = event
                        .committer
                        .clone()
                        .context(format!("cannot get label from Timeline event{:?}", event))?
                        .date
                        .context(format!("cannot get label from Timeline event{:?}", event))?
                        .naive_utc();
                    vec.push(crate::db::model::issue::IssueState {
                        state: "open".to_string(),
                        timestamp: time,
                    });
                }
                models::Event::Commented => {
                    let time = event
                        .created_at
                        .context(format!("cannot get time from Timeline event{:?}", event))?
                        .naive_utc();
                }
                models::Event::Reopened => {
                    let time = event
                        .created_at
                        .context(format!("cannot get time from Timeline event{:?}", event))?
                        .naive_utc();
                }
                models::Event::Reviewed => {
                    let time = event
                        .updated_at
                        .context(format!("cannot get time from Timeline event{:?}", event))?
                        .naive_utc();
                }
                _ => {
                    log::trace!("not interesting timeline event: {:#?}", event);
                }
            }
        }
        Ok(vec)
    }
}

#[derive(Clone)]
pub enum SyncMode {
    /// Synchronize from the date
    Since(NaiveDateTime),
    /// Synchronize last N pages
    Last(u32),
}
