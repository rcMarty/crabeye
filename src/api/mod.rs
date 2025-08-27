use schemars::JsonSchema;

pub mod webhooks;
pub mod review;
pub mod app_state;
pub mod docs;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, JsonSchema)]
pub struct ReviewParams {
    file: String,
    last_n_days: Option<i64>,
    from_date: Option<chrono::NaiveDateTime>,
}

#[derive(serde::Deserialize, JsonSchema)]
pub struct Pagination {
    skip: Option<i32>,
    page: Option<i32>,
}
