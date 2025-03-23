pub mod model;

// src/db/mod.rs
use crate::db::model::pr_event::{FileActivity, PrEvent};
use anyhow::Result;
use sqlx::{Pool, Sqlite, SqlitePool};

pub struct Database {
    pub pool: Pool<Sqlite>,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self> {
        // i guess this is useless since sqlx is checking database in compiletime already
        // if !Sqlite::database_exists(database_url).await? {
        //     Sqlite::create_database(database_url).await?;
        // }

        let pool = SqlitePool::connect(database_url).await?;
        sqlx::migrate!().run(&pool).await?;

        Ok(Self { pool })
    }

    pub async fn insert_pr_event(&self, event: &PrEvent) -> Result<()> {
        let timestamp = event.get_timestamp();
        let merge_sha = event.get_merge_sha();
        let author_id = event.author_id.0 as i64;
        sqlx::query!(
            r#"
INSERT INTO pr_event_log (pr, state,timestamp, merge_sha, author_id) 
VALUES (?, ?,?,?,?)
"#,
            event.pr_number,
            event.state,
            timestamp,
            merge_sha,
            author_id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_pr_state_at(
        &self,
        pr: i64,
        timestamp: chrono::DateTime<chrono::Utc>,
    ) -> Result<Option<String>> {
        let record = sqlx::query!(
            r#"SELECT state FROM pr_event_log 
               WHERE pr = ? AND timestamp <= ?
               ORDER BY timestamp DESC
               LIMIT 1"#,
            pr,
            timestamp
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(record.map(|r| r.state))
    }

    pub async fn insert_file_activity(&self, activity: &FileActivity) -> Result<()> {
        sqlx::query!(
            r#"INSERT INTO file_activity
               (pr, file_path, user_login, timestamp)
               VALUES (?, ?, ?, ?)"#,
            activity.pr,
            activity.file_path,
            activity.user_login,
            activity.timestamp
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
