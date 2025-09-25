use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::encode::IsNull;
use sqlx::error::BoxDynError;
use sqlx::{Database, Encode, Postgres, Row, Type};
use std::fmt::Display;
use crate::db::model::team_member::TeamMember;

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct PrEvent {
    pub pr_number: i64,
    pub author_id: octocrab::models::UserId,
    pub state: PullRequestStatus,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, sqlx::FromRow)]
pub struct FileActivity {
    pub pr: i64,
    pub file_path: String,
    pub user_id: octocrab::models::UserId,
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
        let pr_number: i64 = row.try_get("pr")?;
        let state: String = row.try_get("state")?;
        let timestamp: DateTime<Utc> = row.try_get("timestamp")?;
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
            pr_number,
            author_id: octocrab::models::UserId(author_id as u64),
            state: status,
        })
    }
}

impl PullRequestStatus {
    pub fn from_str(
        value: &str,
        time: DateTime<Utc>,
        merge_sha: Option<String>,
    ) -> Option<Self> {
        match value {
            "open" => Some(PullRequestStatus::Open { time }),
            "closed" => Some(PullRequestStatus::Closed { time }),
            "merged" => {
                if let Some(merge_sha) = merge_sha {
                    Some(PullRequestStatus::Merged { merge_sha, time })
                } else {
                    panic!("Merge SHA is required for merged state");
                }
            }
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
            .find_map(|label| PullRequestStatus::from_str(label, time, merge_sha.clone()))
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
