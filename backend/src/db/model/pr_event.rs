use crate::db::model::issue::{Issue, IssueLabel, IssueEvent};
use crate::db::model::team_member::TeamMember;
use crate::db::model::IssueLike;
use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::encode::IsNull;
use sqlx::error::BoxDynError;
use sqlx::{Database, Encode, Postgres, Row, Type};
use std::fmt::Display;

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PrEvent {
    pub repository: String,
    pub pr_number: i64,
    pub author_id: i64,
    pub state: PullRequestStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub events_history: Option<Vec<IssueEvent>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels_history: Option<Vec<IssueLabel>>,
}

impl IssueLike for PrEvent {
    fn events_history(&self) -> Option<&Vec<IssueEvent>> {
        self.events_history.as_ref()
    }
    fn labels_history(&self) -> Option<&Vec<IssueLabel>> {
        self.labels_history.as_ref()
    }
    fn repository(&self) -> &String {
        &self.repository
    }
    fn issue_number(&self) -> i64 {
        self.pr_number
    }
    fn author_id(&self) -> i64 {
        self.author_id
    }
    fn is_pr(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub enum PullRequestStatus {
    WaitingForReview {
        time: DateTime<Utc>,
    },
    WaitingForBors {
        time: DateTime<Utc>,
    },
    WaitingForAuthor {
        time: DateTime<Utc>,
    },
    Open {
        time: DateTime<Utc>,
    },
    Closed {
        time: DateTime<Utc>,
    },
    Merged {
        merge_sha: String,
        time: DateTime<Utc>,
    },
}

/// Represents a request to filter pull requests by their status in API calls
#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub enum PullRequestStatusRequest {
    WaitingForReview,
    WaitingForBors,
    WaitingForAuthor,
    Open,
    Closed,
    Merged,
}

impl<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> for PullRequestStatus {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        let state: String = row.try_get("state")?;
        let timestamp: DateTime<Utc> =
            DateTime::<Utc>::from_naive_utc_and_offset(row.try_get("timestamp")?, Utc);
        let merge_sha: Option<String> = row.try_get("merge_sha")?;

        match state.as_str() {
            "open" => Ok(PullRequestStatus::Open { time: timestamp }),
            "closed" => Ok(PullRequestStatus::Closed { time: timestamp }),
            "merged" => {
                if let Some(merge_sha) = merge_sha {
                    Ok(PullRequestStatus::Merged {
                        merge_sha,
                        time: timestamp,
                    })
                } else {
                    Err(sqlx::Error::Decode(
                        "Merge SHA is required for merged state".into(),
                    ))
                }
            }
            "S-waiting-on-review" => Ok(PullRequestStatus::WaitingForReview { time: timestamp }),
            "S-waiting-on-bors" => Ok(PullRequestStatus::WaitingForBors { time: timestamp }),
            "S-waiting-on-author" => Ok(PullRequestStatus::WaitingForAuthor { time: timestamp }),
            _ => Err(sqlx::Error::Decode("Invalid state".into())),
        }
    }
}

impl Display for PullRequestStatusRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state_str = match self {
            PullRequestStatusRequest::WaitingForReview => "S-waiting-on-review",
            PullRequestStatusRequest::WaitingForBors => "S-waiting-on-bors",
            PullRequestStatusRequest::WaitingForAuthor => "S-waiting-on-author",

            PullRequestStatusRequest::Open => "open",
            PullRequestStatusRequest::Closed => "closed",
            PullRequestStatusRequest::Merged => "merged",
        };
        write!(f, "{}", state_str)
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct FileActivity {
    pub repository: String,
    pub pr: i64,
    pub file_path: String,
    pub user_id: i64,
    pub timestamp: DateTime<Utc>,
}

impl PrEvent {
    pub fn get_timestamp(&self) -> DateTime<Utc> {
        match &self.state {
            PullRequestStatus::Open { time } => *time,
            PullRequestStatus::Closed { time } => *time,
            PullRequestStatus::Merged { time, .. } => *time,

            PullRequestStatus::WaitingForReview { time } => *time,
            PullRequestStatus::WaitingForBors { time } => *time,
            PullRequestStatus::WaitingForAuthor { time } => *time,
        }
    }

