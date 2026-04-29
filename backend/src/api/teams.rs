use crate::api::{ApiError, AppState};
use aide::axum::{routing::get_with, ApiRouter, IntoApiResponse};
use axum::{debug_handler, extract::State, http::StatusCode, response::IntoResponse, Json};

pub fn teams_routes(state: AppState) -> ApiRouter {
    ApiRouter::new()
        .api_route(
            "/",
            get_with(list_teams, |op| {
                op.description("List all known teams (for populating frontend selectors)")
                    .tag("Meta")
                    .response::<200, Json<Vec<String>>>()
                    .response::<500, Json<ApiError>>()
            }),
        )
        .with_state(state)
}

#[debug_handler]
async fn list_teams(State(app): State<AppState>) -> impl IntoApiResponse {
    match app.db.get_all_teams().await {
        Ok(teams) => (StatusCode::OK, Json(teams)).into_response(),
        Err(err) => {
            log::error!("Error listing teams: {}", err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(format!("Error listing teams: {}", err))),
            )
                .into_response()
        }
    }
}
