use octocrab::models::Author;

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize, serde::Deserialize)]
pub struct User {
    pub login: String,
    pub id: octocrab::models::UserId,
}

impl User {
    pub fn new(login: String, id: octocrab::models::UserId) -> Self {
        Self { login, id }
    }
    pub fn from_author(author: Author) -> Self {
        Self {
            login: author.login,
            id: author.id,
        }
    }
}
