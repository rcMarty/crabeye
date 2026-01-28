use crate::db::model::pr_event::PullRequestStatusRequest;
use chrono::{NaiveDate, NaiveDateTime};

pub mod app_state;
pub mod docs;
pub mod review;
pub mod webhooks;

/// Common pagination parameters
/// Used in multiple endpoints
/// Defaults to page 1 and 100 items per page
/// If page or per_page is not provided, defaults are used
#[derive(serde::Deserialize, schemars::JsonSchema, Debug, Clone)]
pub struct Pagination {
    pub page: i64,
    pub per_page: i64,
}

impl Default for Pagination {
    fn default() -> Self {
        Pagination {
            page: 1,
            per_page: 100,
        }
    }
}

impl Pagination {
    pub fn new(page: Option<i64>, per_page: Option<i64>) -> Self {
        Pagination {
            page: page.unwrap_or(1).max(1),
            per_page: per_page.unwrap_or(100).clamp(1, 1000),
        }
    }
    /// Returns (limit, offset) tuple for SQL queries
    pub fn limit_offset(&self) -> (i64, i64) {
        let limit = self.per_page;
        let offset = (self.page - 1) * self.per_page;
        (limit.max(0), offset.max(0))
    }
}

/// Parameters for getting reviews for a specific file
#[derive(serde::Deserialize, Debug, Clone, schemars::JsonSchema)]
pub struct ReviewParams {
    /// File path to filter reviews, example = "src/lib.rs", exmple = "src/"
    file: String,
    ///Number of days to look back, default 7, example = 30
    last_n_days: Option<i64>,
    /// Start date (ISO 8601). default Now, example = "2025-01-01"
    from_date: Option<NaiveDate>,
    /// Page number for pagination, default is 1
    page: Option<i64>,
    /// Number of items per page, default is 100, max is 1000
    per_page: Option<i64>,
}

/// Parameters for getting top N files modified by a user
#[derive(serde::Deserialize, Debug, Clone, schemars::JsonSchema)]
pub struct PrTopFilesParams {
    /// User ID to get top files for
    pub user_id: i64,
    /// Number of top files to return
    pub top_n: i64,
    /// Duration in days to look back, default is 10 days
    pub duration: Option<i64>,

}

/// Parameters for getting PR count by status
#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct PrCountParams {
    /// Optional timestamp to filter PRs in that day, default is now, format: YYYY-MM-DD
    pub timestamp: Option<NaiveDate>,
    /// Status of the pull requests to filter by
    pub state: PullRequestStatusRequest,
}

/// Parameters for getting PR state at a specific timestamp
#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct PrStateParams {
    /// Pull request number
    pub pr: i64,
    /// Timestamp to get the PR state at, format: YYYY-MM-DD
    pub timestamp: NaiveDate,
}