use super::*;

/// Read-only analytical queries for PRs, issues and file activity.
impl Database {
    /// Returns a list of files modified by contributors of a given team within a specified time window.
    ///
    /// The time window is defined by `anchor_date` (defaults to today, end of window) and `last_n_days` (defaults to 7), and is aligned to full days (00:00 to 00:00). The query looks up contributors in the specified team and counts their modifications to files in the given repository within the time window. Results are ordered by modification count descending.
    /// # Errors
    /// Returns an error on SQL/DB failure.
    pub async fn get_files_modified_by_team(
        &self,
        repository: &str,
        team_name: &str,
        anchor_date: Option<NaiveDate>,
        last_n_days: Option<i64>,
    ) -> Result<HashMap<String, i64>> {
        let timestamp_end = anchor_date
            .unwrap_or_else(|| Utc::now().date_naive())
            .and_hms_opt(0, 0, 0)
            .unwrap();
        let timestamp_start = timestamp_end - chrono::Duration::days(last_n_days.unwrap_or(7));
        log::debug!("timestamp_ start {} end {}", timestamp_start, timestamp_end);

        let entries = sqlx::query_as::<_, (String, i64)>(
            r#"
SELECT fa.file_path, count(*) as editions
FROM file_activity fa
JOIN contributors_teams ct
  ON ct.contributor_id = fa.contributor_id
 AND ct.team = $4
WHERE fa.repository = $1
  AND fa.timestamp BETWEEN $2 AND $3
GROUP BY fa.file_path
ORDER BY editions DESC
        "#,
        )
            .bind(repository)
            .bind(timestamp_start)
            .bind(timestamp_end)
            .bind(team_name)
            .fetch_all(&self.pool)
            .await?;

        Ok(entries.into_iter().collect())
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
            log::warn!(
                "Issue {}#{} not found in database when querying events",
                repository,
                issue,
            );
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
SELECT repository as repository, issue as pr, current_state as state, edited_at as edited_at, created_at as created_at, merge_sha as merge_sha, contributor_id as author_id
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


    /// Returns the count of PRs that were in the given state on the given day.
    ///
    /// The day boundary is `00:00 UTC` of the given date.
    /// State determination uses the full event and label history:
    ///
    /// - `WaitingForReview` / `WaitingForBors` / `WaitingForAuthor`: counts PRs whose most
    ///   recent `issue_labels_history` row for the matching `S-*` label up to `timestamp` had
    ///   `action = 'ADDED'`, **and** whose most recent "closed"/"merged"/"reopened" event was not
    ///   "closed" or "merged" (i.e. PR was still open on that day).
    ///
    /// - `Merged`: counts all PRs that have a `"merged"` event in `issue_event_history` with
    ///   `timestamp <= T`.  Once merged a PR stays merged, so this is a cumulative count.
    ///
    /// - `Closed`: counts PRs whose most recent `"closed"/"merged"/"reopened"` event up to `T`
    ///   was `"closed"` (closed but not merged, and not later reopened).
    ///
    /// - `Open`: counts PRs where no `"closed"/"merged"` event exists up to `T`, or whose most
    ///   recent such event was `"reopened"`.  The `issues.created_at` column is used to exclude PRs
    ///   not yet created at time `T`.
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
        let ts = timestamp.and_hms_opt(0, 0, 0).unwrap();

        let (count, ): (i64,) = match &state {
            // ── Label-based waiting states ──────────────────────────────────────────────────
            PullRequestStatusRequest::WaitingForReview
            | PullRequestStatusRequest::WaitingForBors
            | PullRequestStatusRequest::WaitingForAuthor => {
                let label = state.to_string();
                sqlx::query_as::<_, (i64,)>(
                    r#"
WITH latest_labels AS (
    -- For each PR, keep only the most recent label event for the target S-* label up to T
    SELECT DISTINCT ON (issue) issue, action
    FROM issue_labels_history
    WHERE repository = $1
      AND is_pr     = true
      AND label     = $3
      AND timestamp <= $2
    ORDER BY issue, timestamp DESC
),
latest_state AS (
    -- For each PR, keep the most recent state-change event (closed / merged / reopened) up to T
    SELECT DISTINCT ON (issue) issue, event
    FROM issue_event_history
    WHERE repository = $1
      AND is_pr    = true
      AND event   IN ('closed', 'merged', 'reopened')
      AND timestamp <= $2
    ORDER BY issue, timestamp DESC
)
SELECT COUNT(*)
FROM latest_labels ll
LEFT JOIN latest_state ls ON ls.issue = ll.issue
WHERE ll.action = 'ADDED'
  -- PR must be open at T (no close/merge event, or most recent was 'reopened')
  AND (ls.issue IS NULL OR ls.event = 'reopened')
                    "#,
                )
                    .bind(repository)
                    .bind(ts)
                    .bind(&label)
                    .fetch_one(&self.pool)
                    .await?
            }

            // ── Merged ──────────────────────────────────────────────────────────────────────
            // A PR is merged once it has a "merged" event; that state is permanent.
            PullRequestStatusRequest::Merged => {
                sqlx::query_as::<_, (i64,)>(
                    r#"
SELECT COUNT(DISTINCT issue)
FROM issue_event_history
WHERE repository = $1
  AND is_pr     = true
  AND event     = 'merged'
  AND timestamp <= $2
                    "#,
                )
                    .bind(repository)
                    .bind(ts)
                    .fetch_one(&self.pool)
                    .await?
            }

            // ── Closed (not merged, not later reopened) ──────────────────────────────────
            PullRequestStatusRequest::Closed => {
                sqlx::query_as::<_, (i64,)>(
                    r#"
WITH latest_state AS (
    SELECT DISTINCT ON (issue) issue, event
    FROM issue_event_history
    WHERE repository = $1
      AND is_pr    = true
      AND event   IN ('closed', 'merged', 'reopened')
      AND timestamp <= $2
    ORDER BY issue, timestamp DESC
)
SELECT COUNT(*)
FROM latest_state
WHERE event = 'closed'
                    "#,
                )
                    .bind(repository)
                    .bind(ts)
                    .fetch_one(&self.pool)
                    .await?
            }

            // ── Open (no close/merge event yet, or most recent was reopened) ─────────────
            PullRequestStatusRequest::Open => {
                sqlx::query_as::<_, (i64,)>(
                    r#"
WITH latest_state AS (
    -- Most recent state-change event per PR up to T
    SELECT DISTINCT ON (issue) issue, event
    FROM issue_event_history
    WHERE repository = $1
      AND is_pr    = true
      AND event   IN ('closed', 'merged', 'reopened')
      AND timestamp <= $2
    ORDER BY issue, timestamp DESC
)
SELECT COUNT(i.issue)
FROM issues i
LEFT JOIN latest_state ls ON ls.issue = i.issue
WHERE i.repository = $1
  AND i.is_pr = true
  -- PR must be open at T
  AND (ls.issue IS NULL OR ls.event = 'reopened')
  -- PR must have existed before T
  AND i.created_at <= $2
                    "#,
                )
                    .bind(repository)
                    .bind(ts)
                    .fetch_one(&self.pool)
                    .await?
            }
        };

        Ok(count)
    }

