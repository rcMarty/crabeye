use crate::db::model::team_member::Contributor;

/// Response model for top files in a PR
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema, sqlx::FromRow)]
pub struct TopFilesResponse {
    /// File path
    pub file_path: String,
    /// ID of the PR
    pub pr_id: i64,
    /// Creator of the PR
    #[sqlx(flatten)]
    pub pr_creator: Contributor,
}