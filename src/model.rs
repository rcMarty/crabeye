use std::fmt::Display;
use git2::Oid;
use octocrab::models::IssueState;


#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PullRequest {
    pub commit_id: String,
    pub author: String,
    pub state: Option<IssueState>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
    pub files: FilesState,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum FilesState {
    Fetched {
        files: Vec<String>
    },
    NotFetched,
}
impl Display for FilesState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FilesState::Fetched { files } => write!(f, "files: {:?}", files),
            FilesState::NotFetched => write!(f, "files: NotFetched"),
        }
    }
}

impl PullRequest {
    pub fn get_commit_id(&self) -> anyhow::Result<Oid> {
        match Oid::from_str(&self.commit_id) {
            Ok(oid) => Ok(oid),
            Err(e) => Err(anyhow::anyhow!("failed to parse commit_id: {}", e))
        }
    }
}

impl Display for PullRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "commit_id: {}, \
        author: {}, \
        state: {:?}, \
        title: {:?}, \
        description: {}, \
        files: {:?}",
               self.commit_id,
               self.author,
               self.state,
               self.title.clone().unwrap_or("No title".to_string()),
               self.description.clone().unwrap_or("No description".to_string()),
               self.files)
    }
}

