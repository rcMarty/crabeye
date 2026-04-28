use crate::db;
use crate::db::model::issue::{IssueEvent, IssueLabel};
use sqlx::Row;

pub mod issue;
pub mod paginated_response;
pub mod pr_event;
pub mod responses;
pub mod team_member;

pub trait IssueLike {
    fn events_history(&self) -> Option<&Vec<IssueEvent>>;
    fn labels_history(&self) -> Option<&Vec<IssueLabel>>;
    fn repository(&self) -> &String;
    fn issue_number(&self) -> i64;
    fn author_id(&self) -> i64;
    fn is_pr(&self) -> bool;

    fn has_events_history(&self) -> bool {
        self.events_history().is_some()
    }

    fn has_labels_history(&self) -> bool {
        self.labels_history().is_some()
    }
}

impl<T: IssueLike> IssueLike for &T {
    fn events_history(&self) -> Option<&Vec<IssueEvent>> {
        (**self).events_history()
    }
    fn labels_history(&self) -> Option<&Vec<IssueLabel>> {
        (**self).labels_history()
    }
    fn repository(&self) -> &String {
        (**self).repository()
    }
    fn issue_number(&self) -> i64 {
        (**self).issue_number()
    }
    fn author_id(&self) -> i64 {
        (**self).author_id()
    }
    fn is_pr(&self) -> bool {
        (**self).is_pr()
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct BackfillRecord {
    pub states_history: Option<Vec<IssueEvent>>,
    pub labels_history: Option<Vec<IssueLabel>>,

    pub repository: String,
    pub issue_number: i64,
    pub author_id: i64,
    pub is_pr: bool,
}

impl IssueLike for BackfillRecord {
    fn events_history(&self) -> Option<&Vec<IssueEvent>> {
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
        self.is_pr
    }
}

impl<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> for BackfillRecord {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        Ok(BackfillRecord {
            states_history: None,
            labels_history: None,
            repository: row.try_get("repository")?,
            issue_number: row.try_get("issue_number")?,
            author_id: row.try_get("author_id")?,
            is_pr: row.try_get("is_pr")?,
        })
    }
}