    pub fn get_merge_sha(&self) -> Option<String> {
        match &self.state {
            PullRequestStatus::Merged { merge_sha, .. } => Some(merge_sha.clone()),
            _ => None,
        }
    }
}

impl Display for PrEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PR #{}: {:?} at {}",
            self.pr_number,
            self.state,
            match &self.state {
                PullRequestStatus::Open { time } => time,
                PullRequestStatus::Closed { time } => time,
                PullRequestStatus::Merged { merge_sha, time } => time,

                PullRequestStatus::WaitingForReview { time } => time,
                PullRequestStatus::WaitingForBors { time } => time,
                PullRequestStatus::WaitingForAuthor { time } => time,
            }
        )
    }
}

///Custom FromRow implementation for PrEvent
impl<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> for PrEvent {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        let repository: String = row.try_get("repository")?;
        let pr_number: i64 = row.try_get("pr")?;
        let state: String = row.try_get("state")?;
        let timestamp: DateTime<Utc> =
            DateTime::<Utc>::from_naive_utc_and_offset(row.try_get("timestamp")?, Utc);
        let merge_sha: Option<String> = row.try_get("merge_sha")?;
        let author_id: i64 = row.try_get("author_id")?;

        let status = match state.as_str() {
            "open" => PullRequestStatus::Open { time: timestamp },
            "closed" => PullRequestStatus::Closed { time: timestamp },
            "merged" => PullRequestStatus::Merged {
                merge_sha: merge_sha.unwrap_or_default(),
                time: timestamp,
            },
            "S-waiting-on-review" => PullRequestStatus::WaitingForReview { time: timestamp },
            "S-waiting-on-bors" => PullRequestStatus::WaitingForBors { time: timestamp },
            "S-waiting-on-author" => PullRequestStatus::WaitingForAuthor { time: timestamp },

            _ => return Err(sqlx::Error::Decode("Invalid state".into())),
        };

        Ok(PrEvent {
            repository,
            pr_number,
            author_id,
            state: status,
            events_history: None,
            labels_history: None,
        })
    }
}

impl PullRequestStatus {
    pub fn from_parts(value: &str, time: DateTime<Utc>, merge_sha: Option<String>) -> Option<Self> {
        match value {
            "open" => Some(PullRequestStatus::Open { time }),
            "closed" => Some(PullRequestStatus::Closed { time }),
            "merged" => merge_sha
                .map(|sha| PullRequestStatus::Merged {
                    merge_sha: sha,
                    time,
                })
                .or_else(|| {
                    log::warn!("Merge SHA is required for merged state, but was not provided.");
                    None
                }),
            "S-waiting-on-review" => Some(PullRequestStatus::WaitingForReview { time }),
            "S-waiting-on-bors" => Some(PullRequestStatus::WaitingForBors { time }),
            "S-waiting-on-author" => Some(PullRequestStatus::WaitingForAuthor { time }),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            PullRequestStatus::Open { .. } => "open",
            PullRequestStatus::Closed { .. } => "closed",
            PullRequestStatus::Merged { .. } => "merged",

            PullRequestStatus::WaitingForReview { .. } => "S-waiting-on-review",
            PullRequestStatus::WaitingForBors { .. } => "S-waiting-on-bors",
            PullRequestStatus::WaitingForAuthor { .. } => "S-waiting-on-author",
        }
    }

    /// Find the first matching status in the vector
    pub fn find_status(
        vector: Vec<String>,
        time: DateTime<Utc>,
        merge_sha: Option<String>,
    ) -> Option<PullRequestStatus> {
        vector
            .iter()
            .find_map(|label| PullRequestStatus::from_parts(label, time, merge_sha.clone()))
    }
}

/// Custom Encode implementation for PullRequestStatus
impl<'q> Encode<'q, Postgres> for PullRequestStatus {
    fn encode_by_ref(
        &self,
        buf: &mut <Postgres as Database>::ArgumentBuffer<'q>,
    ) -> Result<IsNull, BoxDynError> {
        <String as sqlx::Encode<'q, Postgres>>::encode_by_ref(&self.as_str().to_string(), buf)
    }
}
/// Tell postgres into what type to decode the value
impl sqlx::Type<Postgres> for PullRequestStatus {
    fn type_info() -> <Postgres as sqlx::Database>::TypeInfo {
        <String as sqlx::Type<Postgres>>::type_info()
    }
}
