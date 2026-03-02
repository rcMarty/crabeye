use super::*;

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

        Ok(Self { pool })
    }

    /// Converts a [`rust_team_data::v1::TeamKind`] variant into the corresponding
    /// lowercase string used as the `kind` column value in the `teams` table.
    fn team_kind_to_str(kind: rust_team_data::v1::TeamKind) -> &'static str {
        match kind {
            rust_team_data::v1::TeamKind::Team => "team",
            rust_team_data::v1::TeamKind::WorkingGroup => "working_group",
            rust_team_data::v1::TeamKind::ProjectGroup => "project_group",
            rust_team_data::v1::TeamKind::MarkerTeam => "marker_team",
            rust_team_data::v1::TeamKind::Unknown => "unknown",
        }
    }

    /// Upserts contributors, teams and their many-to-many relations in a single transaction.
    ///
    /// The function deduplicates contributors by `github_id` before inserting, so duplicate
    /// entries in `team_members` are safely collapsed. The `contributors_teams` join table is
    /// completely replaced on every call (DELETE + INSERT) to reflect the current team membership.
    ///
    /// # Errors
    /// Returns an error if any SQL operation or the transaction commit fails.
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

    /// Inserts contributors that do not yet exist, skipping any whose `github_id` is already present.
    ///
    /// Unlike [`upsert_team_members`], this does **not** update existing rows — it is a
    /// conditional insert only. Runs one query per contributor (no batching).
    ///
    /// # Errors
    /// Returns an error if any SQL insert fails.
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

    /// Upserts a single [`PrEvent`] into the `issues` table and, if history is present,
    /// also inserts its event and label history rows in the same transaction.
    ///
    /// The `issues` row is only updated when the incoming `timestamp` is newer than what
    /// is already stored (optimistic upsert). If either `events_history` or `labels_history`
    /// is `None`, history insertion is skipped and a warning is logged.
    ///
    /// # Errors
    /// Returns an error if any SQL operation or the transaction commit fails.
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

    /// Bulk-upserts a slice of [`PrEvent`]s into the `issues` table using a single UNNEST query.
    ///
    /// Each row is only updated when the incoming `timestamp` is newer than the stored one.
    /// After the bulk upsert, if **all** events carry both `events_history` and `labels_history`,
    /// their history rows are inserted via [`insert_issues_history`]. Otherwise history insertion
    /// is silently skipped for the whole batch.
    ///
    /// Returns early (no-op) when the slice is empty.
    ///
    /// # Errors
    /// Returns an error if any SQL operation or the transaction commit fails.
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

    /// Bulk-upserts a slice of [`Issue`]s into the `issues` table using a single UNNEST query.
    ///
    /// Each row is inserted with `is_pr = false`. Existing rows are only updated when the
    /// incoming `timestamp` is newer (optimistic upsert). If **all** events carry both
    /// `events_history` and `labels_history`, history rows are inserted afterwards via
    /// [`insert_issues_history`].
    ///
    /// Returns early (no-op) when the slice is empty.
    ///
    /// # Errors
    /// Returns an error if any SQL operation or the transaction commit fails.
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

    /// Inserts event and label history for a slice of any [`IssueLike`] items.
    ///
    /// If some items are missing `labels_history` or `events_history`, only the items that
    /// have **both** are inserted and a warning is logged. If all items have complete history
    /// the full slice is passed directly to [`insert_issues_history`].
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

    /// Low-level helper that bulk-inserts label and event history rows for a slice of
    /// [`IssueLike`] items into `issue_labels_history` and `issue_event_history` respectively,
    /// using the provided open transaction.
    ///
    /// Both sections are guarded by an `all()` check — if any item is missing the corresponding
    /// history, that section is skipped and a warning is logged. Duplicate rows are ignored
    /// via `ON CONFLICT DO NOTHING`.
    ///
    /// This function does **not** commit the transaction; the caller is responsible for that.
    ///
    /// # Errors
    /// Returns an error if any SQL operation fails.
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
