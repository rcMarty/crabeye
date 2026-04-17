use crate::db::model::pr_event::PullRequestStatusRequest;
use chrono::NaiveDate;
use serde::Deserialize;
use std::collections::HashMap;
use indexmap::IndexMap;

pub mod app_state;
pub mod docs;
pub mod issues;
pub mod review;
pub mod teams;

/// Unified error response returned by all API endpoints.
#[derive(serde::Serialize, schemars::JsonSchema, Debug, Clone)]
pub struct ApiError {
    pub message: String,
}

impl ApiError {
    pub fn new(message: impl Into<String>) -> Self {
        ApiError { message: message.into() }
    }
}

/// A single data point in a time-series response
#[derive(Debug, Clone, serde::Serialize, schemars::JsonSchema)]
pub struct DateCount {
    pub date: NaiveDate,
    pub count: i64,
}

/// Deserializes an `Option<i64>` from a query-string value (always a string like `"42"`).
///
/// Required because `#[serde(flatten)]` with `serde_urlencoded` (axum `Query`) bypasses
/// the normal string-to-number coercion, passing raw `&str` values to the inner deserializer.
fn deserialize_opt_i64_from_str<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    match opt {
        None => Ok(None),
        Some(s) => s.parse::<i64>().map(Some).map_err(serde::de::Error::custom),
    }
}

/// Flat pagination query parameters — used with `#[serde(flatten)]`.
///
/// Both fields are optional; missing values fall back to `Pagination` defaults
/// (page = 1, per_page = 100).  This form is required because serde_urlencoded
/// (axum `Query`) does **not** support flattening `Option<T>`.
#[derive(serde::Deserialize, schemars::JsonSchema, Debug, Clone, Default)]
pub struct PaginationParams {
    /// Page number (1-based), default 1
    #[serde(default, deserialize_with = "deserialize_opt_i64_from_str")]
    pub page: Option<i64>,
    /// Items per page, default 100, max 1000
    #[serde(default, deserialize_with = "deserialize_opt_i64_from_str")]
    pub per_page: Option<i64>,
}

impl PaginationParams {
    pub fn into_pagination(self) -> Pagination {
        Pagination::new(self.page, self.per_page)
    }
}

/// Optional pagination parameters
/// Used in multiple endpoints
/// If pagination is not provided, defaults are used
/// See Pagination struct for details
#[derive(serde::Deserialize, schemars::JsonSchema, Debug, Clone)]
pub struct WaitingForReviewParams {
    /// Repository identifier to filter reviews, example = "owner/repo"
    pub repository: String,
    #[serde(flatten)]
    pub pagination: PaginationParams,
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

/// Parameters for getting reviews for a specific file
#[derive(serde::Deserialize, Debug, Clone, schemars::JsonSchema)]
pub struct ReviewParams {
    /// Repository identifier to filter reviews, example = "owner/repo"
    pub repository: String,
    /// File path to filter reviews, example = "src/lib.rs", example = "src/"
    pub file: String,
    /// Number of days to look back, default 7, example = 30
    pub last_n_days: Option<i64>,
    /// Anchor date — end of the lookback window (ISO 8601). Defaults to today, example = "2025-01-01"
    pub anchor_date: Option<NaiveDate>,
    #[serde(flatten)]
    pub pagination: PaginationParams,
}

#[derive(serde::Deserialize, Debug, Clone, schemars::JsonSchema)]
pub struct PrTopFilesParams {
    /// Repository identifier to filter reviews, example = "owner/repo"
    pub repository: String,
    /// User name to get top files for
    pub name: String,
    /// Number of top files to return
    pub top_n: i64,
    /// Number of days to look back, default is 10 days
    pub last_n_days: Option<i64>,
}

/// Parameters for getting PR count by status
#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct PrCountParams {
    /// Repository identifier to filter PRs, example = "owner/repo"
    pub repository: String,
    /// Anchor date — end of the window (ISO 8601). Defaults to today, example = "2025-01-01"
    pub anchor_date: Option<NaiveDate>,
    /// Status of the pull requests to filter by
    pub state: PullRequestStatusRequest,
}

/// Parameters for getting PR count by status over a date range (time-series).
/// Returns one count per day within the lookback window.
#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct PrCountOverTimeParams {
    /// Repository identifier to filter PRs, example = "owner/repo"
    pub repository: String,
    /// Status of the pull requests to filter by
    pub state: PullRequestStatusRequest,
    /// Anchor date — end of the lookback window (ISO 8601). Defaults to today, example = "2025-01-01"
    pub anchor_date: Option<NaiveDate>,
    /// Number of days to look back from anchor_date, default 7, max 90, example = 30
    pub last_n_days: Option<i64>,
}

