use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::error::BoxDynError;
use sqlx::sqlite::{SqliteArgumentValue, SqliteTypeInfo};
use sqlx::{Encode, Row, Sqlite, Type};
use std::fmt::Display;

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct PrEvent {
    pub pr_number: i64,
    pub author_id: octocrab::models::UserId,
    pub state: PullRequestStatus,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum PullRequestStatus {
    Open {
        time: DateTime<Utc>,
    },
    Closed {
        time: DateTime<Utc>,
    },
    Waiting_for_review {
        time: DateTime<Utc>,
    },
    Waiting_for_merge {
        time: DateTime<Utc>,
    },
    Waiting_for_author {
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
            PullRequestStatus::Waiting_for_review { time } => *time,
            PullRequestStatus::Waiting_for_merge { time } => *time,
            PullRequestStatus::Waiting_for_author { time } => *time,
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

                PullRequestStatus::Waiting_for_review { time } => time,
                PullRequestStatus::Waiting_for_merge { time } => time,
                PullRequestStatus::Waiting_for_author { time } => time,
            }
        )
    }
}

// Custom FromRow implementation for PrEvent
impl<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> for PrEvent {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
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
            _ => return Err(sqlx::Error::Decode("Invalid state".into())),
        };

        Ok(PrEvent {
            pr_number,
            author_id: octocrab::models::UserId(author_id as u64),
            state: status,
        })
    }
}

impl Type<Sqlite> for PullRequestStatus {
    fn type_info() -> SqliteTypeInfo {
        <String as Type<Sqlite>>::type_info()
    }
}

impl<'q> Encode<'q, Sqlite> for PullRequestStatus {
    fn encode_by_ref(
        &self,
        buf: &mut Vec<SqliteArgumentValue<'q>>,
    ) -> Result<sqlx::encode::IsNull, BoxDynError> {
        let string_repr = match self {
            PullRequestStatus::Open { .. } => "open".to_string(),
            PullRequestStatus::Closed { .. } => "closed".to_string(),
            PullRequestStatus::Merged { .. } => "merged".to_string(),
            PullRequestStatus::Waiting_for_review { .. } => "waiting_for_review".to_string(),
            PullRequestStatus::Waiting_for_merge { .. } => "waiting_for_merge".to_string(),
            PullRequestStatus::Waiting_for_author { .. } => "waiting_for_author".to_string(),
        };

        <std::string::String as sqlx::Encode<'_, Sqlite>>::encode_by_ref(&string_repr, buf)
    }
}