    /// Returns per-day PR counts in the given state over a date range, using a single SQL query.
    ///
    /// The window is `[anchor_date - last_n_days, anchor_date]` aligned to day boundaries.
    ///
    /// Instead of re-scanning event tables for every day (LATERAL), this computes
    /// **time intervals** when each PR was in the target state (via `LEAD` window functions),
    /// then counts how many intervals overlap each day with a range join.
    /// Each table is scanned once; complexity is O(events + days × active_periods).
    ///
    /// # Errors
    /// Returns an error on SQL/DB failure.
    pub async fn get_pr_count_in_state_over_time(
        &self,
        repository: &str,
        anchor_date: NaiveDate,
        last_n_days: i64,
        state: PullRequestStatusRequest,
    ) -> Result<Vec<(NaiveDate, i64)>> {
        let ts_end = anchor_date.and_hms_opt(0, 0, 0).unwrap();
        let ts_start = ts_end - chrono::Duration::days(last_n_days);

        let rows: Vec<(NaiveDate, i64)> = match &state {
            // ── Label-based waiting states ───────────────────────────────────────
            // A PR is "in state" when the label is ADDED *and* the PR is open.
            // 1. Build open_periods  from creation/reopen → next close/merge  (LEAD)
            // 2. Build label_active  from ADDED → next label event            (LEAD)
            // 3. Intersect the two period sets → in_state_periods
            // 4. Count distinct PRs whose period covers each day
            PullRequestStatusRequest::WaitingForReview
            | PullRequestStatusRequest::WaitingForBors
            | PullRequestStatusRequest::WaitingForAuthor => {
                let label = state.to_string();
                sqlx::query_as::<_, (NaiveDate, i64)>(
                    r#"
WITH
-- Unified timeline of PR open/close transitions
all_transitions AS (
    SELECT issue, created_at AS timestamp, 'created' AS event_type
    FROM issues
    WHERE repository = $1 AND is_pr = true
    UNION ALL
    SELECT issue, timestamp, event AS event_type
    FROM issue_event_history
    WHERE repository = $1 AND is_pr = true
      AND event IN ('closed', 'merged', 'reopened')
),
ordered_transitions AS (
    SELECT issue, timestamp, event_type,
           LEAD(timestamp) OVER (PARTITION BY issue ORDER BY timestamp) AS next_ts
    FROM all_transitions
),
-- Periods when a PR is open: [created|reopened, next close/merge)
open_periods AS (
    SELECT issue,
           timestamp AS start_ts,
           COALESCE(next_ts, '9999-12-31'::timestamp) AS end_ts
    FROM ordered_transitions
    WHERE event_type IN ('created', 'reopened')
),
-- Label ADDED/REMOVED transitions
label_transitions AS (
    SELECT issue, timestamp, action,
           LEAD(timestamp) OVER (PARTITION BY issue ORDER BY timestamp) AS next_ts
    FROM issue_labels_history
    WHERE repository = $1 AND is_pr = true AND label = $4
),
-- Periods when the label is active: [ADDED, next label event)
label_active_periods AS (
    SELECT issue,
           timestamp AS start_ts,
           COALESCE(next_ts, '9999-12-31'::timestamp) AS end_ts
    FROM label_transitions
    WHERE action = 'ADDED'
),
-- Intersection: PR is in target state when BOTH open AND label active
in_state_periods AS (
    SELECT lap.issue,
           GREATEST(lap.start_ts, op.start_ts) AS start_ts,
           LEAST(lap.end_ts, op.end_ts)         AS end_ts
    FROM label_active_periods lap
    JOIN open_periods op
      ON lap.issue = op.issue
     AND lap.start_ts < op.end_ts
     AND lap.end_ts   > op.start_ts
),
date_series AS (
    SELECT d::date AS day
    FROM generate_series($2::timestamp, $3::timestamp, '1 day'::interval) d
)
SELECT ds.day AS date, COUNT(DISTINCT isp.issue) AS count
FROM date_series ds
LEFT JOIN in_state_periods isp
       ON ds.day >= isp.start_ts::date
      AND ds.day <  isp.end_ts::date
GROUP BY ds.day
ORDER BY ds.day
                    "#,
                )
                    .bind(repository)
                    .bind(ts_start)
                    .bind(ts_end)
                    .bind(&label)
                    .fetch_all(&self.pool)
                    .await?
            }

            // ── Merged ──────────────────────────────────────────────────────────
            // Once merged, always merged → cumulative count.
            // 1. Find each PR's first merge date
            // 2. Count merges before the range (base)
            // 3. Running SUM of daily new merges over the date series
            PullRequestStatusRequest::Merged => {
                sqlx::query_as::<_, (NaiveDate, i64)>(
                    r#"
WITH
first_merges AS (
    SELECT issue, MIN(timestamp)::date AS merged_date
    FROM issue_event_history
    WHERE repository = $1 AND is_pr = true AND event = 'merged'
    GROUP BY issue
),
pre_range AS (
    SELECT COUNT(*) AS cnt
    FROM first_merges
    WHERE merged_date < $2::date
),
daily_merges AS (
    SELECT merged_date, COUNT(*) AS cnt
    FROM first_merges
    WHERE merged_date BETWEEN $2::date AND $3::date
    GROUP BY merged_date
),
date_series AS (
    SELECT d::date AS day
    FROM generate_series($2::timestamp, $3::timestamp, '1 day'::interval) d
)
SELECT ds.day AS date,
       ((SELECT cnt FROM pre_range)
         + COALESCE(SUM(dm.cnt) OVER (ORDER BY ds.day), 0))::bigint AS count
FROM date_series ds
LEFT JOIN daily_merges dm ON dm.merged_date = ds.day
ORDER BY ds.day
                    "#,
                )
                    .bind(repository)
                    .bind(ts_start)
                    .bind(ts_end)
                    .fetch_all(&self.pool)
                    .await?
            }

            // ── Closed (not merged, not later reopened) ─────────────────────────
            // A PR is "closed" from a 'closed' event until the next state event.
            // 1. LEAD over state events to build closed periods
            // 2. Count distinct PRs whose closed period covers each day
            PullRequestStatusRequest::Closed => {
                sqlx::query_as::<_, (NaiveDate, i64)>(
                    r#"
WITH
state_transitions AS (
    SELECT issue, timestamp, event,
           LEAD(timestamp) OVER (PARTITION BY issue ORDER BY timestamp) AS next_ts
    FROM issue_event_history
    WHERE repository = $1 AND is_pr = true
      AND event IN ('closed', 'merged', 'reopened')
),
closed_periods AS (
    SELECT issue,
           timestamp AS start_ts,
           COALESCE(next_ts, '9999-12-31'::timestamp) AS end_ts
    FROM state_transitions
    WHERE event = 'closed'
),
date_series AS (
    SELECT d::date AS day
    FROM generate_series($2::timestamp, $3::timestamp, '1 day'::interval) d
)
SELECT ds.day AS date, COUNT(DISTINCT cp.issue) AS count
FROM date_series ds
LEFT JOIN closed_periods cp
       ON ds.day >= cp.start_ts::date
      AND ds.day <  cp.end_ts::date
GROUP BY ds.day
ORDER BY ds.day
                    "#,
                )
                    .bind(repository)
                    .bind(ts_start)
                    .bind(ts_end)
                    .fetch_all(&self.pool)
                    .await?
            }

            // ── Open (created/reopened, not yet closed/merged) ──────────────────
            // 1. Build open periods from creation/reopen → next close/merge (same
            //    CTE pattern as the label-based states)
            // 2. Count distinct PRs whose open period covers each day
            PullRequestStatusRequest::Open => {
                sqlx::query_as::<_, (NaiveDate, i64)>(
                    r#"
WITH
all_transitions AS (
    SELECT issue, created_at AS timestamp, 'created' AS event_type
    FROM issues
    WHERE repository = $1 AND is_pr = true
    UNION ALL
    SELECT issue, timestamp, event AS event_type
    FROM issue_event_history
    WHERE repository = $1 AND is_pr = true
      AND event IN ('closed', 'merged', 'reopened')
),
ordered_transitions AS (
    SELECT issue, timestamp, event_type,
           LEAD(timestamp) OVER (PARTITION BY issue ORDER BY timestamp) AS next_ts
    FROM all_transitions
),
open_periods AS (
    SELECT issue,
           timestamp AS start_ts,
           COALESCE(next_ts, '9999-12-31'::timestamp) AS end_ts
    FROM ordered_transitions
    WHERE event_type IN ('created', 'reopened')
),
date_series AS (
    SELECT d::date AS day
    FROM generate_series($2::timestamp, $3::timestamp, '1 day'::interval) d
)
SELECT ds.day AS date, COUNT(DISTINCT op.issue) AS count
FROM date_series ds
LEFT JOIN open_periods op
       ON ds.day >= op.start_ts::date
      AND ds.day <  op.end_ts::date
GROUP BY ds.day
ORDER BY ds.day
                    "#,
                )
                    .bind(repository)
                    .bind(ts_start)
                    .bind(ts_end)
                    .fetch_all(&self.pool)
                    .await?
            }
        };

        Ok(rows)
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
    /// `anchor_date` (defaults to today) and extends back `last_n_days` days (defaults to 7).
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
        anchor_date: Option<NaiveDate>,
        last_n_days: Option<i64>,
        pagination: Pagination,
    ) -> Result<PaginatedResponse<Contributor>> {
        let timestamp_end = anchor_date
            .unwrap_or(Utc::now().date_naive())
            .and_hms_opt(0, 0, 0)
            .unwrap();
        let timestamp_start = timestamp_end - chrono::Duration::days(last_n_days.unwrap_or(7));
        log::debug!("timestamp_ start {} end {}", timestamp_start, timestamp_end);
        let file_path = format!("{}%", file_path);

        let (limit, offset) = pagination.limit_offset();

        let count = sqlx::query!(
            r#"
SELECT count(distinct contributor_id) as count
FROM file_activity
WHERE file_path like $1
  AND timestamp between $2 and $3
  AND repository = $4
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
SELECT c.github_id, c.github_name, c.name
FROM contributors c
JOIN (
    SELECT DISTINCT contributor_id
    FROM file_activity
    WHERE file_path LIKE $1
      AND timestamp BETWEEN $2 AND $3
      AND repository = $4
    ORDER BY contributor_id
    OFFSET $5 LIMIT $6
) fa ON fa.contributor_id = c.github_id
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
      AND is_pr = true
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
      AND is_pr = true
      AND label IN ('S-waiting-on-review', 'S-waiting-on-bors', 'S-waiting-on-author')
    ORDER BY issue, label, timestamp DESC
)
SELECT
    c.repository AS repository,
    l.issue     AS pr,
    l.label     AS state,
    l.timestamp AS edited_at,
    c.created_at AS created_at,
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
