use crate::db::model::pr_event::{PrEvent, PullRequestStatus};
use chrono::Utc;
use sqlx::encode::IsNull;
use sqlx::error::BoxDynError;
use sqlx::Database;
use sqlx::Encode;
use sqlx::Postgres;
use sqlx::{FromRow, Row, Type};
use std::hash::Hash;

#[derive(sqlx::FromRow, serde::Serialize, serde::Deserialize)]
#[derive(Hash, PartialEq, Eq, Debug, Clone, )]
pub struct TeamMember {
    #[sqlx(try_from = "i64")]
    pub github_id: u64,
    pub github_name: String,
    pub name: String,
    pub teams: Vec<Team>,
}


#[derive(sqlx::FromRow, serde::Serialize, serde::Deserialize)]
#[derive(PartialEq, Debug, Clone)]
pub struct Team {
    pub team: String,
    pub subteam_of: Option<String>,
    pub kind: rust_team_data::v1::TeamKind,
}
impl Eq for Team {}
impl Hash for Team {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.team.hash(state);
    }
}

#[derive(sqlx::FromRow, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[derive(Debug, Clone, )]
pub struct Contributor {
    #[sqlx(try_from = "i64")]
    pub github_id: u64,
    pub github_name: String,
    pub name: Option<String>,
}

impl From<octocrab::models::Author> for Contributor {
    fn from(author: octocrab::models::Author) -> Self {
        Contributor {
            github_id: author.id.0,
            github_name: author.login,
            name: author.name,
        }
    }
}
