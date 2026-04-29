use crate::api::{ApiError, AppState, IssueStateParams};
use crate::db::model::issue::IssueEvent;
use aide::axum::{routing::get_with, ApiRouter, IntoApiResponse};
use axum::{
    debug_handler,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};

pub fn issues_routes(state: AppState) -> ApiRouter {
    ApiRouter::new()
        .api_route(
            "/issue-events/{issue}",
            get_with(issue_events, |op| {
                op.description("Get the state of an Issue at a specific timestamp")
                    .tag("Issue")
                    .response::<200, Json<Vec<IssueEvent>>>()
                    .response::<404, Json<ApiError>>()
                    .response::<500, Json<ApiError>>()
            }),
        )
        .with_state(state)
}

#[debug_handler]
async fn issue_events(
    State(app): State<AppState>,
    Path(issue): Path<i64>,
    Query(params): Query<IssueStateParams>,
) -> impl IntoApiResponse {
    match app
        .db
        .get_issue_events_at(params.repository.as_str(), issue, params.timestamp)
        .await
    {
        Ok(events) => {
            if events.is_empty() {
                log::warn!(
                    "No issue events found for {}#{} at {}",
                    params.repository,
                    issue,
                    params.timestamp
                );
                (
                    StatusCode::NOT_FOUND,
                    Json(ApiError::new(format!(
                        "No issue events found for {}#{} at {}",
                        params.repository, issue, params.timestamp
                    ))),
                )
                    .into_response()
            } else {
                (StatusCode::OK, Json(events)).into_response()
            }
        }
        Err(err) => {
            log::error!("Error getting issue events: {}", err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(err.to_string())),
            )
                .into_response()
        }
    }
}
