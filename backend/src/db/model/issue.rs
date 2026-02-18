use chrono::{DateTime, NaiveDateTime, Utc};
use sqlx::{FromRow, Row};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct Issue {
    pub repository: String,
    pub issue_number: i64,
    pub author_id: i64,
    pub status: IssueStatus,
    pub states_history: Option<Vec<IssueState>>,
    pub labels_history: Option<Vec<IssueLabel>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub enum IssueStatus {
    Open { time: DateTime<Utc> },
    Closed { time: DateTime<Utc> },
}

impl IssueStatus {
    pub fn from_str(status: &str, timestamp: DateTime<Utc>) -> Option<Self> {
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

impl LabelEventAction {
    pub fn from_str(action: &str) -> Option<Self> {
        match action {
            "ADDED" => Some(LabelEventAction::Added),
            "REMOVED" => Some(LabelEventAction::Removed),
            _ => None,
        }
    }
    pub fn as_str(&self) -> &str {
        match self {
            LabelEventAction::Added => "ADDED",
            LabelEventAction::Removed => "REMOVED",
        }
    }
}

impl FromRow<'_, sqlx::postgres::PgRow> for LabelEventAction {
    fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Self::from_str(row.try_get("label_event")?).ok_or_else(|| {
            sqlx::Error::ColumnDecode {
                index: "label_event".to_string(),
                source: Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Invalid label event action",
                )),
            }
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
