use crate::db::Database;

#[derive(Debug, Clone)]
pub struct AppState {
    pub db: Database,
}
