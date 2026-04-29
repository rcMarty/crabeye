use super::rate_limit::retry_on_rate_limit;
use super::{GitHubApi, SyncMode};
use crate::db::model::pr_event::{PrEvent, PullRequestStatus};
use crate::db::model::{team_member, BackfillRecord};
use crate::progress::with_progress_bar_async;
use anyhow::Context;
use indicatif::MultiProgress;
use octocrab::models::IssueState;
use octocrab::{params, params::issues, params::pulls};
use rust_team_data::v1::Team;

/// Public functions about getting all the data from api
impl GitHubApi {
    pub async fn get_authorized_users(
        &self,
    ) -> Result<(Vec<Team>, Vec<team_member::TeamMember>), String> {
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

        log::debug!(
            "launching pad exists:{}",
            teams
                .iter()
                .filter(|team| team.name == "launching-pad")
                .count()
        );

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
        Ok((teams, authorized_users))
    }

    pub async fn get_issues(
        &self,
        state: params::State,
        sync_mode: SyncMode,
        with_timeline: bool,
    ) -> anyhow::Result<(
        Vec<crate::db::model::issue::Issue>,
        Vec<team_member::Contributor>,
    )> {
        // check how many prs are there in total
        let issues = retry_on_rate_limit("get issues (initial)", || async {
            self.issue_request(state, 1, async |req| req.send().await)
                .await
        })
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
                    let response = retry_on_rate_limit("get issues (since)", || async {
                        self.issue_request(state, page, async |req| {
                            req.direction(params::Direction::Descending)
                                .sort(issues::Sort::Updated)
                                .send()
                                .await
                        })
                        .await
                    })
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
                            self.parse_issues(
                                &mut parsed_issues,
                                &mut parsed_users,
                                response,
                                with_timeline,
                            )
                            .await;
                            page += 1;
                        }
                    }
                }

                Ok((parsed_issues, parsed_users))
            }
            SyncMode::Last(no_of_pages) => {
                log::info!("Synchronizing last {no_of_pages} pages of pull requests");
                let pages = no_of_pages;

                with_progress_bar_async(
                    pages as usize,
                    Some("Getting issues".parse()?),
                    async |bar_opt, _multi: &MultiProgress| {
                        let bar = bar_opt.unwrap();
                        for page in 1..=pages {
                            bar.inc(1);
                            bar.set_message(format!("Processing page {}/{}", page, pages));
                            let pr = retry_on_rate_limit("get issues (last)", || async {
                                self.issue_request(state, page, async |req| {
                                    req.direction(params::Direction::Descending)
                                        .sort(issues::Sort::Created)
                                        .send()
                                        .await
                                })
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

                            self.parse_issues(
                                &mut parsed_issues,
                                &mut parsed_users,
                                pr,
                                with_timeline,
                            )
                            .await;
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
        with_timeline: bool,
    ) -> anyhow::Result<(Vec<PrEvent>, Vec<team_member::Contributor>)> {
        // check how many prs are there in total
        let pr = retry_on_rate_limit("get pull requests (initial)", || async {
            self.pr_request(state, 1, async |req| req.send().await)
                .await
        })
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
                    let response = retry_on_rate_limit("get pull requests (since)", || async {
                        self.pr_request(state, page, async |req| {
                            req.direction(params::Direction::Descending)
                                .sort(pulls::Sort::Updated)
                                .send()
                                .await
                        })
                        .await
                    })
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
                                response.number_of_pages().unwrap_or(10)
                            );
                            log::debug!("Found {} pull requests", response.items.len());
                            log::debug!(
                                "Last PR updated at: {}",
                                pr.updated_at.unwrap_or(pr.created_at.unwrap())
                            );
                            self.parse_prs(
                                &mut parsed_prs,
                                &mut parsed_users,
                                response,
                                with_timeline,
                            )
                            .await;
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
                        pr.number_of_pages().unwrap_or(10)
                    );

                    pr.number_of_pages().unwrap_or(10)
                } else {
                    no_of_pages
                };

                with_progress_bar_async(
                    pages as usize,
                    Some("Getting pull requests".parse()?),
                    async |bar_opt, _multi: &MultiProgress| {
                        let bar = bar_opt.unwrap();
                        for page in 1..=pages {
                            bar.inc(1);
                            bar.set_message(format!("Processing page {}/{}", page, pages));
                            let pr = retry_on_rate_limit("get pull requests (last)", || async {
                                self.pr_request(state, page, async |req| {
                                    req.direction(params::Direction::Descending)
                                        .sort(pulls::Sort::Created)
                                        .send()
                                        .await
                                })
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

                            self.parse_prs(&mut parsed_prs, &mut parsed_users, pr, with_timeline)
                                .await;
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

    pub async fn process_backfill(&self, records_from_db: &mut [BackfillRecord]) {
        with_progress_bar_async(
            records_from_db.len(),
            Some("Processing backfill records".parse().unwrap()),
            async |bar_opt, multi| {
                let bar = bar_opt.unwrap();
                for record in records_from_db {
                    bar.inc(1);
                    bar.set_message(format!(
                        "Processing backfill (issue #{})",
                        record.issue_number
                    ));
                    let (labels, states) = self
                        .fetch_and_parse_timeline(record.issue_number as u64)
                        .await;

                    record.labels_history = labels;
                    record.states_history = states;
                }
                bar.finish_with_message("Done");
                Ok(())
            },
        )
        .await
        .unwrap();
    }
}
