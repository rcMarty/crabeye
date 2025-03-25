pub mod model;

// src/db/mod.rs
use crate::db::model::pr_event::{FileActivity, PrEvent, PullRequestStatus};
use anyhow::Result;
use chrono::{Datelike, NaiveDate};
use sqlx::{Pool, Sqlite, SqlitePool};

#[derive(Debug, Clone)]
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
    ) -> Result<Vec<String>> {
        let timestamp_start =
            NaiveDate::from_ymd_opt(timestamp.year(), timestamp.month(), timestamp.day()).unwrap();
        let timestamp_end = timestamp_start + chrono::Duration::days(1);

        let record = sqlx::query!(
            r#"
SELECT distinct state FROM pr_event_log
WHERE pr = ? and timestamp between ? and ?
ORDER BY timestamp DESC
               "#,
            pr,
            timestamp_start,
            timestamp_end
        )
        .fetch_all(&self.pool)
        .await?;

        let ret = record.iter().map(|r| r.state.clone()).collect::<Vec<_>>();
        log::debug!("return value from get pr state at: \n{:?}", ret);

        Ok(ret)
    }

    /**
    Get the count of the state of the pull request at the given timestamp

    @param pr: i64 - the pull request number
    */
    pub async fn get_pr_count_in_state(
        &self,
        timestamp: chrono::DateTime<chrono::Utc>,
        state: PullRequestStatus,
    ) -> Result<i64> {
        let timestamp_start =
            NaiveDate::from_ymd_opt(timestamp.year(), timestamp.month(), timestamp.day()).unwrap();
        let timestamp_end = timestamp_start + chrono::Duration::days(1);

        let record = sqlx::query!(
            r#"
SELECT count(*) as count FROM pr_event_log
WHERE timestamp BETWEEN ? AND ? AND state = ?;
               "#,
            timestamp_start,
            timestamp_end,
            state
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(record.count)
    }

    pub async fn insert_file_activity(&self, activity: &FileActivity) -> Result<()> {
        let user_id = activity.user_id.0 as i64;
        sqlx::query!(
            r#"INSERT INTO file_activity
               (pr, file_path, user_login, timestamp)
               VALUES (?, ?, ?, ?)"#,
            activity.pr,
            activity.file_path,
            user_id,
            activity.timestamp
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
