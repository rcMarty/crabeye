use super::rate_limit::retry_on_rate_limit;
use super::GitHubApi;
use crate::db::model::issue::IssueLabel;
use crate::db::model::pr_event::{PrEvent, PullRequestStatus};
use crate::db::model::team_member;
use crate::progress::with_progress_bar_async;
use anyhow::Context;
use indicatif::{MultiProgress, ProgressBar};
use octocrab::models;
use octocrab::models::timelines::TimelineEvent;
use octocrab::models::IssueState;

/// Private fetching functions and parsing
impl GitHubApi {
    fn should_timeline_requests_continue(
        response: &[TimelineEvent],
        last_timestamp: &Option<chrono::NaiveDateTime>,
    ) -> bool {
        match last_timestamp {
            Some(last_ts) => response.iter().any(|event| {
                event
                    .created_at
                    .is_some_and(|ts| ts.naive_utc() <= *last_ts)
            }),
            None => false,
        }
    }

    async fn get_event_timeline(&self, event_number: u64) -> anyhow::Result<Vec<TimelineEvent>> {
        let mut events: Vec<TimelineEvent> = Vec::new();
        let mut page = 1u32;

        let response = match retry_on_rate_limit("timeline events (first page)", || async {
            self.octocrab
                .issues(self.owner.clone(), self.repository.clone())
                .list_timeline_events(event_number)
                .per_page(100)
                .send()
                .await
        })
            .await
            .context(format!(
                "Failed to get timeline events for PR #{} in {}/{}",
                event_number, self.owner, self.repository
            )) {
            Ok(res) => res,
            Err(err) => {
                log::error!(
                    "Error getting timeline events for PR #{}: {}",
                    event_number,
                    err
                );
                let error_message = format!(
                    "Error getting timeline events for PR #{}: {:#?} \n",
                    event_number, err
                );
                Self::append_to_file(&error_message);
                return Err(err);
            }
        };

        if response.items.len() < 100 {
            events.extend(response.items);
            return Ok(events);
        }

        // should i download more pages because of history?
        let last_timestamp = self
            .database
            .get_last_update(self.repository_identifier.as_str(), event_number as i64)
            .await?;
        if Self::should_timeline_requests_continue(&response.items, &last_timestamp) {
            events.extend(response.items);
            return Ok(events);
        };

        events.extend(response.items);

        'pageLoop: loop {
            let response = match retry_on_rate_limit("timeline events (pagination)", || async {
                self.octocrab
                    .issues(self.owner.clone(), self.repository.clone())
                    .list_timeline_events(event_number)
                    .per_page(100)
                    .page(page)
                    .send()
                    .await
            })
                .await
                .context(format!(
                    "Failed to get timeline events for PR #{} in {}/{}",
                    event_number, self.owner, self.repository
                )) {
                Ok(res) => res,
                Err(err) => {
                    log::error!(
                        "Error getting timeline events for PR #{}: {}",
                        event_number,
                        err
                    );
                    let error_message = format!(
                        "Error getting timeline events for PR #{}: {}",
                        event_number, err
                    );
                    Self::append_to_file(&error_message);
                    return Ok(events);
                }
            };

            page += 1;
            log::debug!("Found another {} timeline events", response.items.len());

            // check if there are no more events
            if response.items.is_empty() {
                break;
            }

            if Self::should_timeline_requests_continue(&response.items, &last_timestamp) {
                events.extend(response.items);
                return Ok(events);
            };

            events.extend(response.items);
        }

