use crate::api::app_state::AppState;
use crate::api::{Pagination, ReviewParams};
use crate::db::model::paginated_response::PaginatedResponse;
use crate::db::model::team_member::Contributor;
use aide::{
    axum::{
        routing::{get_with, post_with, put_with},
        ApiRouter, IntoApiResponse,
    },
    transform::TransformOperation,
};
use axum::response::IntoResponse;
use axum::{
    debug_handler,
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};

pub fn review_routes(state: AppState) -> ApiRouter {
    ApiRouter::new()
        .api_route(
            "/",
            get_with(made_review, |op| {
                op.description("Get users who made reviews on a specific file within a date range")
                    .tag("Review")
                    .response::<200, Json<PaginatedResponse<Contributor>>>()
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
    let res = app
        .db
        .get_users_who_modified_file(
            params.file,
            params.from_date,
            params.last_n_days,
            Pagination::new(params.page, params.per_page),
        )
        .await;

    match res {
        Ok(reviewers) => (StatusCode::OK, Json(reviewers)).into_response(),
        Err(err) => {
            log::error!("Error getting reviewers: {}", err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(format!("Error getting reviewers: {}", err)),
            )
                .into_response()
        }
    }
}
