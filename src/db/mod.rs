#[allow(unused)]
pub mod model;

// src/db/mod.rs
use crate::db::model::pr_event::{FileActivity, PrEvent, PullRequestStatus};
use anyhow::Result;
use chrono::{Datelike, NaiveDate, Utc};
use rust_team_data::v1::PermissionPerson;
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

    /// Cleans the old users from table and insert new users
    pub async fn insert_team_members(&self, team_members: &[PermissionPerson]) -> Result<()> {
        sqlx::query!(
            r#"-- noinspection SqlWithoutWhereForFile
DELETE FROM team_members
"#
        )
        .execute(&self.pool)
        .await?;

        for user in team_members.iter() {
            let gh_id = user.github_id as i64;
            sqlx::query!(
                r#"
INSERT INTO team_members (github_id, github_name, name)
VALUES (?,?,?)"#,
                gh_id,
                user.github,
                user.name
            )
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    pub async fn insert_pr_event(&self, event: &PrEvent) -> Result<()> {
        let timestamp = event.get_timestamp();
        let merge_sha = event.get_merge_sha();
        let author_id = event.author_id.0 as i64;
        sqlx::query!(
            r#"
INSERT INTO pr_event_log (pr, state,timestamp, merge_sha, author_id) 
VALUES (?,?,?,?,?)
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

// part where is analyzing functions
impl Database {
    /**
    TEMP: Jaký byl stav konkrétního PR v daný timestamp?
    */
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
    TEMP: Jaký byl počet PR v daném stavu (waiting for review, waiting for author, waiting for bors, merged) v daný timestamp/den.

    Get the count of the state of the pull request at the given timestamp
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

    /**
    TEMP -- 3) Pro daného uživatele/tým (z https://github.com/rust-lang/team), jakých je top N souborů, které byly buď upraveny nebo reviewovány za posledních N časových jednotek?
    */
    pub async fn get_top_n_files(
        &self,
        user_id: i64,
        pr_id: i64,
        timestamp: chrono::DateTime<chrono::Utc>,
        n: i64,
    ) -> Result<Vec<(String, i64)>> {
        let timestamp_start =
            NaiveDate::from_ymd_opt(timestamp.year(), timestamp.month(), timestamp.day()).unwrap();
        let timestamp_end = timestamp_start + chrono::Duration::days(1);

        let record = sqlx::query!(
            r#"
select pr, file_path
from file_activity
where user_login = ?
  and timestamp between ? and ?
  and pr = ?
order by timestamp DESC
limit ?;
"#,
            user_id,
            timestamp_start,
            timestamp_end,
            pr_id,
            n
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(record
            .iter()
            .map(|r| (r.file_path.clone(), r.pr))
            .collect::<Vec<_>>())
    }

    /**
    TEMP Pro daný soubor/složku, kteří uživatelé/týmy jej v posledních N časových jednotkách upravovali nebo reviewovali?
    TODO mby add on which pr it was
    */
    pub async fn get_users_who_modified_file(
        &self,
        file_path: String,
        timestamp: chrono::DateTime<chrono::Utc>,
        n: i64,
    ) -> Result<Vec<String>> {
        let timestamp_start =
            NaiveDate::from_ymd_opt(timestamp.year(), timestamp.month(), timestamp.day()).unwrap();
        let timestamp_end = timestamp_start + chrono::Duration::days(1);
        let file_path = format!("{}%", file_path);

        let record = sqlx::query!(
            r#"
select distinct user_login
from file_activity
where file_path like ?
  and timestamp between ? and ?;
"#,
            file_path,
            timestamp_start,
            timestamp_end
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(record
            .iter()
            .map(|r| r.user_login.clone())
            .collect::<Vec<_>>())
    }

    /**
    temp -- 5) dotaz: PR, které čekají nejdelší dobu na review (jednodušší verze: jsou nejdelší čas ve stavu "waiting-on-review",
    */
    pub async fn get_prs_waiting_for_review(
        &self,
        timestamp: chrono::DateTime<chrono::Utc>,
        n: i64,
    ) -> Result<Vec<(i64, chrono::DateTime<chrono::Utc>)>> {
        let timestamp_start =
            NaiveDate::from_ymd_opt(timestamp.year(), timestamp.month(), timestamp.day()).unwrap();
        let timestamp_end = timestamp_start + chrono::Duration::days(1);

        let record = sqlx::query!(
            r#"
select pr, timestamp
from pr_event_log
where state = 'S-waiting-on-review'
   or state = 'S-waiting-on-bors'
   or state = 'S-waiting-on-author'
order by timestamp;
"#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(record
            .iter()
            .map(|r| {
                (
                    r.pr,
                    chrono::DateTime::<Utc>::from_naive_utc_and_offset(r.timestamp, Utc),
                )
            })
            .collect::<Vec<_>>())
    }
}
