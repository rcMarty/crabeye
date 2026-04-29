#[allow(unused)]
pub mod model;

use crate::db::model::issue::{IssueEvent, IssueLabel};
use crate::db::model::paginated_response::PaginatedResponse;
use crate::db::model::pr_event::{FileActivity, PrEvent, PullRequestStatusRequest};
use crate::db::model::responses::TopFilesResponse;
use crate::db::model::team_member::Contributor;
use crate::db::model::{BackfillRecord, IssueLike};
use crate::pagination::Pagination;
use anyhow::Result;
use chrono::{NaiveDate, NaiveDateTime, Utc};
use sqlx::migrate::MigrateDatabase;
use sqlx::{PgPool, Pool, Postgres};
use std::collections::HashMap;

mod inserts;
mod misc;
mod queries;

#[derive(Debug, Clone)]
pub struct Database {
    pool: Pool<Postgres>,
    /// Maximum number of rows sent per bulk INSERT/UPSERT statement.
    /// Read from the `BULK_CHUNK_SIZE` environment variable (defaults to 10 000).
    chunk_size: usize,
}
