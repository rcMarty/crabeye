use super::*;

/// Read-only analytical queries for PRs, issues and file activity.
impl Database {
    // Změna: Místo slice IDček bereme rovnou jméno týmu jako string
    pub async fn get_files_modified_by_team(
        &self,
        repository: &str,
        team_name: &str,
        from_timestamp: Option<NaiveDate>,
        last_n_days: Option<i64>,
        pagination: Pagination,
    ) -> Result<PaginatedResponse<String>> {
        let timestamp_end = from_timestamp.unwrap_or_else(|| Utc::now().date_naive()).and_hms_opt(0, 0, 0).unwrap();
        let timestamp_start = timestamp_end - chrono::Duration::days(last_n_days.unwrap_or(7));
        log::debug!("timestamp_ start {} end {}", timestamp_start, timestamp_end);

        let (limit, offset) = pagination.limit_offset();

        let count = sqlx::query!(
        r#"
SELECT COUNT(DISTINCT fa.file_path) as count
FROM file_activity fa
-- TADY JE TA MAGIE: Propojíme to s tabulkou týmů
JOIN contributors_teams ct ON fa.contributor_id = ct.contributor_id
WHERE fa.repository = $1
  AND fa.timestamp BETWEEN $2 AND $3
  AND ct.team = $4
        "#,
        repository,
        timestamp_start,
        timestamp_end,
        team_name
    )
            .fetch_one(&self.pool)
            .await?
            .count
            .unwrap_or(0) as usize;

        let entries = sqlx::query_scalar::<_, String>(
            r#"
SELECT DISTINCT fa.file_path
FROM file_activity fa
JOIN contributors_teams ct ON fa.contributor_id = ct.contributor_id
WHERE fa.repository = $1
  AND fa.timestamp BETWEEN $2 AND $3
  AND ct.team = $4
ORDER BY fa.file_path
OFFSET $5 LIMIT $6
        "#,
        )
            .bind(repository)
            .bind(timestamp_start)
            .bind(timestamp_end)
            .bind(team_name)
            .bind(offset)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;

        Ok(PaginatedResponse::new(count, pagination, entries))
    }


    /// Returns all distinct issue events recorded on the day that contains `timestamp`.
    ///
    /// Queries `issue_event_history` for rows where `is_pr = false`, filtered to the full
    /// calendar day `[00:00, 00:00 next day)`. Results are ordered by timestamp descending.
    /// Returns an empty vector if the issue does not exist in the database.
    ///
    /// # Errors
    /// Returns an error on SQL/DB failure.
    //TEMP: Jaký byl stav konkrétního PR v daný timestamp?
    pub async fn get_issue_events_at(
        &self,
        repository: &str,
        issue: i64,
        timestamp: NaiveDate,
    ) -> Result<Vec<IssueEvent>> {
        // Check if issue exists // low startup cost
        let exists = sqlx::query!(
            "SELECT EXISTS(SELECT 1 FROM issues WHERE repository = $1 AND issue = $2 AND is_pr = false) as exists",
            repository,
            issue
        )
            .fetch_one(&self.pool)
            .await?
            .exists
            .unwrap_or(false);
        if !exists {
            return Ok(vec![]);
        }

        let timestamp_start = timestamp.and_hms_opt(0, 0, 0).unwrap();
        let timestamp_end = timestamp_start + chrono::Duration::days(1);

        let ret = sqlx::query_as::<_, IssueEvent>(
            r#"
SELECT distinct hist.event as event, hist.timestamp as timestamp
FROM issue_event_history hist
WHERE hist.repository = $1 and hist.issue = $2 and hist.timestamp between $3 and $4 and hist.is_pr = false
ORDER BY timestamp DESC
"#,
        )
            .bind(repository)
            .bind(issue)
            .bind(timestamp_start)
            .bind(timestamp_end)
            .fetch_all(&self.pool)
            .await?;

        log::debug!("return value from get pr state at: \n{:?}", ret);
        Ok(ret)
    }

