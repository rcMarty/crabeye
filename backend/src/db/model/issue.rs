use crate::db::model::IssueLike;
use anyhow::anyhow;
use chrono::format::Numeric::Timestamp;
use chrono::{DateTime, NaiveDateTime, Utc};
use sqlx::{FromRow, Row};
use std::str::FromStr;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct Issue {
    pub repository: String,
    pub issue_number: i64,
    pub author_id: i64,
    pub status: IssueStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub events_history: Option<Vec<IssueEvent>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels_history: Option<Vec<IssueLabel>>,
}
impl IssueLike for Issue {
    fn events_history(&self) -> Option<&Vec<IssueEvent>> {
        self.events_history.as_ref()
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

impl FromRow<'_, sqlx::postgres::PgRow> for IssueLabel {
    fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(IssueLabel {
            label: row.try_get("label")?,
            timestamp: row.try_get("timestamp")?,
            action: LabelEventAction::from_row(row)?,
        })
    }
}

#[derive(
    Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema, sqlx::FromRow,
)]
pub struct IssueEvent {
    pub event: String,
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
                format!("Invalid label event action {:#?}", row),
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
