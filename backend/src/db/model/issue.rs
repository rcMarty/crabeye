use chrono::{DateTime, NaiveDateTime};
use sqlx::{FromRow, Row};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct Issue {
    pub issue_number: i64,
    pub author_id: i64,
    pub timestamp: NaiveDateTime,
    pub label: String,
    pub action: LabelEventAction,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub enum LabelEventAction {
    Added,
    Removed,
}

impl FromRow<'_, sqlx::postgres::PgRow> for Issue {
    fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(Issue {
            issue_number: row.try_get("issue")?,
            author_id: row.try_get("author_id")?,
            timestamp: row.try_get("timestamp")?,
            label: row.try_get("label")?,
            action: LabelEventAction::from_row(row)?,
        })
    }
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