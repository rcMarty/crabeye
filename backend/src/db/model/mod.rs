use crate::db::model::issue::{IssueLabel, IssueState};

pub mod issue;
pub mod paginated_response;
pub mod pr_event;
pub mod responses;
pub mod team_member;

pub trait IssueLike {
    fn states_history(&self) -> Option<&Vec<IssueState>>;
    fn labels_history(&self) -> Option<&Vec<IssueLabel>>;
    fn repository(&self) -> &String;
    fn issue_number(&self) -> i64;
    fn author_id(&self) -> i64;
}