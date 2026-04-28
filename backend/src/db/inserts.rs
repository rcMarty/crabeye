use super::*;

/// Default chunk size when `BULK_CHUNK_SIZE` env var is missing or invalid.
const DEFAULT_CHUNK_SIZE: usize = 10_000;

/// Insert / upsert operations for all database entities.
impl Database {
    /// Creates a new [`Database`] instance, running migrations on startup.
    ///
    /// Creates the Postgres database if it doesn't exist yet (may be redundant
    /// since sqlx already validates the schema at compile time), then opens a
    /// connection pool and runs all pending sqlx migrations.
    pub async fn new(database_url: &str) -> Result<Self> {
        // i guess this is useless since sqlx is checking database in compiletime already
        if !Postgres::database_exists(database_url).await? {
            Postgres::create_database(database_url).await?;
        }
        let pool = PgPool::connect(database_url).await?;
        sqlx::migrate!().run(&pool).await?;

        let chunk_size = std::env::var("BULK_CHUNK_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_CHUNK_SIZE);

        log::info!("Database bulk chunk size: {}", chunk_size);

        Ok(Self { pool, chunk_size })
    }

    /// Converts a [`rust_team_data::v1::TeamKind`] variant into the corresponding
    /// lowercase string used as the `kind` column value in the `teams` table.
    #[cfg(feature = "git")]
    fn team_kind_to_str(kind: rust_team_data::v1::TeamKind) -> &'static str {
        match kind {
            rust_team_data::v1::TeamKind::Team => "team",
            rust_team_data::v1::TeamKind::WorkingGroup => "working_group",
            rust_team_data::v1::TeamKind::ProjectGroup => "project_group",
            rust_team_data::v1::TeamKind::MarkerTeam => "marker_team",
            rust_team_data::v1::TeamKind::Unknown => "unknown",
        }
    }

    /// Returns only items that carry event history.
    fn collect_events_history<T: IssueLike>(items: &[T]) -> Vec<&T> {
        items
            .iter()
            .filter(|item| item.has_events_history())
            .collect()
    }

    /// Returns only items that carry label history.
    fn collect_labels_history<T: IssueLike>(items: &[T]) -> Vec<&T> {
        items
            .iter()
            .filter(|item| item.has_labels_history())
            .collect()
    }

    /// Deduplicates a slice of [`IssueLike`] items by `(repository, issue_number)`, keeping
    /// the entry with the latest `edited_at` for each key and merging event/label history
    /// from all duplicates so no timeline data is lost.
    ///
    /// This is required because GitHub pagination can return the same item on different pages
    /// (e.g. when sorting by `updated_at` and an item is updated between requests).
    /// Postgres's `INSERT … ON CONFLICT DO UPDATE` cannot touch the same row twice in one
    /// statement, so duplicates must be collapsed beforehand.
    fn dedup_issuelikes<T: IssueLike + Clone>(
        events: &[T],
        key_fn: impl Fn(&T) -> (String, i64),
        edited_at_fn: impl Fn(&T) -> chrono::DateTime<chrono::Utc>,
    ) -> Vec<T> {
        use std::collections::hash_map::Entry;

        let mut map: HashMap<(String, i64), T> = HashMap::with_capacity(events.len());
        for event in events {
            let key = key_fn(event);
            match map.entry(key) {
                Entry::Vacant(v) => {
                    v.insert(event.clone());
                }
                Entry::Occupied(mut o) => {
                    let existing = o.get_mut();
                    if edited_at_fn(event) > edited_at_fn(existing) {
                        let mut merged = event.clone();
                        merged.merge_history_from(existing);
                        *existing = merged;
                    } else {
                        existing.merge_history_from(&event.clone());
                    }
                }
            }
        }
        map.into_values().collect()
    }