    /// Returns the full state of a PR at a given date, including its current `S-*` labels and all
    /// recorded events up to (and including) `timestamp`.
    ///
    /// Runs three queries inside a single logical read:
    /// 1. Fetches the latest `ADDED` `S-*` label per label name up to `timestamp`.
    /// 2. Fetches all distinct events up to `timestamp` ordered descending.
    /// 3. Fetches the base [`PrEvent`] row from `issues` (returns `None` if PR not found).
    ///
    /// Returns `Ok(None)` if the PR does not exist in the database.
    /// Returns `Ok(Some(PrEvent))` on success.
    ///
    /// # Errors
    /// Returns an error on SQL/DB failure.
    pub async fn get_pr_history_from(
        &self,
        repository: &str,
        pr: i64,
        timestamp: NaiveDate,
    ) -> Result<Option<PrEvent>> {
        let timestamp_start = timestamp.and_hms_opt(0, 0, 0).unwrap();

        let labels = sqlx::query_as::<_, IssueLabel>(
            r#"
SELECT
    subquery.label as label,
    subquery.timestamp as timestamp,
    subquery.action as label_event
FROM (
         SELECT DISTINCT ON (issue, label)
         *
         FROM issue_labels_history
         WHERE issue = $2 and repository = $1 and timestamp <= $3 and is_pr = true and label like 'S-%'
         ORDER BY issue, label, timestamp DESC
     ) subquery
WHERE action = 'ADDED';
"#,
        )
            .bind(repository)
            .bind(pr)
            .bind(timestamp_start)
            .fetch_all(&self.pool)
            .await?;

        let states = sqlx::query_as::<_, IssueEvent>(
            r#"
SELECT distinct hist.event as event, hist.timestamp as timestamp
FROM issue_event_history hist
WHERE hist.repository = $1 and hist.issue = $2 and hist.timestamp <= $3 and hist.is_pr = true
ORDER BY timestamp DESC
"#,
        )
            .bind(repository)
            .bind(pr)
            .bind(timestamp_start)
            .fetch_all(&self.pool)
            .await?;

        let mut pr = match sqlx::query_as::<_, PrEvent>(
            r#"
SELECT repository as repository, issue as pr, current_state as state, timestamp as timestamp, merge_sha as merge_sha, contributor_id as author_id
FROM issues
WHERE repository = $1 and issue = $2 and is_pr = true
"#,
        )
            .bind(repository)
            .bind(pr)
            .fetch_optional(&self.pool)
            .await?
        {
            Some(pr) => pr,
            None => return Ok(None),
        };

        pr.labels_history = Some(labels);
        pr.events_history = Some(states);

        log::debug!("return value from get pr state at: \n{:?}", pr.clone());
        Ok(Some(pr))
    }

    /// Returns the number of PRs that had a given event recorded on the day of `timestamp`.
    ///
    /// Counts rows in `issue_event_history` where `is_pr = true`, `event` matches
    /// `state.to_string()`, and the timestamp falls within the calendar day
    /// `[00:00, 00:00 next day)`.
    ///
    /// # Errors
    /// Returns an error on SQL/DB failure.
    //TEMP: Jaký byl počet PR v daném stavu (waiting for review, waiting for author, waiting for bors, merged) v daný timestamp/den.
    pub async fn get_pr_count_in_state(
        &self,
        repository: &str,
        timestamp: NaiveDate,
        state: PullRequestStatusRequest,
    ) -> Result<i64> {
        let timestamp_start = timestamp.and_hms_opt(0, 0, 0).unwrap();
        let timestamp_end = timestamp_start + chrono::Duration::days(1);

        //TODO to je naprosto špatně
        let record = sqlx::query!(
            r#"
SELECT count(*) as count FROM issue_event_history
WHERE timestamp BETWEEN $1 AND $2
  AND event = $3
  AND repository = $4
  AND is_pr = true;
               "#,
            timestamp_start,
            timestamp_end,
            state.to_string(),
            repository
        )
            .fetch_one(&self.pool)
            .await?;

        Ok(record.count.unwrap())
    }

    /// Returns up to `n` most recent file activity records for the given contributors
    /// within the last `duration` days, scoped to `repository`.
    ///
    /// The time window is `[today - duration, today)` (day-aligned, UTC).
    /// Results are ordered by activity timestamp descending.
    ///
    /// # Errors
    /// Returns an error on SQL/DB failure.
    //TEMP -- 3) Pro daného uživatele/tým (z https://github.com/rust-lang/team), jakých je top N souborů, které byly buď upraveny nebo reviewovány za posledních N časových jednotek?
    pub async fn get_top_n_files(
        &self,
        repository: &str,
        contributors: Vec<Contributor>,
        duration: chrono::Duration,
        n: i64,
    ) -> Result<Vec<TopFilesResponse>> {
        let timestamp_end = Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap();
        let timestamp_start = timestamp_end - duration;
        let ids = contributors
            .iter()
            .map(|c| c.github_id as i64)
            .collect::<Vec<i64>>();

        let record = sqlx::query_as::<_, TopFilesResponse>(
            r#"--
select issue as pr_id, repository, file_path, github_id, github_name, name
from file_activity
join contributors c on file_activity.contributor_id = c.github_id
where contributor_id = ANY($1)
  and timestamp between $2 and $3
  and repository = $4
order by timestamp DESC
LIMIT $5;
"#,
        )
            .bind(&ids)
            .bind(timestamp_start)
            .bind(timestamp_end)
            .bind(repository)
            .bind(n)
            .fetch_all(&self.pool)
            .await?;
        Ok(record)
    }

