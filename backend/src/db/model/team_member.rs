use crate::db::model::pr_event::{PrEvent, PullRequestStatus};
use chrono::Utc;
use sqlx::encode::IsNull;
use sqlx::error::BoxDynError;
use sqlx::Database;
use sqlx::Encode;
use sqlx::Postgres;
use sqlx::{FromRow, Row, Type};

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize, serde::Deserialize)]
pub struct TeamMember {
    pub github_id: u64,
    pub github_name: String,
    pub name: String,
    pub team: String,
    pub subteam_of: Option<String>,
    pub kind: rust_team_data::v1::TeamKind,
}

#[derive(
    Debug, Clone, sqlx::FromRow, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct Contributor {
    pub github_name: String,
    #[sqlx(try_from = "i64")]
    pub github_id: u64,
}

impl From<octocrab::models::Author> for Contributor {
    fn from(author: octocrab::models::Author) -> Self {
        Contributor {
            github_id: author.id.0,
            github_name: author.login,
        }
    }
}
