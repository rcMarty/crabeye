use crate::api::app_state::AppState;
use crate::api::{Pagination, PrCountParams, PrStateParams, PrTopFilesParams, ReviewParams};
use crate::db::model::paginated_response::PaginatedResponse;
use crate::db::model::team_member::Contributor;
use aide::axum::{
    routing::get_with,
    ApiRouter, IntoApiResponse,
};
use axum::response::IntoResponse;
use axum::{
    debug_handler,
    extract::{Query, State},
    http::StatusCode,
    Json,
};

pub fn pr_routes(state: AppState) -> ApiRouter {
    ApiRouter::new()
        .api_route(
            "/reviewers",
            get_with(made_review, |op| {
                op.description("Get users who made reviews on a specific file within a date range")
                    .tag("PR")
                    .response::<200, Json<PaginatedResponse<Contributor>>>()
                    .response::<500, (StatusCode, String)>()
            }),
        )
        .api_route(
            "/top-n-files",
            get_with(top_n_files, |op| {
                op.description("Get top N files modified by a user within a duration")
                    .tag("PR")
                    .response::<200, Json<Vec<(String, i64)>>>()
                    .response::<500, (StatusCode, String)>()
            }),
        )
        .api_route(
            "/prs-in-state",
            get_with(prs_in_state, |op| {
                op.description("Get count of PRs in a specific state at a given timestamp")
                    .tag("PR")
                    .response::<200, Json<i64>>()
                    .response::<500, (StatusCode, String)>()
            }),
        )
        .api_route(
            "/pr-state",
            get_with(pr_state, |op| {
                op.description("Get the state of a PR at a specific timestamp")
                    .tag("PR")
                    .response::<200, Json<Vec<(String, String)>>>()
                    .response::<500, (StatusCode, String)>()
            }),
        )
        .api_route(
            "/waiting-for-review",
            get_with(waiting_for_review, |op| {
                op.description("Get PRs that are currently waiting for review")
                    .tag("PR")
                    .response::<200, Json<PaginatedResponse<String>>>()
                    .response::<500, (StatusCode, String)>()
            }),
        )
        .with_state(state)
}

#[debug_handler]
async fn made_review(
    State(app): State<AppState>,
    Query(params): Query<ReviewParams>,
) -> impl IntoApiResponse {
    log::debug!("{:?}", params.clone());

    match app.db.get_users_who_modified_file(
        params.file,
        params.from_date,
        params.last_n_days,
        Pagination::new(params.page, params.per_page),
    ).await
    {
        Ok(reviewers) => (StatusCode::OK, Json(reviewers)).into_response(),
        Err(err) => {
            log::error!("Error getting reviewers: {}", err);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(format!("Error getting reviewers: {}", err))).into_response()
        }
    }
}


#[debug_handler]
async fn top_n_files(
    State(app): State<AppState>,
    Query(params): Query<PrTopFilesParams>,
) -> impl IntoApiResponse {
    match app.db.get_top_n_files(
        params.user_id,
        chrono::Duration::days(params.duration.unwrap_or(10)),
        params.top_n,
    ).await
    {
        Ok(pairs) => (StatusCode::OK, Json(pairs)).into_response(),
        Err(err) => {
            log::error!("Error getting PR count: {}", err);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(format!("Error getting top N files {}", err))).into_response()
        }
    }
}

#[debug_handler]
async fn prs_in_state(
    State(app): State<AppState>,
    Query(params): Query<PrCountParams>,
) -> impl IntoApiResponse {
    match app.db.get_pr_count_in_state(
        params.timestamp.unwrap_or(chrono::Utc::now().date_naive()),
        params.state,
    ).await
    {
        Ok(files) => (StatusCode::OK, Json(files)).into_response(),
        Err(err) => {
            log::error!("Error getting top files: {}", err);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(format!("Error getting count pr's in state {}", err))).into_response()
        }
    }
}

#[debug_handler]
async fn pr_state(
    State(app): State<AppState>,
    Query(params): Query<PrStateParams>,
) -> impl IntoApiResponse {
    match app.db.get_pr_state_at(params.pr, params.timestamp).await
    {
        Ok(counts) => (StatusCode::OK, Json(counts)).into_response(),
        Err(err) => {
            log::error!("Error getting file counts: {}", err);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(err.to_string())).into_response()
        }
    }
}

#[debug_handler]
async fn waiting_for_review(
    State(app): State<AppState>,
    Query(limit): Query<Option<Pagination>>,
) -> impl IntoApiResponse {
    match app.db.get_prs_waiting_for_review(limit.unwrap_or_default()).await {
        Ok(files) => (StatusCode::OK, Json(files)).into_response(),
        Err(err) => {
            log::error!("Error getting most modified files: {}", err);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(err.to_string())).into_response()
        }
    }
}