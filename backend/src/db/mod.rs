#[allow(unused)]
pub mod model;

// src/db/mod.rs
use crate::api::Pagination;
use crate::db::model::issue::{IssueLabel, IssueEvent};
use crate::db::model::paginated_response::PaginatedResponse;
use crate::db::model::pr_event::{
    FileActivity, PrEvent, PullRequestStatusRequest,
};
use crate::db::model::responses::TopFilesResponse;
use crate::db::model::team_member::{Contributor, Team};
use crate::db::model::{BackfillRecord, IssueLike};
use anyhow::Result;
use chrono::{NaiveDate, NaiveDateTime, Utc};
use sqlx::migrate::MigrateDatabase;
use sqlx::{PgPool, Pool, Postgres};
use std::collections::HashMap;

pub mod inserts;
pub mod queries;
pub mod misc;

#[derive(Debug, Clone)]
pub struct Database {
    pub pool: Pool<Postgres>,
}
