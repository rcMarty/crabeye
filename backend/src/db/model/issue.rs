use crate::db::model::IssueLike;
use anyhow::anyhow;
use chrono::{DateTime, NaiveDateTime, Utc};
use sqlx::{FromRow, Row};
use std::str::FromStr;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct Issue {
    pub repository: String,
    pub issue_number: i64,
    pub author_id: i64,
    pub status: IssueStatus,
    pub states_history: Option<Vec<IssueState>>,
    pub labels_history: Option<Vec<IssueLabel>>,
}
impl IssueLike for Issue {
    fn states_history(&self) -> Option<&Vec<IssueState>> {
        self.states_history.as_ref()
    }
    fn labels_history(&self) -> Option<&Vec<IssueLabel>> {
        self.labels_history.as_ref()
    }
    fn repository(&self) -> &String {
        &self.repository
    }
    fn issue_number(&self) -> i64 {
        self.issue_number
    }
    fn author_id(&self) -> i64 {
        self.author_id
    }
    fn is_pr(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub enum IssueStatus {
    Open { time: DateTime<Utc> },
    Closed { time: DateTime<Utc> },
}

impl IssueStatus {
    pub fn from_parts(status: &str, timestamp: DateTime<Utc>) -> Option<Self> {
        match status {
            "open" => Some(IssueStatus::Open { time: timestamp }),
            "closed" => Some(IssueStatus::Closed { time: timestamp }),
            _ => None,
        }
    }
    pub fn as_str(&self) -> &str {
        match self {
            IssueStatus::Open { .. } => "open",
            IssueStatus::Closed { .. } => "closed",
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub enum IssueStatusRequest {
    Open,
    Closed,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct IssueLabel {
    pub label: String,
    pub timestamp: NaiveDateTime,
    pub action: LabelEventAction,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct IssueState {
    pub state: String,
    pub timestamp: NaiveDateTime,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LabelEventAction {
    Added,
    Removed,
}

impl FromStr for LabelEventAction {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ADDED" => Ok(LabelEventAction::Added),
            "REMOVED" => Ok(LabelEventAction::Removed),
            _ => Err(anyhow!("Invalid label event action: {}", s)),
        }
    }
}

impl LabelEventAction {
    pub fn as_str(&self) -> &str {
        match self {
            LabelEventAction::Added => "ADDED",
            LabelEventAction::Removed => "REMOVED",
        }
    }
}

impl FromRow<'_, sqlx::postgres::PgRow> for LabelEventAction {
    fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Self::from_str(row.try_get("label_event")?).map_err(|e| sqlx::Error::ColumnDecode {
            index: "label_event".to_string(),
            source: Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid label event action",
            )),
        })
    }
}

impl Issue {
    pub fn get_timestamp(&self) -> DateTime<Utc> {
        match &self.status {
            IssueStatus::Open { time } => *time,
            IssueStatus::Closed { time } => *time,
        }
    }
}