    /// Upserts contributors, teams and their many-to-many relations in a single transaction.
    ///
    /// The function deduplicates contributors by `github_id` before inserting, so duplicate
    /// entries in `team_members` are safely collapsed. The `contributors_teams` join table is
    /// completely replaced on every call (DELETE + INSERT) to reflect the current team membership.
    ///
    /// # Errors
    /// Returns an error if any SQL operation or the transaction commit fails.
    #[cfg(feature = "git")]
    pub async fn upsert_team_members(
        &self,
        teams: &[rust_team_data::v1::Team],
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

        // Bulk Insert Contributors (chunked to bound bind-parameter count)
        for chunk_start in (0..ids.len()).step_by(self.chunk_size) {
            let chunk_end = (chunk_start + self.chunk_size).min(ids.len());
            sqlx::query!(
                r#"
INSERT INTO contributors (github_id, github_name, name)
SELECT * FROM UNNEST($1::BIGINT[], $2::TEXT[], $3::TEXT[])
ON CONFLICT (github_id) DO UPDATE SET
    github_name = EXCLUDED.github_name,
    name = EXCLUDED.name
        "#,
                &ids[chunk_start..chunk_end],
                &github_names[chunk_start..chunk_end],
                &names[chunk_start..chunk_end]
            )
                .execute(&mut *tx)
                .await?;
        }

        let mut team_names = Vec::with_capacity(teams.len());
        let mut team_subteams = Vec::with_capacity(teams.len());
        let mut team_kinds = Vec::with_capacity(teams.len());

        for team in teams {
            team_names.push(team.name.clone());
            team_subteams.push(team.subteam_of.clone());
            team_kinds.push(Self::team_kind_to_str(team.kind));
        }

        sqlx::query!(
            r#"
INSERT INTO teams (team, subteam_of, kind)
SELECT * FROM UNNEST($1::TEXT[], $2::TEXT[], $3::TEXT[])
ON CONFLICT (team) DO UPDATE SET
subteam_of = EXCLUDED.subteam_of,
kind = EXCLUDED.kind
"#,
            team_names as Vec<String>,
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

        for chunk_start in (0..member_ids.len()).step_by(self.chunk_size) {
            let chunk_end = (chunk_start + self.chunk_size).min(member_ids.len());
            sqlx::query!(
                r#"
INSERT INTO contributors_teams (contributor_id,team)
SELECT * FROM UNNEST($1::BIGINT[], $2::TEXT[])
"#,
                &member_ids[chunk_start..chunk_end],
                &member_teams[chunk_start..chunk_end],
            )
                .execute(&mut *tx)
                .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    /// Inserts contributors that do not yet exist, skipping any whose `github_id` is already present.
    ///
    /// Unlike [`upsert_team_members`], this does **not** update existing rows — it is a
    /// conditional insert only. Rows are deduplicated by `github_id` and pushed in chunks of
    /// [`CHUNK_SIZE`] using a single bulk UNNEST per chunk.
    ///
    /// # Errors
    /// Returns an error if any SQL insert fails.
    pub async fn upsert_contributors(
        &self,
        contributors: &[model::team_member::Contributor],
    ) -> Result<()> {
        if contributors.is_empty() {
            return Ok(());
        }

        // Deduplicate by github_id so a single chunk doesn't repeat the same key.
        let unique: HashMap<i64, &model::team_member::Contributor> = contributors
            .iter()
            .map(|c| (c.github_id as i64, c))
            .collect();

        let mut ids: Vec<i64> = Vec::with_capacity(unique.len());
        let mut github_names: Vec<&str> = Vec::with_capacity(unique.len());
        let mut names: Vec<Option<&str>> = Vec::with_capacity(unique.len());
        for (id, c) in &unique {
            ids.push(*id);
            github_names.push(c.github_name.as_str());
            names.push(c.name.as_deref());
        }

        for chunk_start in (0..ids.len()).step_by(self.chunk_size) {
            let chunk_end = (chunk_start + self.chunk_size).min(ids.len());
            sqlx::query!(
                r#"
INSERT INTO contributors (github_id, github_name, name)
SELECT * FROM UNNEST($1::BIGINT[], $2::TEXT[], $3::TEXT[])
ON CONFLICT (github_id) DO NOTHING
"#,
                &ids[chunk_start..chunk_end],
                &github_names[chunk_start..chunk_end] as &[&str],
                &names[chunk_start..chunk_end] as &[Option<&str>],
            )
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    /// Upserts a single [`PrEvent`] into the `issues` table and, if any history is present,
    /// also inserts its event and/or label history rows in the same transaction.
    ///
    /// The `issues` row is only updated when the incoming `edited_at` is newer than what
    /// is already stored (optimistic upsert). The `created_at` is set on first insert only.
    /// Event and label history are inserted independently; if either one is missing it is
    /// skipped and a warning is logged.
    ///
    /// # Errors
    /// Returns an error if any SQL operation or the transaction commit fails.
    pub async fn insert_pr_event(&self, event: &PrEvent) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        sqlx::query!(
            r#"
INSERT INTO issues (repository, issue, is_pr, current_state, edited_at, created_at, merge_sha, contributor_id)
VALUES ($1, $2, true, $3, $4, $5, $6, $7)
ON CONFLICT(repository, issue) DO UPDATE SET
    current_state = excluded.current_state,
    edited_at = excluded.edited_at,
    merge_sha = excluded.merge_sha,
    contributor_id = excluded.contributor_id
WHERE issues.edited_at < EXCLUDED.edited_at
"#,
            event.repository,
            event.pr_number,
            event.state.as_str(),
            event.get_edited_at().naive_utc(),
            event.get_created_at().naive_utc(),
            event.get_merge_sha(),
            event.author_id
        )
            .execute(&mut *tx)
            .await?;

        self.insert_issuelike_history(std::slice::from_ref(event), &mut tx)
            .await?;

        tx.commit().await?;

        Ok(())
    }

    /// Bulk-upserts a slice of [`PrEvent`]s into the `issues` table using a single UNNEST query.
    ///
    /// Each row is only updated when the incoming `edited_at` is newer than the stored one.
    /// After the bulk upsert, event and label history are inserted independently for all
    /// events that carry the corresponding history; incomplete events are skipped per section
    /// with a warning.
    ///
    /// Returns early (no-op) when the slice is empty.
    ///
    /// # Errors
    /// Returns an error if any SQL operation or the transaction commit fails.
    pub async fn insert_pr_events(&self, events: &[PrEvent]) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }

        let events = Self::dedup_issuelikes(
            events,
            |e| (e.repository.clone(), e.pr_number),
            |e| e.get_edited_at(),
        );
        let mut tx = self.pool.begin().await?;

        // Build column vectors directly from iterators for clarity.
        let count = events.len();

        let mut repos: Vec<&str> = Vec::with_capacity(count);
        let mut prs: Vec<i64> = Vec::with_capacity(count);
        let mut states: Vec<&str> = Vec::with_capacity(count);
        let mut edited_ats: Vec<chrono::NaiveDateTime> = Vec::with_capacity(count);
        let mut created_ats: Vec<chrono::NaiveDateTime> = Vec::with_capacity(count);
        let mut merge_shas: Vec<Option<String>> = Vec::with_capacity(count);
        let mut author_ids: Vec<i64> = Vec::with_capacity(count);

        for event in &events {
            repos.push(event.repository.as_str());
            prs.push(event.pr_number);
            states.push(event.state.as_str());
            edited_ats.push(event.get_edited_at().naive_utc());
            created_ats.push(event.get_created_at().naive_utc());
            merge_shas.push(event.get_merge_sha());
            author_ids.push(event.author_id);
        }

        for chunk_start in (0..count).step_by(self.chunk_size) {
            let chunk_end = (chunk_start + self.chunk_size).min(count);
            sqlx::query!(
                r#"
INSERT INTO issues (repository, issue, is_pr, current_state, edited_at, created_at, merge_sha, contributor_id)
SELECT repository as repository,
       issue as issue,
       true as is_pr,
       current_state as current_state,
       edited_at as edited_at,
       created_at as created_at,
       merge_sha as merge_sha,
       contributor_id as contributor_id
    FROM UNNEST($1::TEXT[], $2::BIGINT[] ,$3::TEXT[], $4::TIMESTAMP[], $5::TIMESTAMP[], $6::TEXT[], $7::BIGINT[])
         as t(repository, issue, current_state, edited_at, created_at, merge_sha, contributor_id)
ON CONFLICT(repository,issue) DO UPDATE SET
current_state = excluded.current_state,
    edited_at = excluded.edited_at,
    merge_sha = excluded.merge_sha,
    contributor_id = excluded.contributor_id
WHERE issues.edited_at < EXCLUDED.edited_at
"#,
                &repos[chunk_start..chunk_end] as &[&str],
                &prs[chunk_start..chunk_end],
                &states[chunk_start..chunk_end] as &[&str],
                &edited_ats[chunk_start..chunk_end],
                &created_ats[chunk_start..chunk_end],
                &merge_shas[chunk_start..chunk_end] as &[Option<String>],
                &author_ids[chunk_start..chunk_end]
            )
                .execute(&mut *tx)
                .await?;
        }

        self.insert_issuelike_history(&events, &mut tx).await?;
        tx.commit().await?;

        Ok(())
    }

    /// Inserts a single [`FileActivity`] record into the `file_activity` table.
    ///
    /// Duplicate rows (same repository, issue, timestamp and file_path) are silently ignored
    /// via `ON CONFLICT DO NOTHING`.
    ///
    /// # Errors
    /// Returns an error if the SQL insert fails.
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

    /// Bulk-inserts a slice of [`FileActivity`] records using a single UNNEST query.
    ///
    /// Duplicate rows (same repository, issue, timestamp and file_path) are silently ignored
    /// via `ON CONFLICT DO NOTHING`.
    ///
    /// # Errors
    /// Returns an error if the SQL insert fails.
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

        for chunk_start in (0..count).step_by(self.chunk_size) {
            let chunk_end = (chunk_start + self.chunk_size).min(count);
            sqlx::query!(
                r#"
INSERT INTO file_activity(repository, issue, file_path, contributor_id, timestamp)
SELECT * FROM UNNEST($1::TEXT[], $2::BIGINT[], $3::TEXT[], $4::BIGINT[], $5::TIMESTAMP[])
as t(repository, pr, file_path, user_login, timestamp)
ON CONFLICT(repository, issue, timestamp, file_path) DO NOTHING
               "#,
                &repositories[chunk_start..chunk_end] as &[&str],
                &prs[chunk_start..chunk_end],
                &file_paths[chunk_start..chunk_end] as &[&str],
                &user_ids[chunk_start..chunk_end],
                &timestamps[chunk_start..chunk_end]
            )
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    /// Bulk-upserts a slice of [`Issue`]s into the `issues` table using a single UNNEST query.
    ///
    /// Each row is inserted with `is_pr = false`. Existing rows are only updated when the
    /// incoming `edited_at` is newer (optimistic upsert). The `created_at` is set on first
    /// insert only. Event and label history are inserted afterwards independently for issues
    /// that carry the corresponding history; incomplete issues are skipped per section with a
    /// warning.
    ///
    /// Returns early (no-op) when the slice is empty.
    ///
    /// # Errors
    /// Returns an error if any SQL operation or the transaction commit fails.
    pub async fn insert_issues(&self, events: &[model::issue::Issue]) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }

        let events = Self::dedup_issuelikes(
            events,
            |e| (e.repository.clone(), e.issue_number),
            |e| e.get_edited_at(),
        );
        let mut tx: sqlx::Transaction<sqlx::Postgres> = self.pool.begin().await?;
        let count = events.len();

        // Vektory pro sloupce tabulky `issues`
        let mut repos: Vec<&str> = Vec::with_capacity(count);
        let mut issues: Vec<i64> = Vec::with_capacity(count);
        let mut author_ids: Vec<i64> = Vec::with_capacity(count);
        let mut current_states: Vec<&str> = Vec::with_capacity(count);
        let mut edited_ats: Vec<chrono::NaiveDateTime> = Vec::with_capacity(count);
        let mut created_ats: Vec<chrono::NaiveDateTime> = Vec::with_capacity(count);

        for event in &events {
            repos.push(&event.repository);
            issues.push(event.issue_number);
            author_ids.push(event.author_id);
            edited_ats.push(event.get_edited_at().naive_utc());
            created_ats.push(event.get_created_at().naive_utc());
            current_states.push(event.status.as_str());
        }

        for chunk_start in (0..count).step_by(self.chunk_size) {
            let chunk_end = (chunk_start + self.chunk_size).min(count);
            sqlx::query!(
                r#"
INSERT INTO issues (repository, issue, contributor_id, current_state, edited_at, created_at, is_pr)
SELECT
    t.repo,
    t.issue,
    t.author,
    t.state,
    t.edited_at,
    t.created_at,
    false -- is_pr hardcoded
FROM UNNEST(
    $1::TEXT[],
    $2::BIGINT[],
    $3::BIGINT[],
    $4::TEXT[],
    $5::TIMESTAMP[],
    $6::TIMESTAMP[]
) AS t(repo, issue, author, state, edited_at, created_at)
ON CONFLICT (repository, issue) DO UPDATE SET
    current_state = EXCLUDED.current_state,
    edited_at = EXCLUDED.edited_at,
    contributor_id = EXCLUDED.contributor_id
WHERE issues.edited_at < EXCLUDED.edited_at

        "#,
                &repos[chunk_start..chunk_end] as &[&str],
                &issues[chunk_start..chunk_end],
                &author_ids[chunk_start..chunk_end],
                &current_states[chunk_start..chunk_end] as &[&str],
                &edited_ats[chunk_start..chunk_end],
                &created_ats[chunk_start..chunk_end]
            )
                .execute(&mut *tx)
                .await?;
        }

        self.insert_issuelike_history(&events, &mut tx).await?;
        tx.commit().await?;

        Ok(())
    }

    /// Inserts event and label history for a slice of any [`IssueLike`] items.
    ///
    /// If some items are missing `labels_history` or `events_history`, only the items that
    /// have the corresponding history are inserted into that specific history table and a
    /// warning is logged.
    ///
    /// The operation runs inside a single transaction that is committed on success.
    ///
    /// # Errors
    /// Returns an error if any SQL operation or the transaction commit fails.
    pub async fn insert_history<T>(&self, history: &[T]) -> Result<()>
    where
        T: IssueLike,
    {
        let mut tx = self.pool.begin().await?;

        self.insert_issuelike_history(history, &mut tx).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Low-level helper that bulk-inserts label and event history rows for a slice of
    /// [`IssueLike`] items into `issue_labels_history` and `issue_event_history` respectively,
    /// using the provided open transaction.
    ///
    /// Event and label history are handled independently: each section filters the incoming
    /// slice to items that provide the corresponding history. Duplicate rows are ignored via
    /// `ON CONFLICT DO NOTHING`.
    ///
    /// This function does **not** commit the transaction; the caller is responsible for that.
    ///
    /// # Errors
    /// Returns an error if any SQL operation fails.
    async fn insert_issuelike_history<'c, T: IssueLike>(
        &self,
        events: &[T],
        tx: &mut sqlx::Transaction<'c, Postgres>,
    ) -> Result<()> {
        // %%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%
        // section for issue_labels_history
        let labels_events = Self::collect_labels_history(events);
        if labels_events.len() != events.len() {
            log::warn!(
                "Some events are missing labels_history. Only inserting labels history for events where it is present. Total: {}, with labels history: {}",
                events.len(),
                labels_events.len()
            );
        }
        if !labels_events.is_empty() {
            let total_labels: usize = labels_events
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

            for e in labels_events {
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

            let total = repos.len();
            for chunk_start in (0..total).step_by(self.chunk_size) {
                let chunk_end = (chunk_start + self.chunk_size).min(total);
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
                    &repos[chunk_start..chunk_end] as &[&str],
                    &issues[chunk_start..chunk_end],
                    &labels[chunk_start..chunk_end] as &[&str],
                    &timestamps[chunk_start..chunk_end],
                    &actions[chunk_start..chunk_end] as &[&str],
                    &is_prs[chunk_start..chunk_end],
                )
                    .execute(&mut **tx)
                    .await?;
            }
        }

        // %%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%
        // section for issue_state_history

        let state_events = Self::collect_events_history(events);
        if state_events.len() != events.len() {
            log::warn!(
                "Some events are missing states_history. Only inserting states history for events where it is present. Total: {}, with states history: {}",
                events.len(),
                state_events.len()
            );
        }
        if !state_events.is_empty() {
            let total_states: usize = state_events
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

            for e in state_events {
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

            let total = repos.len();
            for chunk_start in (0..total).step_by(self.chunk_size) {
                let chunk_end = (chunk_start + self.chunk_size).min(total);
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
                    &repos[chunk_start..chunk_end] as &[&str],
                    &issues[chunk_start..chunk_end],
                    &states[chunk_start..chunk_end] as &[&str],
                    &timestamps[chunk_start..chunk_end],
                    &is_prs[chunk_start..chunk_end],
                )
                    .execute(&mut **tx)
                    .await?;
            }
        }

        Ok(())
    }
}
