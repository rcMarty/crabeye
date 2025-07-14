use chrono::Utc;
use sqlx::Type;
use sqlx::error::BoxDynError;
use sqlx::Database;
use sqlx::Postgres;
use sqlx::Encode;
use sqlx::encode::IsNull;
use crate::db::model::pr_event::{PrEvent, PullRequestStatus};

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize, serde::Deserialize)]
pub struct TeamMember {
    pub github_id: u64,
    pub github_name: String,
    pub name: String,
    pub team: String,
    pub subteam_of: Option<String>,
    pub kind: rust_team_data::v1::TeamKind,
}


#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize, serde::Deserialize)]
pub struct Contributor {
    pub github_id: u64,
    pub github_name: String,
    // pub kind: rust_team_data::v1::TeamKind,
}

impl From<octocrab::models::Author> for Contributor {
    fn from(author: octocrab::models::Author) -> Self {
        Contributor {
            github_id: author.id.0,
            github_name: author.login,
        }
    }
}
