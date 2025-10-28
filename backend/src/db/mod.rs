#[allow(unused)]
pub mod model;

// src/db/mod.rs
use crate::api::Pagination;
use crate::db::model::paginated_response::PaginatedResponse;
use crate::db::model::pr_event::{FileActivity, PrEvent, PullRequestStatus};
use anyhow::Result;
use chrono::{NaiveDateTime, DateTime, Utc};
use sqlx::migrate::MigrateDatabase;
use sqlx::{PgPool, Pool, Postgres, QueryBuilder};

#[derive(Debug, Clone)]
pub struct Database {
    pub pool: Pool<Postgres>,
}

// part where is inserting into database
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

    /// Cleans the old users from table and insert new users
    pub async fn upsert_team_members(
        &self,
        team_members: &[model::team_member::TeamMember],
    ) -> Result<()> {
        let github_ids: Vec<i64> = team_members.iter().map(|user| user.github_id as i64).collect();
        let github_names: Vec<&str> = team_members.iter().map(|user| user.github_name.as_str()).collect();
        let names: Vec<&str> = team_members.iter().map(|user| user.name.as_str()).collect();
        let teams: Vec<&str> = team_members.iter().map(|user| user.team.as_str()).collect();
        let subteams: Vec<Option<&str>> = team_members.iter().map(|user| user.subteam_of.as_deref()).collect();
        let kinds: Vec<&str> = team_members.iter().map(|user| Self::team_kind_to_str(user.kind)).collect();

        sqlx::query!(
            r#"
INSERT INTO team_members (github_id, github_name, name, team, subteam_of, kind)
SELECT * FROM UNNEST($1::BIGINT[], $2::TEXT[], $3::TEXT[], $4::TEXT[], $5::TEXT[], $6::TEXT[])
            as t(github_id, github_name, name, team, subteam_of, kind)
ON CONFLICT (github_id) DO UPDATE SET
    github_name = excluded.github_name,
    name = excluded.name,
    team = excluded.team,
    subteam_of = excluded.subteam_of,
    kind = excluded.kind
"#,
            &github_ids[..],
            &github_names[..] as &[&str],
            &names[..] as &[&str],
            &teams[..] as &[&str],
            &subteams[..] as &[Option<&str>],
            &kinds[..] as &[&str]
        )
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Cleans the old users from table and insert new users
    pub async fn upsert_contributors(
        &self,
        contributors: &[model::team_member::Contributor],
    ) -> Result<()> {
        for user in contributors.iter() {
            let gh_id = user.github_id as i64;
            let kind = Self::team_kind_to_str(rust_team_data::v1::TeamKind::Unknown); // Contributors are not part of any team, so we use Unknown kind
            sqlx::query!(
                r#"
INSERT INTO team_members (github_id, github_name, name,team, subteam_of, kind)
select $1,$2,$3,$4,$5,$6
where not exists (select 1 from team_members where github_id = $1);
"#,
                gh_id,
                user.github_name,
                "",
                "",
                "",
                kind
            )
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    pub async fn insert_pr_event(&self, event: &PrEvent) -> Result<()> {
        let timestamp = event.get_timestamp().naive_utc();
        let merge_sha = event.get_merge_sha();
        let author_id = event.author_id.0 as i64;

        sqlx::query!(
            r#"
INSERT INTO pull_requests (pr,current_state, timestamp, merge_sha, author_id)
VALUES ($1,$2,$3,$4,$5)
ON CONFLICT(pr) DO UPDATE SET
current_state = excluded.current_state,
    timestamp = excluded.timestamp,
    merge_sha = excluded.merge_sha,
    author_id = excluded.author_id
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

    pub async fn insert_pr_events(&self, events: &[PrEvent]) -> Result<()> {
        let prs = events.iter().map(|event| event.pr_number).collect::<Vec<_>>();
        let states = events.iter().map(|event| event.state.as_str()).collect::<Vec<_>>();
        let timestamps = events.iter().map(|event| event.get_timestamp().naive_utc()).collect::<Vec<_>>();
        let merge_shas = events.iter().map(|event| event.get_merge_sha()).collect::<Vec<_>>();
        let author_ids = events.iter().map(|event| event.author_id.0 as i64).collect::<Vec<_>>();

        sqlx::query!(
            r#"
INSERT INTO pull_requests (pr,current_state, timestamp, merge_sha, author_id)
SELECT * FROM UNNEST($1::BIGINT[], $2::TEXT[], $3::TIMESTAMP[], $4::TEXT[], $5::BIGINT[])
         as t(pr, current_state, timestamp, merge_sha, author_id)
ON CONFLICT(pr) DO UPDATE SET
current_state = excluded.current_state,
    timestamp = excluded.timestamp,
    merge_sha = excluded.merge_sha,
    author_id = excluded.author_id
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
        let user_id = activity.user_id.0 as i64;
        sqlx::query!(
            r#"
INSERT INTO file_activity(pr, file_path, user_login, timestamp)
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
        let user_ids = activities.iter().map(|activity| activity.user_id.0 as i64).collect::<Vec<_>>();
        let prs = activities.iter().map(|activity| activity.pr).collect::<Vec<_>>();
        let file_paths = activities.iter().map(|activity| activity.file_path.as_str()).collect::<Vec<_>>();
        let timestamps = activities.iter().map(|activity| activity.timestamp.naive_utc()).collect::<Vec<_>>();

        sqlx::query!(
            r#"
INSERT INTO file_activity(pr, file_path, user_login, timestamp)
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
        let timestamp_start = timestamp.date_naive().and_hms_opt(0, 0, 0).unwrap();
        let timestamp_end = timestamp_start + chrono::Duration::days(1);

        let record = sqlx::query!(
            r#"
SELECT distinct state, timestamp
FROM pr_state_history
WHERE pr = $1 and timestamp between $2 and $3
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
        timestamp: chrono::DateTime<Utc>,
        state: PullRequestStatus,
    ) -> Result<i64> {
        let timestamp_start = timestamp.date_naive().and_hms_opt(0, 0, 0).unwrap();
        let timestamp_end = timestamp_start + chrono::Duration::days(1);

        let record = sqlx::query!(
            r#"
SELECT count(*) as count FROM pr_state_history
WHERE timestamp BETWEEN $1 AND $2 AND state = $3;
               "#,
            timestamp_start,
            timestamp_end,
            state.as_str()
        )
            .fetch_one(&self.pool)
            .await?;

        Ok(record.count.unwrap())
    }

    /**
    TEMP -- 3) Pro daného uživatele/tým (z https://github.com/rust-lang/team), jakých je top N souborů, které byly buď upraveny nebo reviewovány za posledních N časových jednotek?
    */
    pub async fn get_top_n_files(
        &self,
        user_id: i64,
        duration: chrono::Duration,
        n: i64,
    ) -> Result<Vec<(String, i64)>> {
        let timestamp_end = Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap();
        let timestamp_start = timestamp_end - duration;

        let record = sqlx::query!(
            r#"
select pr, file_path
from file_activity
where user_login = $1
  and timestamp between $2 and $3
order by timestamp DESC
LIMIT $4;
"#,
            user_id,
            timestamp_start,
            timestamp_end,
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
    TEMP Pro daný soubor/složku, kteří uživatelé/týmy jej v posledních N dní od určitého datumu upravovali nebo reviewovali?
    TODO mby add on which pr it was
    */
    pub async fn get_users_who_modified_file(
        &self,
        file_path: String,
        from_timestamp: Option<NaiveDateTime>,
        last_n_days: Option<i64>,
        pagination: Pagination,
    ) -> Result<PaginatedResponse<model::team_member::Contributor>> {
        let timestamp_end = from_timestamp.unwrap_or(Utc::now().naive_utc());
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
from team_members
where github_id in
(
        select distinct user_login
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

        let entries = sqlx::query_as::<_, model::team_member::Contributor>(
            r#"
select distinct github_id, github_name
from team_members
where github_id in
      (
        select distinct user_login
        from file_activity
        where file_path like $1
          and timestamp between $2 and $3
        order by user_login
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

    /**
    temp -- 5) dotaz: PR, které čekají nejdelší dobu na review (jednodušší verze: jsou nejdelší čas ve stavu "waiting-on-review",
    */
    pub async fn get_prs_waiting_for_review(
        &self,
        timestamp: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<(i64, chrono::DateTime<chrono::Utc>)>> {
        let record = sqlx::query!(
            r#"
select pr, timestamp
from pr_state_history as p
where NOT EXISTS (SELECT id FROM pr_state_history AS p2 WHERE p.id = p2.id AND p2.timestamp > p.timestamp)
  AND (p.state = 'S-waiting-on-review' OR p.state = 'S-waiting-on-bors' OR p.state = 'S-waiting-on-author')
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
}