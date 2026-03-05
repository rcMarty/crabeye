use crate::api::app_state::AppState;
use crate::api::IssueStateParams;
use crate::db::model::pr_event::PullRequestStatus;
use aide::axum::{routing::get_with, ApiRouter, IntoApiResponse};
use axum::{
    debug_handler,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};

pub fn issues_routes(state: AppState) -> ApiRouter {
    ApiRouter::new()
        .api_route(
            "/issue-events",
            get_with(issue_events, |op| {
                op.description("Get the state of an Issue at a specific timestamp")
                    .tag("Issue")
                    .response::<200, Json<Vec<PullRequestStatus>>>()
                    .response::<404, Json<String>>()
                    .response::<500, Json<String>>()
            }),
        )
        .with_state(state)
}

#[debug_handler]
async fn issue_events(
    State(app): State<AppState>,
    Query(params): Query<IssueStateParams>,
) -> impl IntoApiResponse {
    match app
        .db
        .get_issue_events_at(params.repository.as_str(), params.issue, params.timestamp)
        .await
    {
        Ok(events) => {
            if events.is_empty() {
                log::warn!(
                    "No issue events found for {}#{} at {}",
                    params.repository,
                    params.issue,
                    params.timestamp
                );
                (
                    StatusCode::NOT_FOUND,
                    Json(format!(
                        "No issue events found for {}#{} at {}",
                        params.repository, params.issue, params.timestamp
                    )),
                )
                    .into_response()
            } else {
                (StatusCode::OK, Json(events)).into_response()
            }
        }
        Err(err) => {
            log::error!("Error getting issue events: {}", err);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(err.to_string())).into_response()
        }
    }
}
