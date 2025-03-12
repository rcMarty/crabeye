use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::error::BoxDynError;
use sqlx::sqlite::{SqliteArgumentValue, SqliteTypeInfo};
use sqlx::{Encode, Sqlite, Type};
use std::fmt::Display;

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct PrEvent {
    pub pr_number: i64,
    pub state: PullRequestStatus,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum PullRequestStatus {
    Open,
    Closed,
    Merged { merge_sha: String },
}

#[derive(Debug, sqlx::FromRow)]
pub struct FileActivity {
    pub pr: i64,
    pub file_path: String,
    pub user_login: String,
    pub timestamp: DateTime<Utc>,
}

impl Display for PrEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PR #{}: {:?} at {}",
            self.pr_number, self.state, self.timestamp
        )
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
            PullRequestStatus::Open => "open".to_string(),
            PullRequestStatus::Closed => "closed".to_string(),
            PullRequestStatus::Merged { merge_sha } => format!("merged:{}", merge_sha),
        };

        <std::string::String as sqlx::Encode<'_, Sqlite>>::encode_by_ref(&string_repr, buf)
    }
}