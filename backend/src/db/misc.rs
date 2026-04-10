use super::*;

/// Miscellaneous helper queries used by background jobs and the backfill pipeline.
impl Database {
    pub async fn get_all_teams(&self) -> Result<Vec<String>> {
        let records = sqlx::query!(
            r#"
SELECT team FROM teams
"#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(records.into_iter().map(|r| r.team).collect())
    }

    /// Returns the timestamp of the most recent event stored in `issue_event_history`
    /// for the given repository, or `None` if no events have been recorded yet.
    ///
    /// # Errors
    /// Returns an error on SQL/DB failure.
    pub async fn get_last_issue_event_timestamp(
        &self,
        repository: &str,
    ) -> Result<Option<NaiveDateTime>> {
        let record = sqlx::query!(
            r#"
SELECT MAX(timestamp) as timestamp
FROM issue_event_history
WHERE repository = $1
"#,
            repository
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(record.timestamp)
    }

    /// Looks up contributors whose `github_name` contains `github_name` (case-insensitive ILIKE).
    ///
    /// Returns `Ok(Some(Vec<Contributor>))` when at least one match is found, or `Ok(None)`
    /// when the search returns no rows.
    ///
    /// # Errors
    /// Returns an error on SQL/DB failure.
    pub async fn get_user_id_by_name(
        &self,
        github_name: &String,
    ) -> Result<Option<Vec<Contributor>>> {
        let github_name = format!("%{}%", github_name);
        let record: Vec<Contributor> = sqlx::query_as::<_, Contributor>(
            r#"
SELECT name,github_name,github_id FROM contributors
WHERE github_name ilike $1
"#,
        )
        .bind(github_name)
        .fetch_all(&self.pool)
        .await?;

        Ok(if record.is_empty() {
            None
        } else {
            Some(record)
        })
    }

    /// Returns all issues/PRs that have no rows in `issue_event_history`.
    ///
    /// Used by the backfill pipeline to find records that need their history fetched
    /// from GitHub and inserted retroactively.
    ///
    /// # Errors
    /// Returns an error on SQL/DB failure.
    pub async fn get_issues_without_history(&self) -> Result<Vec<BackfillRecord>> {
        let records = sqlx::query_as::<Postgres, BackfillRecord>(
            r#"
SELECT
    i.repository  AS repository,
    i.issue       AS issue_number,
    i.is_pr       AS is_pr,
    i.contributor_id AS author_id
FROM issues i
WHERE NOT EXISTS (
    SELECT 1
    FROM issue_event_history e
    WHERE e.repository = i.repository
      AND e.issue = i.issue
)
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(records)
    }

    /// Returns the timestamp of the most recent event in `issue_event_history` for a specific
    /// repository + issue pair, or `None` if no events have been recorded for it yet.
    ///
    /// # Errors
    /// Returns an error on SQL/DB failure.
    pub async fn get_last_update(
        &self,
        repository: &str,
        issue: i64,
    ) -> Result<Option<NaiveDateTime>> {
        let record = sqlx::query!(
            r#"
SELECT MAX(timestamp) as timestamp
FROM issue_event_history
WHERE repository = $1 AND issue = $2
"#,
            repository,
            issue
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(record.timestamp)
    }
}