    /// Returns a paginated list of contributors who touched files matching `file_path` prefix
    /// within the given time window.
    ///
    /// `file_path` is treated as a SQL `LIKE` prefix (`file_path%`). The search window ends at
    /// `from_timestamp` (defaults to today) and extends back `last_n_days` days (defaults to 7).
    /// Two queries are run: one for the total distinct-contributor count, one for the paginated page.
    ///
    /// # Errors
    /// Returns an error on SQL/DB failure.
    //TEMP Pro daný soubor/složku, kteří uživatelé/týmy jej v posledních N dní od určitého datumu upravovali nebo reviewovali?
    //TODO mby add on which pr it was
    pub async fn get_users_who_modified_file(
        &self,
        repository: &str,
        file_path: String,
        from_timestamp: Option<NaiveDate>,
        last_n_days: Option<i64>,
        pagination: Pagination,
    ) -> Result<PaginatedResponse<Contributor>> {
        let timestamp_end = from_timestamp
            .unwrap_or(Utc::now().date_naive())
            .and_hms_opt(0, 0, 0)
            .unwrap();
        let timestamp_start = timestamp_end - chrono::Duration::days(last_n_days.unwrap_or(7));
        log::debug!("timestamp_ start {} end {}", timestamp_start, timestamp_end);
        let file_path = format!("{}%", file_path);

        let (limit, offset) = pagination.limit_offset();

        let count = sqlx::query!(
            r#"
select count(distinct github_id) as count
from contributors
where github_id in
(
        select distinct contributor_id
        from file_activity
        where file_path like $1
            and timestamp between $2 and $3
            and repository = $4
        );
"#,
            file_path,
            timestamp_start,
            timestamp_end,
            repository
        )
            .fetch_one(&self.pool)
            .await?
            .count
            .unwrap_or(0) as usize;

        let entries = sqlx::query_as::<_, Contributor>(
            r#"
select distinct github_id, github_name, name
from contributors
where github_id in
      (
        select distinct contributor_id
        from file_activity
        where file_path like $1
            and timestamp between $2 and $3
            and repository = $4
        order by contributor_id
        offset $5 limit $6
        );
"#,
        )
            .bind(file_path)
            .bind(timestamp_start)
            .bind(timestamp_end)
            .bind(repository)
            .bind(offset)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;

        Ok(PaginatedResponse::new(count, pagination, entries))
    }

    /// Returns a paginated list of PRs whose most recent label is one of the three
    /// waiting states: `S-waiting-on-review`, `S-waiting-on-bors`, or `S-waiting-on-author`.
    ///
    /// Uses a CTE that selects the latest label event per `(issue, label)` pair and keeps
    /// only those where the last action was `ADDED`. Includes count of matching PRs for
    /// pagination metadata.
    ///
    /// # Errors
    /// Returns an error on SQL/DB failure.
    //temp -- 5) dotaz: PR, které čekají nejdelší dobu na review (jednodušší verze: jsou nejdelší čas ve stavu "waiting-on-review",
    pub async fn get_prs_waiting_for_review(
        &self,
        repository: &str,
        pagination: Pagination,
    ) -> Result<PaginatedResponse<PrEvent>> {
        log::debug!(
            "get_prs_waiting_for_review called with pagination: page {}, per_page {}",
            pagination.page,
            pagination.per_page
        );

        let (limit, offset) = pagination.limit_offset();
        let count = sqlx::query!(
            r#"
WITH current_waiting_labels AS (
    SELECT DISTINCT ON (issue, label)
        issue,
        label,
        action
    FROM issue_labels_history
    WHERE repository = $1
      AND label IN ('S-waiting-on-review', 'S-waiting-on-bors', 'S-waiting-on-author')
    ORDER BY issue, label, timestamp DESC
)
SELECT COUNT(DISTINCT l.issue)
FROM current_waiting_labels l
JOIN issues c ON l.issue = c.issue AND c.repository = $1
WHERE l.action = 'ADDED'
  AND c.is_pr = true;
"#,
            repository
        )
            .fetch_one(&self.pool)
            .await?
            .count
            .unwrap_or(0) as usize;

        let record = sqlx::query_as::<_, PrEvent>(
            r#"
WITH current_waiting_labels AS (
    SELECT DISTINCT ON (issue, label)
        issue,
        label,
        timestamp,
        action
    FROM issue_labels_history
    WHERE repository = $1
      AND label IN ('S-waiting-on-review', 'S-waiting-on-bors', 'S-waiting-on-author')
    ORDER BY issue, label, timestamp DESC
)
SELECT
    c.repository AS repository,
    l.issue     AS pr,
    l.label     AS state,
    l.timestamp AS timestamp,
    c.merge_sha AS merge_sha,
    c.contributor_id AS author_id
FROM current_waiting_labels l
JOIN issues c ON l.issue = c.issue AND c.repository = $1
WHERE l.action = 'ADDED'
  AND c.is_pr = true
OFFSET $2
LIMIT $3;
        "#,
        )
            .bind(repository)
            .bind(offset)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;

        log::debug!(
            "return value from get_prs_waiting_for_review: \n{:?}",
            record
        );

        Ok(PaginatedResponse::new(count, pagination, record))
    }
}
