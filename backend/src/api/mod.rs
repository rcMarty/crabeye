pub mod app_state;
pub mod docs;
pub mod review;
pub mod webhooks;

#[derive(serde::Deserialize, Debug, Clone, schemars::JsonSchema)]
pub struct ReviewParams {
    file: String,
    last_n_days: Option<i64>,
    from_date: Option<chrono::NaiveDateTime>,
    pagination: Option<Pagination>,
}

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
