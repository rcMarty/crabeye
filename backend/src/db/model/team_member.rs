#[cfg(feature = "git")]
#[derive(sqlx::FromRow, serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct TeamMember {
    #[sqlx(try_from = "i64")]
    pub github_id: u64,
    pub github_name: String,
    pub name: String,
    pub teams: Vec<Team>,
}

#[cfg(feature = "git")]
#[derive(sqlx::FromRow, serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct Team {
    pub team: String,
    pub subteam_of: Option<String>,
    pub kind: rust_team_data::v1::TeamKind,
}

#[derive(
    sqlx::FromRow, serde::Serialize, serde::Deserialize, schemars::JsonSchema, Debug, Clone,
)]
pub struct Contributor {
    #[sqlx(try_from = "i64")]
    pub github_id: u64,
    pub github_name: String,
    pub name: Option<String>,
}

#[cfg(feature = "git")]
impl From<octocrab::models::Author> for Contributor {
    fn from(author: octocrab::models::Author) -> Self {
        Contributor {
            github_id: author.id.0,
            github_name: author.login,
            name: author.name,
        }
    }
}