        Ok(events)
    }

    pub(super) async fn fetch_and_parse_timeline(
        &self,
        issue_number: u64,
    ) -> (
        Option<Vec<crate::db::model::issue::IssueLabel>>,
        Option<Vec<crate::db::model::issue::IssueEvent>>,
    ) {
        match self.get_event_timeline(issue_number).await {
            Ok(timeline) => (
                self.get_labels(issue_number, &timeline)
                    .map_err(|err| {
                        log::error!("Cannot get labels for PR #{}: {:#?}", issue_number, err)
                    })
                    .ok(),
                self.get_events(issue_number, &timeline)
                    .map_err(|err| {
                        log::error!("Cannot get events for PR #{}: {:#?}", issue_number, err)
                    })
                    .ok(),
            ),
            Err(e) => {
                log::error!(
                    "Cannot download timeline events for PR #{}: {:#?}",
                    issue_number,
                    e
                );
                (None, None)
            }
        }
    }

    pub(super) async fn parse_prs(
        &self,
        parsed_prs: &mut Vec<PrEvent>,
        parsed_users: &mut Vec<team_member::Contributor>,
        pr: octocrab::Page<octocrab::models::pulls::PullRequest>,
        with_timeline: bool,
    ) {
        with_progress_bar_async(pr.items.len(), None, async |_bar, multi: &MultiProgress| {
            let inner_bar = multi.add(ProgressBar::new(pr.items.len() as u64));
            inner_bar.set_style(
                indicatif::ProgressStyle::default_bar()
                    .template("[{elapsed_precise}] {bar:40.green/cyan} {pos:>7}/{len:7} {msg}")?
                    .progress_chars("##-"),
            );

            for pr in pr.items {
                inner_bar.set_message("Processing PR events with timeline events and labels");
                inner_bar.inc(1);
                parsed_users.push(team_member::Contributor::from(
                    *pr.user.clone(),
                ));

                let (labels, events) = if with_timeline {
                    let (lab, mut event) = self.fetch_and_parse_timeline(pr.number).await;
                    event
                        .get_or_insert_default()
                        .push(crate::db::model::issue::IssueEvent {
                            event: "created".to_string(),
                            timestamp: pr.created_at.naive_utc(),
                        });
                    (lab, event)
                } else {
                    let lab = Some(vec![]);
                    let event = Some(vec![crate::db::model::issue::IssueEvent {
                        event: "created".to_string(),
                        timestamp: pr.created_at.naive_utc(),
                    }]);
                    (lab, event)
                };

                let parsed = PrEvent {
                    repository: self.repository_identifier.clone(),
                    pr_number: pr.number as i64,
                    author_id: pr.user.as_ref().id.0 as i64,
                    created_at: pr.created_at,
                    state: match (pr.state, pr.merged_at, pr.labels) {
                        (IssueState::Open, _, labels) if labels.is_empty() => PullRequestStatus::Open {
                            time: pr.created_at,
                        },
                        (IssueState::Open, _, labels) => {
                            PullRequestStatus::find_status(
                                labels
                                    .into_iter()
                                    .map(|label| label.name.to_string())
                                    .collect::<Vec<String>>(),
                                pr.updated_at,
                                None,
                            )
                                .unwrap_or(PullRequestStatus::Open {
                                    time: pr.created_at,
                                })
                        }
                        (IssueState::Closed, None, _) => PullRequestStatus::Closed {
                            time: pr.closed_at.expect("Missing closed time"),
                        },
                        (IssueState::Closed, Some(_), _) => PullRequestStatus::Merged {
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
                    events_history: events,
                    labels_history: labels,
                };
                parsed_prs.push(parsed);
            }
            inner_bar.finish_with_message("Done");
            multi.remove(&inner_bar);
            Ok(())
        })
            .await
            .unwrap();
    }

    pub(super) async fn parse_issues(
        &self,
        parsed_issues: &mut Vec<crate::db::model::issue::Issue>,
        parsed_users: &mut Vec<team_member::Contributor>,
        pr: octocrab::Page<octocrab::models::issues::Issue>,
        with_timeline: bool,
    ) {
        with_progress_bar_async(pr.items.len(), None, async |_bar, multi: &MultiProgress| {
            let inner_bar = multi.add(ProgressBar::new(pr.items.len() as u64));
            inner_bar.set_style(
                indicatif::ProgressStyle::default_bar()
                    .template("[{elapsed_precise}] {bar:40.green/cyan} {pos:>7}/{len:7} {msg}")?
                    .progress_chars("##-"),
            );

            for issue in pr.items {
                inner_bar.set_message(format!(
                    "Processing Issue:'{}' events {} timeline events and labels",
                    issue.number,
                    if with_timeline {
                        "with".to_string()
                    } else {
                        "without".to_string()
                    }
                ));
                inner_bar.inc(1);

                if issue.pull_request.is_some() {
                    log::warn!("Issue #{} is a pull request, skipping", issue.number);
                    continue;
                }

                parsed_users.push(team_member::Contributor::from(issue.user.clone()));

                let (labels, states) = if with_timeline {
                    self.fetch_and_parse_timeline(issue.number).await
                } else {
                    (None, None)
                };

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
                    created_at: issue.created_at,
                    status: last_state,
                    events_history: states,
                    labels_history: labels,
                };

                parsed_issues.push(parsed_issue);
            }
            inner_bar.finish_with_message("Done");
            multi.remove(&inner_bar);
            Ok(())
        })
            .await
            .unwrap();
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
                        .name;
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
                        .name;
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

    fn get_events(
        &self,
        issue_number: u64,
        timeline: &[TimelineEvent],
    ) -> anyhow::Result<Vec<crate::db::model::issue::IssueEvent>> {
        let mut vec = Vec::new();
        for event in timeline {
            match event.event {
                models::Event::Merged => {
                    let time = event
                        .created_at
                        .context(format!("cannot get time from Timeline event{:?}", event))?
                        .naive_utc();
                    vec.push(crate::db::model::issue::IssueEvent {
                        event: "merged".to_string(),
                        timestamp: time,
                    });
                }
                models::Event::Closed => {
                    let time = event
                        .created_at
                        .context(format!("cannot get time from Timeline event{:?}", event))?
                        .naive_utc();
                    vec.push(crate::db::model::issue::IssueEvent {
                        event: "closed".to_string(),
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
                    vec.push(crate::db::model::issue::IssueEvent {
                        event: "committed".to_string(),
                        timestamp: time,
                    });
                }
                models::Event::Commented => {
                    let time = event
                        .created_at
                        .context(format!("cannot get time from Timeline event{:?}", event))?
                        .naive_utc();
                    vec.push(crate::db::model::issue::IssueEvent {
                        event: "commented".to_string(),
                        timestamp: time,
                    });
                }
                models::Event::Reopened => {
                    let time = event
                        .created_at
                        .context(format!("cannot get time from Timeline event{:?}", event))?
                        .naive_utc();
                    vec.push(crate::db::model::issue::IssueEvent {
                        event: "reopened".to_string(),
                        timestamp: time,
                    });
                }
                models::Event::Reviewed => {
                    let time = event
                        .updated_at
                        .context(format!("cannot get time from Timeline event{:?}", event))?
                        .naive_utc();
                    vec.push(crate::db::model::issue::IssueEvent {
                        event: "reviewed".to_string(),
                        timestamp: time,
                    });
                }
                _ => {
                    log::trace!("not interesting timeline event: {:#?}", event.event);
                }
            }
        }
        Ok(vec)
    }
}
