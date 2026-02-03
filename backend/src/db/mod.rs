#[allow(unused)]
pub mod model;

// src/db/mod.rs
use crate::api::{Pagination, PrStateParams};
use crate::db::model::paginated_response::PaginatedResponse;
use crate::db::model::pr_event::{FileActivity, PrEvent, PullRequestStatus, PullRequestStatusRequest};
use crate::db::model::team_member::{Contributor, Team};
use anyhow::Result;
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use sqlx::migrate::MigrateDatabase;
use sqlx::{PgPool, Pool, Postgres};
use std::collections::{HashMap, HashSet};
use crate::db::model::responses::TopFilesResponse;

#[derive(Debug, Clone)]
pub struct Database {
    pub pool: Pool<Postgres>,
}

/// part where is inserting into database
impl Database {
    pub async fn new(database_url: &str) -> Result<Self> {
        // i guess this is useless since sqlx is checking database in compiletime already
        if !Postgres::database_exists(database_url).await? {
            Postgres::create_database(database_url).await?;
        }
        let pool = PgPool::connect(database_url).await?;
        sqlx::migrate!().run(&pool).await?;

        Ok(Self { pool })
    }

    fn team_kind_to_str(kind: rust_team_data::v1::TeamKind) -> &'static str {
        match kind {
            rust_team_data::v1::TeamKind::Team => "team",
            rust_team_data::v1::TeamKind::WorkingGroup => "working_group",
            rust_team_data::v1::TeamKind::ProjectGroup => "project_group",
            rust_team_data::v1::TeamKind::MarkerTeam => "marker_team",
            rust_team_data::v1::TeamKind::Unknown => "unknown",
        }
    }

    /// Upserts team members and upserts teams and their relations
    pub async fn upsert_team_members(
        &self,
        team_members: &[model::team_member::TeamMember],
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        // for some reason when i try to bulk insert with unnest it fails with ON CONFLICT DO UPDATE command cannot affect row a second time
        for member in team_members {
            sqlx::query!(
                r#"
    INSERT INTO contributors (github_id, github_name, name)
    Values ($1, $2, $3)
    ON CONFLICT (github_id) DO UPDATE SET
    github_name = EXCLUDED.github_name,
    name = EXCLUDED.name
    "#,
                member.github_id as i64,
                member.github_name,
                member.name
            )
                .execute(&mut *tx)
                .await?;
        }

        let teams: HashMap<&String, Team> = team_members.iter()
            .flat_map(|tm| tm.teams.iter())
            .fold(HashMap::new(), |mut map, team| {
                map.entry(&team.team).or_insert_with(|| team.clone());
                map
            });
        let subteams: Vec<_> = teams.values().map(|team| team.subteam_of.clone()).collect();
        let kinds: Vec<_> = teams.values().map(|team| Self::team_kind_to_str(team.kind)).collect();

        sqlx::query!(
            r#"
INSERT INTO teams (team, subteam_of, kind)
SELECT * FROM UNNEST($1::TEXT[], $2::TEXT[], $3::TEXT[])
ON CONFLICT (team) DO UPDATE SET
subteam_of = EXCLUDED.subteam_of,
kind = EXCLUDED.kind
"#,
            &teams.keys().cloned().collect::<Vec<&String>>() as &[&String],
            &subteams as &[Option<String>],
            &kinds as &[&str]
        )
            .execute(&mut *tx)
            .await?;

        for member in team_members {
            let member_id: Vec<i64> = vec![member.github_id as i64; member.teams.len()];
            let teams = member.teams.iter().map(|t| t.team.clone()).collect::<Vec<_>>();


            sqlx::query!("DELETE FROM contributors_teams").execute(&mut *tx).await?;

            sqlx::query!(
                r#"
INSERT INTO contributors_teams (github_id,team)
SELECT * FROM UNNEST($1::BIGINT[], $2::TEXT[])
"#,
                &member_id,
                &teams,
            )
                .execute(&mut *tx)
                .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    /// Cleans the old users from table and insert new users
    pub async fn upsert_contributors(
        &self,
        contributors: &[model::team_member::Contributor],
    ) -> Result<()> {
        for user in contributors.iter() {
            sqlx::query!(
                r#"
INSERT INTO contributors (github_id, github_name, name)
select $1,$2,$3
where not exists (select 1 from contributors where github_id = $1);
"#,
                user.github_id as i64,
                user.github_name,
                user.name
            )
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    pub async fn insert_pr_event(&self, event: &PrEvent) -> Result<()> {
        let timestamp = event.get_timestamp().naive_utc();
        let merge_sha = event.get_merge_sha();
        let author_id = event.author_id;

        sqlx::query!(
            r#"
INSERT INTO pull_requests (pr,current_state, timestamp, merge_sha, contributor_id)
VALUES ($1,$2,$3,$4,$5)
ON CONFLICT(pr) DO UPDATE SET
current_state = excluded.current_state,
    timestamp = excluded.timestamp,
    merge_sha = excluded.merge_sha,
    contributor_id = excluded.contributor_id
"#,
            event.pr_number,
            &event.state as &PullRequestStatus,
            timestamp,
            merge_sha,
            author_id
        )
            .execute(&self.pool)
            .await?;

        sqlx::query!(
            r#"
INSERT INTO pr_state_history (pr, state,timestamp, merge_sha)
VALUES ($1,$2,$3,$4)
"#,
            event.pr_number,
            event.state.as_str(),
            timestamp,
            merge_sha
        )
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    fn latest_pr_events(events: &[PrEvent]) -> Vec<PrEvent> {
        let mut map: HashMap<i64, PrEvent> = HashMap::with_capacity(events.len());

        for event in events.iter() {
            map.entry(event.pr_number)
                .and_modify(|existing| {
                    if event.get_timestamp() > existing.get_timestamp() {
                        *existing = event.clone();
                    }
                })
                .or_insert_with(|| event.clone());
        }

        map.into_values().collect()
    }
    pub async fn insert_pr_events(&self, events: &[PrEvent]) -> Result<()> {
        let events = Self::latest_pr_events(events);

        let mut prs = Vec::with_capacity(events.len());
        let mut states = Vec::with_capacity(events.len());
        let mut timestamps = Vec::with_capacity(events.len());
        let mut merge_shas = Vec::with_capacity(events.len());
        let mut author_ids = Vec::with_capacity(events.len());

        for event in events.iter() {
            let (pr, state_str, timestamp, merge_sha, author_id) = event.prepare_for_db();
            prs.push(pr);
            states.push(state_str);
            timestamps.push(timestamp);
            merge_shas.push(merge_sha);
            author_ids.push(author_id);
        }

        sqlx::query!(
            r#"
INSERT INTO pull_requests (pr,current_state, timestamp, merge_sha, contributor_id)
SELECT * FROM UNNEST($1::BIGINT[], $2::TEXT[], $3::TIMESTAMP[], $4::TEXT[], $5::BIGINT[])
         as t(pr, current_state, timestamp, merge_sha, contributor_id)
ON CONFLICT(pr) DO UPDATE SET
current_state = excluded.current_state,
    timestamp = excluded.timestamp,
    merge_sha = excluded.merge_sha,
    contributor_id = excluded.contributor_id
"#,
            &prs,
            &states as &[&str],
            &timestamps,
            &merge_shas as &[Option<String>],
            &author_ids
        )
            .execute(&self.pool)
            .await?;

        sqlx::query!(
            r#"
INSERT INTO pr_state_history (pr, state,timestamp, merge_sha)
SELECT * FROM UNNEST($1::BIGINT[], $2::TEXT[], $3::TIMESTAMP[], $4::TEXT[])
         as t(pr, state, timestamp, merge_sha)
ON CONFLICT (timestamp) DO NOTHING
"#,
            &prs,
            &states as &[&str],
            &timestamps,
            &merge_shas as &[Option<String>]
        )
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn insert_file_activity(&self, activity: &FileActivity) -> Result<()> {
        let user_id = activity.user_id;
        sqlx::query!(
            r#"
INSERT INTO file_activity(pr, file_path, contributor_id, timestamp)
VALUES ($1,$2,$3,$4)
               "#,
            activity.pr,
            activity.file_path,
            user_id,
            activity.timestamp.naive_utc()
        )
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn insert_file_activities(&self, activities: &[FileActivity]) -> Result<()> {
        let user_ids = activities.iter().map(|activity| activity.user_id).collect::<Vec<_>>();
        let prs = activities.iter().map(|activity| activity.pr).collect::<Vec<_>>();
        let file_paths = activities.iter().map(|activity| activity.file_path.as_str()).collect::<Vec<_>>();
        let timestamps = activities.iter().map(|activity| activity.timestamp.naive_utc()).collect::<Vec<_>>();

        sqlx::query!(
            r#"
INSERT INTO file_activity(pr, file_path, contributor_id, timestamp)
SELECT * FROM UNNEST($1::BIGINT[], $2::TEXT[], $3::BIGINT[], $4::TIMESTAMP[])
as t(pr, file_path, user_login, timestamp)
               "#,
            &prs,
            &file_paths as &[&str],
            &user_ids,
            &timestamps
        )
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}


/// Analysis/query helpers for PRs and file activity.
///
/// This impl block contains a set of helper methods that perform read-only
/// analytical queries against the database. The methods typically translate
/// a higher-level question (e.g. "what was the PR state on a given day?")
/// into SQL queries and return parsed results. Notes:
/// - Several functions use date truncation to day-bound queries (00:00..00:00 next day).
/// - Timezone handling: functions usually convert to NaiveDateTime or use UTC.
/// - Pagination is supported where results may be large.
impl Database {
    /// Return the list of PR states recorded for `pr` on the day that contains `timestamp`.
    ///
    /// The function considers the full day that contains `timestamp` (from 00:00 to 00:00 next day)
    /// and returns distinct state values found in `pr_state_history` for that PR ordered by the
    /// recorded timestamp descending. The returned Vec contains state strings in that order.
    ///
    /// Parameters:
    /// - `pr`: pull request number (database `pr` column).
    /// - `timestamp`: a DateTime<Utc>; only the day portion is used to form the search window.
    ///
    /// Returns:
    /// - `Ok(Vec<String>)` with states found for that day (may be empty).
    /// - `Err(...)` on SQL/DB errors.
    //TEMP: Jaký byl stav konkrétního PR v daný timestamp?
    pub async fn get_pr_state_at(
        &self,
        pr: i64,
        timestamp: NaiveDate,
    ) -> Result<Vec<PullRequestStatus>> {
        let timestamp_start = timestamp.and_hms_opt(0, 0, 0).unwrap();
        let timestamp_end = timestamp_start + chrono::Duration::days(1);

        let ret = sqlx::query_as::<_, PullRequestStatus>(
            r#"
SELECT distinct state, timestamp
FROM pr_state_history
WHERE pr = $1 and timestamp between $2 and $3
ORDER BY timestamp DESC
"#,
        )
            .bind(pr)
            .bind(timestamp_start)
            .bind(timestamp_end)
            .fetch_all(&self.pool)
            .await?;

        log::debug!("return value from get pr state at: \n{:?}", ret);
        Ok(ret)
    }


    /// Count PR state occurrences for a given day.
    ///
    /// Returns the number of pr_state_history rows that match `state` between
    /// 00:00 and 00:00 (next day) of the provided `timestamp` (UTC).
    ///
    /// Parameters:
    /// - `timestamp`: DateTime<Utc> used to compute the day window.
    /// - `state`: PullRequestStatus enum; `.as_str()` is used in the query.
    ///
    /// Returns:
    /// - `Ok(i64)` count of matching rows.
    /// - `Err(...)` on SQL/DB errors.
    //TEMP: Jaký byl počet PR v daném stavu (waiting for review, waiting for author, waiting for bors, merged) v daný timestamp/den.
    pub async fn get_pr_count_in_state(
        &self,
        timestamp: NaiveDate,
        state: PullRequestStatusRequest,
    ) -> Result<i64> {
        let timestamp_start = timestamp.and_hms_opt(0, 0, 0).unwrap();
        let timestamp_end = timestamp_start + chrono::Duration::days(1);

        let record = sqlx::query!(
            r#"
SELECT count(*) as count FROM pr_state_history
WHERE timestamp BETWEEN $1 AND $2 AND state = $3;
               "#,
            timestamp_start,
            timestamp_end,
            state.to_string()
        )
            .fetch_one(&self.pool)
            .await?;

        Ok(record.count.unwrap())
    }


    /// Get top N file activities for a user within a duration (simple latest-N query).
    ///
    /// This function fetches up to `n` file activity records for `user_id` where the activity
    /// timestamp is between (now - `duration`) and now. The returned Vec contains tuples
    /// `(file_path, pr)` ordered by activity timestamp descending as returned by the SQL query.
    ///
    /// Notes:
    /// - This is NOT an aggregated "top files by count" query; it returns recent file activities limited to N.
    /// - `duration` should be a chrono::Duration indicating how far back to search.
    ///
    /// Parameters:
    /// - `user_id`: contributor id to filter file_activity.contributor_id.
    /// - `duration`: chrono::Duration window to look back from today (day-aligned).
    /// - `n`: maximum number of rows to return.
    ///
    /// Returns:
    /// - `Ok(Vec<(String, i64)>)` where tuple is (file_path, pr).
    /// - `Err(...)` on SQL/DB errors.
    //TEMP -- 3) Pro daného uživatele/tým (z https://github.com/rust-lang/team), jakých je top N souborů, které byly buď upraveny nebo reviewovány za posledních N časových jednotek?
    pub async fn get_top_n_files(
        &self,
        contributors: Vec<Contributor>,
        duration: chrono::Duration,
        n: i64,
    ) -> Result<Vec<TopFilesResponse>> {
        let timestamp_end = Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap();
        let timestamp_start = timestamp_end - duration;
        let ids = contributors.iter().map(|c| c.github_id as i64).collect::<Vec<i64>>();

        let record = sqlx::query_as::<_, TopFilesResponse>(
            r#"--
select pr as pr_id, file_path, github_id, github_name, name
from file_activity
join contributors c on file_activity.contributor_id = c.github_id
where contributor_id = ANY($1)
  and timestamp between $2 and $3
order by timestamp DESC
LIMIT $4;
"#,
        )
            .bind(&ids)
            .bind(timestamp_start)
            .bind(timestamp_end)
            .bind(n)
            .fetch_all(&self.pool)
            .await?;
        Ok(record)
    }


    /// List users who modified (or reviewed) a file path prefix in a time window, paginated.
    ///
    /// This method treats `file_path` as a prefix pattern: it appends `%` and uses SQL `LIKE`.
    /// The search window is computed from `from_timestamp` (or now if None) minus `last_n_days`
    /// (defaults to 7 days).
    ///
    /// Parameters:
    /// - `file_path`: prefix string (not full SQL pattern); function will search `file_path%`.
    /// - `from_timestamp`: optional NaiveDateTime indicating the end of the window; defaults to now.
    /// - `last_n_days`: optional number of days to look back; defaults to 7.
    /// - `pagination`: Pagination object; controls limit + offset of returned contributors.
    ///
    /// Returns:
    /// - `Ok(PaginatedResponse<Contributor>)` containing the contributor entries and total count.
    /// - `Err(...)` on SQL/DB errors.
    ///
    /// Implementation details:
    /// - Count query computes distinct contributors matching the file pattern and timestamp window.
    /// - Entries query returns distinct (github_id, github_name) for matching contributors and applies pagination.
    //TEMP Pro daný soubor/složku, kteří uživatelé/týmy jej v posledních N dní od určitého datumu upravovali nebo reviewovali?
    //TODO mby add on which pr it was
    pub async fn get_users_who_modified_file(
        &self,
        file_path: String,
        from_timestamp: Option<NaiveDate>,
        last_n_days: Option<i64>,
        pagination: Pagination,
    ) -> Result<PaginatedResponse<Contributor>> {
        let timestamp_end = from_timestamp.unwrap_or(Utc::now().date_naive()).and_hms_opt(0, 0, 0).unwrap();
        let timestamp_start = timestamp_end - chrono::Duration::days(last_n_days.unwrap_or(7));
        log::debug!(
            "timestamp_ start {} end {}",
            timestamp_start.to_string(),
            timestamp_end.to_string()
        );
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
        );
"#,
            file_path,
            timestamp_start,
            timestamp_end
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
        order by contributor_id
        offset $4 limit $5
        );
"#,
        )
            .bind(file_path)
            .bind(timestamp_start)
            .bind(timestamp_end)
            .bind(offset)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;

        Ok(PaginatedResponse::new(count, pagination, entries))
    }


    /// Return PRs that are currently waiting for review (or related waiting states).
    ///
    /// The current implementation queries `pr_state_history` and selects the latest record
    /// per-pr (no separate timestamp filter is used despite the `timestamp` parameter).
    /// It filters states in {'S-waiting-on-review', 'S-waiting-on-bors', 'S-waiting-on-author'}
    /// and returns a Vec of (pr, timestamp_of_latest_state). Results are ordered by the stored timestamp.
    ///
    /// Note: the `timestamp` parameter is presently unused in the SQL and may be removed or used
    /// for additional filtering in the future.
    ///
    /// # Returns:
    /// - `Ok(Vec<(i64, DateTime<Utc>)>)` where the second element is the stored timestamp converted to Utc.
    /// - `Err(...)` on SQL/DB errors.
    //temp -- 5) dotaz: PR, které čekají nejdelší dobu na review (jednodušší verze: jsou nejdelší čas ve stavu "waiting-on-review",
    pub async fn get_prs_waiting_for_review(
        &self,
        pagination: Pagination,
    ) -> Result<PaginatedResponse<PrEvent>> {
        log::debug!("get_prs_waiting_for_review called with pagination: page {}, per_page {}", pagination.page, pagination.per_page);

        let (limit, offset) = pagination.limit_offset();
        let count = sqlx::query!(
            r#"
select count(*) as count from (
    select pr
    from pr_state_history as p
    where NOT EXISTS (SELECT id FROM pr_state_history AS p2 WHERE p.id = p2.id AND p2.timestamp > p.timestamp)
        AND (p.state = 'S-waiting-on-review' OR p.state = 'S-waiting-on-bors' OR p.state = 'S-waiting-on-author')
) as subquery;
"#,
        )
            .fetch_one(&self.pool)
            .await?
            .count
            .unwrap_or(0) as usize;


        // Rust
        let record = sqlx::query_as::<_, PrEvent>(
            r#"
SELECT
    p.pr        AS pr,
    p.state     AS state,
    p.timestamp AS timestamp,
    p.merge_sha AS merge_sha,
    c.github_id AS author_id
FROM pr_state_history AS p
JOIN pull_requests     AS pr_table ON p.pr = pr_table.pr
JOIN contributors      AS c        ON pr_table.contributor_id = c.github_id
WHERE NOT EXISTS (
    SELECT 1 FROM pr_state_history AS p2
    WHERE p2.id <> p.id AND p2.pr = p.pr AND p2.timestamp > p.timestamp
)
  AND p.state IN ('S-waiting-on-review', 'S-waiting-on-bors', 'S-waiting-on-author')
ORDER BY p.timestamp
OFFSET $1
LIMIT $2;
"#,
        )
            .bind(offset)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;

        log::debug!("return value from get_prs_waiting_for_review: \n{:?}", record);

        Ok(PaginatedResponse::new(count, pagination, record))
    }
}

/// part where is querying from database misc functions
impl Database {
    /// Get the timestamp of the last PR event in the database
    pub async fn get_last_pr_event_timestamp(&self) -> Result<Option<NaiveDateTime>> {
        let record = sqlx::query!(
            r#"
SELECT MAX(timestamp) as timestamp FROM pr_state_history
"#,
        )
            .fetch_one(&self.pool)
            .await?;

        Ok(record.timestamp)
    }

    pub async fn get_user_id_by_name(&self, github_name: &String) -> Result<Option<Vec<Contributor>>> {
        let github_name = format!("%{}%", github_name);
        let record: Vec<Contributor> = sqlx::query_as::<_, Contributor>(
            r#"
SELECT name,github_name,github_id FROM contributors
WHERE github_name ilike $1
"#
        )
            .bind(github_name)
            .fetch_all(&self.pool)
            .await?
            ;

        Ok(if record.is_empty() {
            None
        } else {
            Some(record)
        })
    }
}
