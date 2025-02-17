use std::fmt::Display;
use git2::Oid;
use octocrab::models::{Author};
use sqlx::{FromRow, Row, SqlitePool};
use sqlx::sqlite::SqliteRow;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct User {
    pub login: String,
    pub id: octocrab::models::UserId,
}

impl User {
    pub fn new(login: String, id: octocrab::models::UserId) -> Self {
        Self { login, id }
    }
    pub fn from_author(author: Author) -> Self {
        Self { login: author.login, id: author.id }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PullRequest {
    pub pr_number: u64,
    pub author: User,
    pub state: PullRequestStatus,
    pub title: Option<String>,
    pub description: Option<String>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
    pub files: FilesState,
}


impl PullRequest {
    pub fn merge_commit_id(&self) -> Option<Oid> {
        match &self.state {
            PullRequestStatus::Merged { merge_sha, .. } => {
                Oid::from_str(merge_sha).ok()
            }
            _ => None
        }
    }
}

//TODO: should i implement FromRow for PullRequest?

// impl FromRow<SqliteRow> for PullRequest {
//     fn from_row(row: &SqliteRow) -> anyhow::Result<Self> {
//         Ok(Self {
//             pr_number: row.get("pr_number"),
//             author: User::new(row.get("author_login"), row.get("author_id")),
//             state:
//
//         })
//     }
// }

impl Display for PullRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "author: {:?}, \
        state: {:?}, \
        title: {:?}, \
        description: {}, \
        files: {:?}",
               self.author,
               self.state,
               self.title.clone().unwrap_or("No title".to_string()),
               self.description.clone().unwrap_or("No description".to_string()),
               self.files)
    }
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum PullRequestStatus {
    Open,
    Closed {
        closed_at: chrono::DateTime<chrono::Utc>,
    },
    Merged {
        closed_at: chrono::DateTime<chrono::Utc>,
        merge_sha: String,
    },
}
