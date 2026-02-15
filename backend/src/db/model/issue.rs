use chrono::{DateTime, NaiveDateTime, Utc};
use sqlx::{FromRow, Row};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct Issue {
    pub repository: String,
    pub issue_number: i64,
    pub author_id: i64,
    pub timestamp: NaiveDateTime,
    pub states_history: Vec<IssueState>,
    pub labels_history: Vec<IssueLabel>,
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


impl FromRow<'_, sqlx::postgres::PgRow> for LabelEventAction {
    fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        let action: String = row.try_get("label_event")?;
        match action.as_str() {
            "added" => Ok(LabelEventAction::Added),
            "removed" => Ok(LabelEventAction::Removed),
            _ => Err(sqlx::Error::ColumnDecode {
                index: "label_event".to_string(),
                source: Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Invalid label event action: {}", action),
                )),
            }),
        }
    }
}