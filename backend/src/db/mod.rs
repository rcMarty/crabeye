#[allow(unused)]
pub mod model;

// src/db/mod.rs
use crate::api::Pagination;
use crate::db::model::issue::{IssueLabel, IssueEvent, IssueStatus};
use crate::db::model::paginated_response::PaginatedResponse;
use crate::db::model::pr_event::{
    FileActivity, PrEvent, PullRequestStatus, PullRequestStatusRequest,
};
use crate::db::model::responses::TopFilesResponse;
use crate::db::model::team_member::{Contributor, Team};
use crate::db::model::{BackfillRecord, IssueLike};
use anyhow::Result;
use chrono::{NaiveDate, NaiveDateTime, Utc};
use sqlx::migrate::MigrateDatabase;
use sqlx::{PgPool, Pool, Postgres};
use std::collections::HashMap;

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

        let unique_members_map: HashMap<i64, &model::team_member::TeamMember> = team_members
            .iter()
            .map(|m| (m.github_id as i64, m))
            .collect();

        let mut ids = Vec::with_capacity(unique_members_map.len());
        let mut names = Vec::with_capacity(unique_members_map.len());
        let mut github_names = Vec::with_capacity(unique_members_map.len());

        for member in unique_members_map.values() {
            ids.push(member.github_id as i64);
            names.push(member.name.clone());
            github_names.push(member.github_name.clone());
        }

        // Bulk Insert Contributors
        sqlx::query!(
            r#"
INSERT INTO contributors (github_id, github_name, name)
SELECT * FROM UNNEST($1::BIGINT[], $2::TEXT[], $3::TEXT[])
ON CONFLICT (github_id) DO UPDATE SET
    github_name = EXCLUDED.github_name,
    name = EXCLUDED.name
        "#,
            &ids,
            &github_names,
            &names
        )
            .execute(&mut *tx)
            .await?;

        let teams: HashMap<&String, &Team> = team_members
            .iter()
            .flat_map(|tm| tm.teams.iter())
            .map(|t| (&t.team, t)) // Přepíše starší výskyty novějšími
            .collect();
        let mut team_names = Vec::with_capacity(teams.len());
        let mut team_subteams = Vec::with_capacity(teams.len());
        let mut team_kinds = Vec::with_capacity(teams.len());

        for (name, team_data) in teams {
            team_names.push(name);
            team_subteams.push(team_data.subteam_of.clone());
            team_kinds.push(Self::team_kind_to_str(team_data.kind));
        }

        sqlx::query!(
            r#"
INSERT INTO teams (team, subteam_of, kind)
SELECT * FROM UNNEST($1::TEXT[], $2::TEXT[], $3::TEXT[])
ON CONFLICT (team) DO UPDATE SET
subteam_of = EXCLUDED.subteam_of,
kind = EXCLUDED.kind
"#,
            &team_names as &[&String],
            &team_subteams as &[Option<String>],
            &team_kinds as &[&str]
        )
            .execute(&mut *tx)
            .await?;

        sqlx::query!("DELETE FROM contributors_teams")
            .execute(&mut *tx)
            .await?;

        let mut member_ids = vec![];
        let mut member_teams = vec![];
        for member in team_members {
            member_ids.extend(vec![member.github_id as i64; member.teams.len()]);
            member_teams.extend(member.teams.iter().map(|t| t.team.clone()));
        }

        sqlx::query!(
            r#"
INSERT INTO contributors_teams (contributor_id,team)
SELECT * FROM UNNEST($1::BIGINT[], $2::TEXT[])
"#,
            &member_ids,
            &member_teams,
        )
            .execute(&mut *tx)
            .await?;

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
        let mut tx = self.pool.begin().await?;

        sqlx::query!(
            r#"
INSERT INTO issues (repository, issue, is_pr, current_state, timestamp, merge_sha, contributor_id)
VALUES ($1, $2, true, $3, $4, $5, $6)
ON CONFLICT(repository, issue) DO UPDATE SET
    current_state = excluded.current_state,
    timestamp = excluded.timestamp,
    merge_sha = excluded.merge_sha,
    contributor_id = excluded.contributor_id
WHERE issues.timestamp < EXCLUDED.timestamp
"#,
            event.repository,
            event.pr_number,
            event.state.as_str(),
            event.get_timestamp().naive_utc(),
            event.get_merge_sha(),
            event.author_id
        )
            .execute(&mut *tx)
            .await?;

        // %%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%
        // insert to issue_event_history

        // Check if i must insert history
        if event.events_history.is_none() || event.labels_history.is_none() {
            log::warn!("Event for PR #{} is missing states_history or labels_history. Skipping history insertion for this event.", event.pr_number);
            tx.commit().await?;
            return Ok(());
        }

        let states_history = event.events_history.as_ref().unwrap();

        let history_events: Vec<&str> = states_history.iter().map(|s| s.event.as_str()).collect();
        let history_timestamps: Vec<_> = states_history.iter().map(|s| s.timestamp).collect();

        // V SQL použijeme repository a issue jako konstanty ($1, $2) a rozbalíme jen zbytek
        sqlx::query!(
            r#"
INSERT INTO issue_event_history (repository, issue, is_pr, event, timestamp)
SELECT
    $1,        -- repository (konstanta)
    $2,        -- issue (konstanta)
    true,      -- is_pr
    t.event,   -- z UNNEST
    t.timestamp -- z UNNEST
FROM UNNEST($3::TEXT[], $4::TIMESTAMP[])
    as t(event, timestamp)
ON CONFLICT (repository, issue, timestamp, event) DO NOTHING
            "#,
            event.repository,
            event.pr_number,
            &history_events as &[&str],
            &history_timestamps
        )
            .execute(&mut *tx)
            .await?;

        // %%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%
        // 4. Insert do ISSUE_LABEL_HISTORY

        let labels_history = event.labels_history.as_ref().unwrap();

        let history_labels: Vec<&str> = labels_history.iter().map(|l| l.label.as_str()).collect();
        let history_timestamps: Vec<_> = labels_history.iter().map(|l| l.timestamp).collect();
        let history_actions: Vec<&str> = labels_history.iter().map(|l| l.action.as_str()).collect();

        // Opět repository a issue jako konstanty
        sqlx::query!(
            r#"
INSERT INTO issue_labels_history (repository, issue, label, timestamp, action, is_pr)
SELECT
    $1,
    $2,
    t.label,
    t.timestamp,
    t.action,
    true -- is_pr hardcoded
FROM UNNEST($3::TEXT[], $4::TIMESTAMP[], $5::TEXT[])
    as t(label, timestamp, action)
ON CONFLICT (repository, issue, timestamp, label) DO NOTHING
            "#,
            event.repository,
            event.pr_number,
            &history_labels as &[&str],
            &history_timestamps,
            &history_actions as &[&str]
        )
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        Ok(())
    }

    pub async fn insert_pr_events(&self, events: &[PrEvent]) -> Result<()> {
        // let events = Self::latest_pr_events(events);
        if events.is_empty() {
            return Ok(());
        }

        let mut tx = self.pool.begin().await?;

        // Build column vectors directly from iterators for clarity.
        let count = events.len();

        let mut repos: Vec<&str> = Vec::with_capacity(count);
        let mut prs: Vec<i64> = Vec::with_capacity(count);
        let mut states: Vec<&str> = Vec::with_capacity(count);
        let mut timestamps: Vec<chrono::NaiveDateTime> = Vec::with_capacity(count);
        let mut merge_shas: Vec<Option<String>> = Vec::with_capacity(count);
        let mut author_ids: Vec<i64> = Vec::with_capacity(count);

        for event in events {
            repos.push(event.repository.as_str());
            prs.push(event.pr_number);
            states.push(event.state.as_str());
            timestamps.push(event.get_timestamp().naive_utc());
            merge_shas.push(event.get_merge_sha());
            author_ids.push(event.author_id);
        }

        sqlx::query!(
            r#"
INSERT INTO issues (repository, issue, is_pr,current_state, timestamp, merge_sha, contributor_id)
SELECT repository as repository,
       issue as issue,
       true as is_pr,
       current_state as current_state,
       timestamp as timestamp,
       merge_sha as merge_sha,
       contributor_id as contributor_id
    FROM UNNEST($1::TEXT[], $2::BIGINT[] ,$3::TEXT[], $4::TIMESTAMP[], $5::TEXT[], $6::BIGINT[])
         as t(repository, issue,current_state, timestamp, merge_sha, contributor_id)
ON CONFLICT(repository,issue) DO UPDATE SET
current_state = excluded.current_state,
    timestamp = excluded.timestamp,
    merge_sha = excluded.merge_sha,
    contributor_id = excluded.contributor_id
WHERE issues.timestamp < EXCLUDED.timestamp
"#,
            &repos as &[&str],
            &prs,
            &states as &[&str],
            &timestamps,
            &merge_shas as &[Option<String>],
            &author_ids
        )
            .execute(&mut *tx)
            .await?;

        if events.iter().all(|event| event.events_history.is_some())
            && events.iter().all(|event| event.labels_history.is_some())
        {
            self.insert_issues_history(events, &mut tx).await?;
        }
        tx.commit().await?;

        Ok(())
    }

    pub async fn insert_file_activity(&self, activity: &FileActivity) -> Result<()> {
        let user_id = activity.user_id;
        sqlx::query!(
            r#"
INSERT INTO file_activity(repository, issue, file_path, contributor_id, timestamp)
VALUES ($1,$2,$3,$4, $5)
ON CONFLICT(repository, issue, timestamp, file_path) DO NOTHING
               "#,
            activity.repository,
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
        let count = activities.len();

        let mut repositories: Vec<&str> = Vec::with_capacity(count);
        let mut user_ids: Vec<i64> = Vec::with_capacity(count);
        let mut prs: Vec<i64> = Vec::with_capacity(count);
        let mut file_paths: Vec<&str> = Vec::with_capacity(count);
        let mut timestamps: Vec<NaiveDateTime> = Vec::with_capacity(count);

        for activity in activities {
            repositories.push(activity.repository.as_str());
            user_ids.push(activity.user_id);
            prs.push(activity.pr);
            file_paths.push(activity.file_path.as_str());
            timestamps.push(activity.timestamp.naive_utc());
        }

        sqlx::query!(
            r#"
INSERT INTO file_activity(repository, issue, file_path, contributor_id, timestamp)
SELECT * FROM UNNEST($1::TEXT[], $2::BIGINT[], $3::TEXT[], $4::BIGINT[], $5::TIMESTAMP[])
as t(repository, pr, file_path, user_login, timestamp)
ON CONFLICT(repository, issue, timestamp, file_path) DO NOTHING
               "#,
            &repositories as &[&str],
            &prs,
            &file_paths as &[&str],
            &user_ids,
            &timestamps
        )
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn insert_issues(&self, events: &[model::issue::Issue]) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }

        let mut tx: sqlx::Transaction<sqlx::Postgres> = self.pool.begin().await?;
        let count = events.len();

        // Vektory pro sloupce tabulky `issues`
        let mut repos: Vec<&str> = Vec::with_capacity(count);
        let mut issues: Vec<i64> = Vec::with_capacity(count);
        let mut author_ids: Vec<i64> = Vec::with_capacity(count);
        let mut current_states: Vec<&str> = Vec::with_capacity(count);
        let mut timestamps: Vec<chrono::NaiveDateTime> = Vec::with_capacity(count);

        for event in events {
            repos.push(&event.repository);
            issues.push(event.issue_number);
            author_ids.push(event.author_id);
            timestamps.push(event.get_timestamp().naive_utc());
            current_states.push(event.status.as_str());
        }

        sqlx::query!(
            r#"
INSERT INTO issues (repository, issue, contributor_id, current_state, timestamp, is_pr)
SELECT
    t.repo,
    t.issue,
    t.author,
    t.state,
    t.ts,
    false -- is_pr hardcoded
FROM UNNEST(
    $1::TEXT[],
    $2::BIGINT[],
    $3::BIGINT[],
    $4::TEXT[],
    $5::TIMESTAMP[]
) AS t(repo, issue, author, state, ts)
ON CONFLICT (repository, issue) DO UPDATE SET
    current_state = EXCLUDED.current_state,
    timestamp = EXCLUDED.timestamp,
    contributor_id = EXCLUDED.contributor_id
WHERE issues.timestamp < EXCLUDED.timestamp

        "#,
            &repos as &[&str],
            &issues,
            &author_ids,
            &current_states as &[&str],
            &timestamps
        )
            .execute(&mut *tx)
            .await?;

        if events.iter().all(|event| event.events_history.is_some())
            && events.iter().all(|event| event.labels_history.is_some())
        {
            self.insert_issues_history(events, &mut tx).await?;
        }
        tx.commit().await?;

        Ok(())
    }

    pub async fn insert_history<T>(&self, history: &[T]) -> Result<()>
    where
        T: IssueLike,
    {
        let mut tx = self.pool.begin().await?;

        let check = history.iter().all(|issue| issue.labels_history().is_some())
            && history.iter().all(|issue| issue.events_history().is_some());
        if !check {
            let ok_history = history
                .iter()
                .filter(|&issue| {
                    issue.labels_history().is_some() && issue.events_history().is_some()
                })
                .collect::<Vec<_>>();
            log::warn!("Some events are missing labels_history or states_history. Only inserting events with complete history. Total: {}, with complete history: {}", history.len(), ok_history.len());
            self.insert_issues_history(ok_history.as_ref(), &mut tx)
                .await?;
            tx.commit().await?;
            return Ok(());
        }

        self.insert_issues_history(history, &mut tx).await?;
        tx.commit().await?;
        Ok(())
    }

    async fn insert_issues_history<'c, T: IssueLike>(
        &self,
        events: &[T],
        tx: &mut sqlx::Transaction<'c, Postgres>,
    ) -> Result<()> {
        // %%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%
        // section for issue_labels_history
        if events.iter().all(|e| e.labels_history().is_some()) {
            let total_labels: usize = events
                .iter()
                .map(|e| {
                    e.labels_history()
                        .as_ref()
                        .expect("No Labels history for events")
                        .len()
                })
                .sum();
            let mut repos: Vec<&str> = Vec::with_capacity(total_labels);
            let mut issues: Vec<i64> = Vec::with_capacity(total_labels);
            let mut labels: Vec<&str> = Vec::with_capacity(total_labels);
            let mut timestamps: Vec<NaiveDateTime> = Vec::with_capacity(total_labels);
            let mut actions: Vec<&str> = Vec::with_capacity(total_labels);
            let mut is_prs: Vec<bool> = Vec::with_capacity(total_labels);

            for e in events {
                let repo_str = e.repository().as_str();
                let issue_num = e.issue_number();

                for l in e
                    .labels_history()
                    .expect("Labels history is missing for some events")
                {
                    repos.push(repo_str);
                    issues.push(issue_num);
                    labels.push(l.label.as_str());
                    timestamps.push(l.timestamp);
                    actions.push(l.action.as_str());
                    is_prs.push(e.is_pr());
                }
            }

            assert_eq!(
                labels.len(),
                timestamps.len(),
                "Labels, timestamps and actions must have the same length"
            );
            assert_eq!(
                labels.len(),
                actions.len(),
                "Labels, timestamps and actions must have the same length"
            );
            assert_eq!(
                repos.len(),
                labels.len(),
                "Repos, labels, timestamps and actions must have the same length"
            );
            assert_eq!(
                issues.len(),
                labels.len(),
                "Issues, labels, timestamps and actions must have the same length"
            );

            sqlx::query!(
                r#"
INSERT INTO issue_labels_history (repository,issue, label,timestamp, action,is_pr)
SELECT
    t.repository,
    t.issue,
    t.label,
    t.timestamp,
    t.action,
    t.is_pr -- is_pr hardcoded
FROM UNNEST($1::TEXT[], $2::BIGINT[], $3::TEXT[], $4::TIMESTAMP[], $5::TEXT[], $6::BOOLEAN[])
     as t(repository, issue, label, timestamp, action, is_pr)
ON CONFLICT (repository,issue,timestamp, label) DO NOTHING
"#,
                &repos as &[&str],
                &issues,
                &labels as &[&str],
                &timestamps,
                &actions as &[&str],
                &is_prs,
            )
                .execute(&mut **tx)
                .await?;
        } else {
            log::warn!("Some events are missing labels_history. Skipping labels history insertion for these events.");
        }

        // %%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%
        // section for issue_state_history

        if events.iter().all(|e| e.events_history().is_some()) {
            let total_states: usize = events
                .iter()
                .map(|e| {
                    e.events_history()
                        .as_ref()
                        .expect("No States history for events")
                        .len()
                })
                .sum();

            let mut repos: Vec<&str> = Vec::with_capacity(total_states);
            let mut issues: Vec<i64> = Vec::with_capacity(total_states);
            let mut states: Vec<&str> = Vec::with_capacity(total_states);
            let mut timestamps: Vec<NaiveDateTime> = Vec::with_capacity(total_states);
            let mut is_prs: Vec<bool> = Vec::with_capacity(total_states);

            for e in events {
                let repo_str = e.repository().as_str();
                let issue_num = e.issue_number();

                for s in e
                    .events_history()
                    .expect("States history is missing for some events")
                {
                    repos.push(repo_str);
                    issues.push(issue_num);
                    states.push(s.event.as_str());
                    timestamps.push(s.timestamp);
                    is_prs.push(e.is_pr());
                }
            }

            assert_eq!(
                repos.len(),
                issues.len(),
                "Repos, issues, states and timestamps must have the same length"
            );
            assert_eq!(
                repos.len(),
                states.len(),
                "Repos, issues, states and timestamps must have the same length"
            );
            assert_eq!(
                repos.len(),
                timestamps.len(),
                "Repos, issues, states and timestamps must have the same length"
            );

            sqlx::query!(
                r#"
INSERT INTO issue_event_history (repository, issue, event, timestamp, is_pr)
SELECT
    t.repo,
    t.issue,
    t.event,
    t.ts,
    t.is_pr
FROM UNNEST(
    $1::TEXT[],
    $2::BIGINT[],
    $3::TEXT[],
    $4::TIMESTAMP[],
    $5::BOOLEAN[]
) AS t(repo, issue, event, ts, is_pr)
ON CONFLICT (repository, issue, timestamp, event) DO NOTHING
            "#,
                &repos as &[&str],
                &issues,
                &states as &[&str],
                &timestamps,
                &is_prs,
            )
                .execute(&mut **tx)
                .await?;
        } else {
            log::warn!("Some events are missing states_history. Skipping states history insertion for these events.");
        }

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
    pub async fn get_issue_events_at(
        &self,
        repository: &str,
        pr: i64,
        timestamp: NaiveDate,
    ) -> Result<Vec<IssueEvent>> {
        let timestamp_start = timestamp.and_hms_opt(0, 0, 0).unwrap();
        let timestamp_end = timestamp_start + chrono::Duration::days(1);

        let ret = sqlx::query_as::<_, IssueEvent>(
            r#"
SELECT distinct hist.event as state, hist.timestamp as timestamp
FROM issue_event_history hist
WHERE hist.repository = $1 and hist.issue = $2 and hist.timestamp between $3 and $4
ORDER BY timestamp DESC
"#,
        )
            .bind(repository)
            .bind(pr)
            .bind(timestamp_start)
            .bind(timestamp_end)
            .fetch_all(&self.pool)
            .await?;

        log::debug!("return value from get pr state at: \n{:?}", ret);
        Ok(ret)
    }

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
         SELECT DISTINCT ON (issue, label) *
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

        let mut pr = sqlx::query_as::<_, PrEvent>(
            r#"
SELECT repository as repository, issue as pr, current_state as state, timestamp as timestamp, merge_sha as merge_sha, contributor_id as author_id
FROM issues
WHERE repository = $1 and issue = $2 and is_pr = true
"#,
        )
            .bind(repository)
            .bind(pr)
            .fetch_one(&self.pool)
            .await?;

        pr.labels_history = Some(labels);
        pr.events_history = Some(states);

        log::debug!("return value from get pr state at: \n{:?}", pr.clone());
        Ok(Some(pr))
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
        repository: &str,
        timestamp: NaiveDate,
        state: PullRequestStatusRequest,
    ) -> Result<i64> {
        let timestamp_start = timestamp.and_hms_opt(0, 0, 0).unwrap();
        let timestamp_end = timestamp_start + chrono::Duration::days(1);

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

/// part where is querying from database misc functions
impl Database {
    /// Get the timestamp of the last Issue / Pr event in the database
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

    pub async fn get_issues_without_history(&self) -> Result<Vec<BackfillRecord>> {
        let records = sqlx::query_as::<Postgres, BackfillRecord>(
            r#"
    select
        repository as repository,
        issue as issue_number,
        is_pr as is_pr,
        contributor_id as author_id
    from issues
    where (repository, issue) NOT IN (
              SELECT repository, issue
               FROM issue_event_history
           );
    "#,
        )
            .fetch_all(&self.pool)
            .await?;

        Ok(records)
    }

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
