use sqlx::Type;
use sqlx::error::BoxDynError;
use sqlx::Database;
use sqlx::Postgres;
use sqlx::Encode;
use sqlx::encode::IsNull;

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize, serde::Deserialize)]
pub struct TeamMember {
    pub github_id: u64,
    pub github_name: String,
    pub name: String,
    pub team: String,
    pub subteam_of: Option<String>,
    pub kind: rust_team_data::v1::TeamKind,
}


// 
// #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
// #[serde(rename_all_fields = "kebab-case")]
// pub enum TeamKind {
//     Team,
//     WorkingGroup,
//     ProjectGroup,
//     MarkerTeam,
// }
// 
// impl TeamKind {
//     pub fn from_str(kind: &str) -> Result<Self, anyhow::Error> {
//         match kind {
//             "team" => Ok(TeamKind::Team),
//             "working-group" => Ok(TeamKind::WorkingGroup),
//             "project-group" => Ok(TeamKind::ProjectGroup),
//             "marker-team" => Ok(TeamKind::MarkerTeam),
//             _ => Err(anyhow::anyhow!("Invalid team kind: {}", kind)),
//         }
//     }
// }