/// Response for a single PR count in a specific state, with the queried time window.
#[derive(serde::Serialize, schemars::JsonSchema, Debug, Clone)]
pub struct PrCountResponse {
    /// oldest issue/pr in database
    pub since: Option<NaiveDate>,
    pub to: NaiveDate,
    pub count: i64,
}

/// Response for PR count over time (time-series), with the queried time window.
#[derive(serde::Serialize, schemars::JsonSchema, Debug, Clone)]
pub struct PrCountOverTimeResponse {
    /// oldest issue/pr in database
    pub since: Option<NaiveDate>,
    pub to: NaiveDate,
    pub data: Vec<DateCount>,
}

/// Parameters for getting PR/issue state at a specific timestamp.
/// The issue/PR number is provided as a path parameter.
#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct IssueStateParams {
    /// Repository identifier to filter reviews, example = "owner/repo"
    pub repository: String,
    /// Timestamp to get the PR state at, format: YYYY-MM-DD
    pub timestamp: NaiveDate,
}

/// Parameters for getting files modified by a team
#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct FilesModifiedByTeamParams {
    /// Repository identifier to filter, example = "owner/repo"
    pub repository: String,
    /// Team name to filter contributors
    pub team_name: String,
    /// Anchor date — end of the lookback window (ISO 8601). Defaults to today, example = "2025-01-01"
    pub anchor_date: Option<NaiveDate>,
    /// Number of days to look back, default 7, example = 30
    pub last_n_days: Option<i64>,
    /// Group results by folder level
    ///
    /// - "none": Return a flat list of files with modification counts (default)
    /// - Number (e.g., 1, 2): Group by folder hierarchy up to that depth level
    /// - "all": Group by the full folder hierarchy
    ///
    /// Examples:
    /// - 1: Groups into top-level folders (src/, library/, etc.)
    /// - 2: Groups into subfolders (src/doc/, library/core/, etc.)
    #[serde(default, deserialize_with = "deserialize_grouping_level")]
    pub group_level: GroupingLevel,
}

#[derive(serde::Deserialize, schemars::JsonSchema, Debug, Clone, Default)]
#[serde(untagged)]
pub enum GroupingLevel {
    #[default]
    #[serde(rename = "none")]
    NoGrouping,
    GroupBy(i64),
    #[serde(rename = "all")]
    GroupByAll,
}

fn deserialize_grouping_level<'de, D>(deserializer: D) -> Result<GroupingLevel, D::Error>
where
    D: serde::Deserializer<'de>,
{
    // helper enum
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum RawValue {
        Str(String),
        Int(i64),
    }

    let raw = RawValue::deserialize(deserializer)?;
    match raw {
        RawValue::Str(s) => match s.as_str() {
            "none" | "" => Ok(GroupingLevel::NoGrouping),
            "all" => Ok(GroupingLevel::GroupByAll),
            _ => s.parse::<i64>().map(GroupingLevel::GroupBy).map_err(|_| {
                serde::de::Error::custom("group_level must be 'none', 'all', or a number")
            }),
        },
        RawValue::Int(n) => Ok(GroupingLevel::GroupBy(n)),
    }
}

#[derive(serde::Serialize, Debug, Clone, schemars::JsonSchema)]
pub struct FileNode {
    pub name: String,
    pub modifications: i64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<FileNode>,
}

pub struct BuilderFileNode {
    pub name: String,
    pub modifications: i64,
    pub children: HashMap<String, BuilderFileNode>,
}

impl BuilderFileNode {
    pub fn new(name: String) -> Self {
        BuilderFileNode {
            name,
            modifications: 0,
            children: HashMap::new(),
        }
    }
    pub fn into_response(self) -> FileNode {
        let mut children: Vec<FileNode> = self
            .children
            .into_values()
            .map(|child| child.into_response())
            .collect();

        // sort children by modifications in descending order
        children.sort_by(|a, b| b.modifications.cmp(&a.modifications));

        FileNode {
            name: self.name,
            modifications: self.modifications,
            children,
        }
    }
}

#[derive(serde::Serialize, schemars::JsonSchema)]
#[serde(tag = "type")]
pub enum FilesModifiedResponse {
    #[serde(rename = "list")]
    List { data: IndexMap<String, i64> },
    #[serde(rename = "tree")]
    Tree { data: FileNode },
}
