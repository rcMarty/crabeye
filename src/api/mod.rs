pub mod webhooks;
pub mod review;
pub mod app_state;
pub mod docs;

#[derive(serde::Deserialize, Debug, Clone, schemars::JsonSchema)]
pub struct ReviewParams {
    file: String,
    last_n_days: Option<i64>,
    from_date: Option<chrono::NaiveDateTime>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct Pagination {
    skip: Option<i32>,
    page: Option<i32>,
}

