#![allow(unused)]
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::hash::Hash;
use sqlx::sqlite::{SqlitePool, SqliteRow};
use crate::model::{PullRequest, User};

pub struct DatabaseConnection {
    connection: SqlitePool,
}

impl DatabaseConnection {
    pub async fn new(sqlite_pool: SqlitePool) -> Self {
        match sqlx::migrate!().run(&sqlite_pool).await {
            Ok(_) => log::info!("Database migrated"),
            Err(e) => {
                if e.to_string().contains("already exists") {
                    log::info!("Database already exists")
                } else { log::error!("Failed to migrate database: {:?}", e) }
            }
        };
        Self {
            connection: sqlite_pool,
        }
    }


    pub async fn save_files_from_all_prs(&self, prs: HashMap<u64, HashSet<String>>) -> anyhow::Result<()> {
        for (pr_id, str_hashset) in prs {
            let files = str_hashset.into_iter().collect::<String>();
            self.save_files_to_pr(pr_id, files).await?;
        }
        Ok(())
    }

    pub async fn save_files_to_pr(&self, pr: u64, files: String) -> anyhow::Result<()> {
        let pr = pr.to_string();
        sqlx::query!("INSERT INTO pull_requests (pr_number, files_state) VALUES ($1, $2) ON CONFLICT DO NOTHING",
                pr,
                files)
            .execute(&self.connection)
            .await
            .expect("Failed to save file to database");

        log::debug!("Files saved: {} for pr {}", files, pr);
        Ok(())
    }

    pub async fn upsert_user(&self, user: User) -> anyhow::Result<()> {
        let id = user.id.to_string();
        sqlx::query!("INSERT INTO users (login, id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
                user.login,
                id)
            .execute(&self.connection)
            .await
            .expect("Failed to save user to database");

        log::debug!("User saved: {:?}", user);
        Ok(())
    }

    pub async fn get_pull_requests(&self) -> Vec<SqliteRow> {
        let pull_requests = sqlx::query("SELECT * FROM pull_requests")
            .fetch_all(&self.connection)
            .await
            .expect("Failed to fetch pull requests");
        pull_requests
    }
}

