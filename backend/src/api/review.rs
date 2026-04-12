use crate::api::app_state::AppState;
use crate::api::{
    ApiError, BuilderFileNode, DateCount, FilesModifiedByTeamParams, FilesModifiedResponse,
    GroupingLevel, IssueStateParams, PrCountOverTimeParams, PrCountOverTimeResponse, PrCountParams,
    PrCountResponse, PrTopFilesParams, ReviewParams, WaitingForReviewParams,
};
use crate::db::model::paginated_response::PaginatedResponse;
use crate::db::model::pr_event::PrEvent;
use crate::db::model::responses::TopFilesResponse;
use crate::db::model::team_member::Contributor;
use aide::axum::{routing::get_with, ApiRouter, IntoApiResponse};
use axum::response::IntoResponse;
use axum::{
    debug_handler,
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use indexmap::IndexMap;

pub fn pr_routes(state: AppState) -> ApiRouter {
    ApiRouter::new()
        .api_route(
            "/reviewers",
            get_with(made_review, |op| {
                op.description(
                    "Get users who made reviews on a specific file/path within a date range",
                )
                    .tag("PR")
                    .response::<200, Json<PaginatedResponse<Contributor>>>()
                    .response::<500, Json<ApiError>>()
            }),
        )
        .api_route(
            "/top-n-files",
            get_with(top_n_files, |op| {
                op.description("Get the N most recent file touches by a user within a time window")
                    .tag("PR")
                    .response::<200, Json<Vec<TopFilesResponse>>>()
                    .response::<404, Json<ApiError>>()
                    .response::<500, Json<ApiError>>()
            }),
        )
        .api_route(
            "/prs-in-state",
            get_with(prs_in_state, |op| {
                op.description("Get count of PRs in a specific state at a given timestamp")
                    .tag("PR")
                    .response::<200, Json<PrCountResponse>>()
                    .response::<500, Json<ApiError>>()
            }),
        )
        .api_route(
            "/prs-in-state-over-time",
            get_with(prs_in_state_over_time, |op| {
                op.description("Get count of PRs in a specific state for each day in a lookback window (time-series)")
                    .tag("PR")
                    .response::<200, Json<PrCountOverTimeResponse>>()
                    .response::<500, Json<ApiError>>()
            }),
        )
        .api_route(
            "/pr-history/{issue}",
            get_with(pr_history, |op| {
                op.description("Get the states and labels of a PR at a specific timestamp")
                    .tag("PR")
                    .response::<200, Json<PrEvent>>()
                    .response::<404, Json<ApiError>>()
                    .response::<500, Json<ApiError>>()
            }),
        )
        .api_route(
            "/waiting-for-review",
            get_with(waiting_for_review, |op| {
                op.description("Get PRs that are currently waiting for review")
                    .tag("PR")
                    .response::<200, Json<PaginatedResponse<PrEvent>>>()
                    .response::<500, Json<ApiError>>()
            }),
        )
        .api_route(
            "/files-modified-by-team",
            get_with(files_modified_by_team, |op| {
                op.description("Get files modified by a team within a time window, ordered by modification count descending. The group_level parameter controls grouping: 'none' returns a flat list, a number groups by that folder depth level, 'all' groups by the full folder hierarchy.")
                    .tag("PR")
                    .response::<200, Json<FilesModifiedResponse>>()
                    .response::<404, Json<ApiError>>()
                    .response::<500, Json<ApiError>>()
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

    match app
        .db
        .get_users_who_modified_file(
            params.repository.as_str(),
            params.file,
            params.anchor_date,
            params.last_n_days,
            params.pagination.unwrap_or_default(),
        )
        .await
    {
        Ok(reviewers) => (StatusCode::OK, Json(reviewers)).into_response(),
        Err(err) => {
            log::error!("Error getting reviewers: {}", err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(format!("Error getting reviewers: {}", err))),
            )
                .into_response()
        }
    }
}

#[debug_handler]
async fn top_n_files(
    State(app): State<AppState>,
    Query(params): Query<PrTopFilesParams>,
) -> impl IntoApiResponse {
    let contributors = match app.db.get_user_id_by_name(&params.name).await {
        Ok(Some(contributor)) => {
            log::debug!("Found users {:?}", contributor);
            if contributor.len() > 1 {
                log::warn!(
                    "Multiple users found with name {}, using the first one",
                    params.name
                );
            }
            contributor
        }
        Ok(None) => {
            log::error!("User not found: {}", params.name);
            return (
                StatusCode::NOT_FOUND,
                Json(ApiError::new(format!("User {} not found", params.name))),
            )
                .into_response();
        }
        Err(err) => {
            log::error!("Error getting user ID: {}", err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(format!("Error getting user ID: {}", err))),
            )
                .into_response();
        }
    };

    match app
        .db
        .get_top_n_files(
            params.repository.as_str(),
            contributors,
            chrono::Duration::days(params.last_n_days.unwrap_or(10)),
            params.top_n,
        )
        .await
    {
        Ok(pairs) => (StatusCode::OK, Json(pairs)).into_response(),
        Err(err) => {
            log::error!("Error getting top N files: {}", err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(format!("Error getting top N files: {}", err))),
            )
                .into_response()
        }
    }
}

#[debug_handler]
async fn prs_in_state(
    State(app): State<AppState>,
    Query(params): Query<PrCountParams>,
) -> impl IntoApiResponse {
    let anchor = params.anchor_date.unwrap_or(chrono::Utc::now().date_naive());
    let oldest = app.db.get_oldest_pr_timestamp(params.repository.as_str()).await.unwrap_or(None).map(|dt| dt.date());

    match app
        .db
        .get_pr_count_in_state(
            params.repository.as_str(),
            anchor,
            params.state,
        )
        .await
    {
        Ok(count) => (StatusCode::OK, Json(PrCountResponse { since: oldest, to: anchor, count })).into_response(),
        Err(err) => {
            log::error!("Error getting PR count in state: {}", err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(format!("Error getting PR count in state: {}", err))),
            )
                .into_response()
        }
    }
}

#[debug_handler]
async fn prs_in_state_over_time(
    State(app): State<AppState>,
    Query(params): Query<PrCountOverTimeParams>,
) -> impl IntoApiResponse {
    let anchor = params.anchor_date.unwrap_or(chrono::Utc::now().date_naive());
    let days = params.last_n_days.unwrap_or(30);

    let oldest = app.db.get_oldest_pr_timestamp(params.repository.as_str()).await.unwrap_or(None).map(|dt| dt.date());

    match app
        .db
        .get_pr_count_in_state_over_time(
            params.repository.as_str(),
            anchor,
            days,
            params.state,
        )
        .await
    {
        Ok(rows) => {
            let data: Vec<DateCount> = rows
                .into_iter()
                .map(|(date, count)| DateCount { date, count })
                .collect();
            (StatusCode::OK, Json(PrCountOverTimeResponse { since: oldest, to: anchor, data })).into_response()
        }
        Err(err) => {
            log::error!("Error getting PR count over time: {}", err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(format!("Error getting PR count over time: {}", err))),
            )
                .into_response()
        }
    }
}

#[debug_handler]
async fn pr_history(
    State(app): State<AppState>,
    Path(issue): Path<i64>,
    Query(params): Query<IssueStateParams>,
) -> impl IntoApiResponse {
    match app
        .db
        .get_pr_history_from(params.repository.as_str(), issue, params.timestamp)
        .await
    {
        Ok(Some(counts)) => (StatusCode::OK, Json(counts)).into_response(),
        Ok(None) => {
            log::warn!(
                "History in that timestamp for PR {} not found",
                issue
            );
            (
                StatusCode::NOT_FOUND,
                Json(ApiError::new(format!(
                    "History for timestamp {} for PR {} not found",
                    params.timestamp, issue
                ))),
            )
                .into_response()
        }
        Err(err) => {
            log::error!("Error getting PR history: {}", err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(format!("Error getting PR history: {}", err))),
            )
                .into_response()
        }
    }
}

#[debug_handler]
async fn waiting_for_review(
    State(app): State<AppState>,
    Query(params): Query<WaitingForReviewParams>,
) -> impl IntoApiResponse {
    match app
        .db
        .get_prs_waiting_for_review(
            params.repository.as_str(),
            params.pagination.unwrap_or_default(),
        )
        .await
    {
        Ok(files) => (StatusCode::OK, Json(files)).into_response(),
        Err(err) => {
            log::error!("Error getting most modified files: {}", err);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError::new(err.to_string()))).into_response()
        }
    }
}

/// Retrieves files modified by a specific team within a time window.
///
/// This endpoint first validates that the requested team exists in the database.
/// It then fetches all files modified by team members during the specified time period,
/// ordered by modification count descending.
///
/// The response format depends on the `group_level` parameter:
/// - `NoGrouping` (default): Returns a flat list of (file_path, modification_count) pairs
/// - `GroupBy(depth)`: Returns a hierarchical tree structure grouped by folder levels up to the specified depth
/// - `GroupByAll`: Returns a hierarchical tree structure with the full folder hierarchy
///
/// # Errors
/// - 404: Team not found in database
/// - 500: Database error or internal server error
#[debug_handler]
async fn files_modified_by_team(
    State(app): State<AppState>,
    Query(params): Query<FilesModifiedByTeamParams>,
) -> impl IntoApiResponse {
    let teams = match app.db.get_all_teams().await {
        Ok(teams) => teams,
        Err(err) => {
            log::error!("Error getting teams from database: {}", err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(format!("Error getting teams from database: {}", err))),
            )
                .into_response();
        }
    };
    if !teams.contains(&params.team_name) {
        log::debug!(
            "Team '{}' not found in database teams: {:?}",
            params.team_name,
            teams
        );
        return (
            StatusCode::NOT_FOUND,
            Json(ApiError::new(format!(
                "Team '{}' not found\nDatabase teams: {:?}",
                params.team_name, teams
            ))),
        )
            .into_response();
    }

    match app
        .db
        .get_files_modified_by_team(
            params.repository.as_str(),
            params.team_name.as_str(),
            params.anchor_date,
            params.last_n_days,
        )
        .await
    {
        Ok(files) => {
            let max_level = match params.group_level {
                GroupingLevel::NoGrouping => {
                    let mut sorted_files: IndexMap<String, i64> = files.into_iter().collect();
                    sorted_files.sort_by(|file1, count1, file2, count2| {
                        count2.cmp(count1).then_with(|| file1.cmp(file2))
                    });
                    return (StatusCode::OK, Json(FilesModifiedResponse::List { data: sorted_files })).into_response();
                }
                GroupingLevel::GroupBy(depth_level) => depth_level as usize,
                GroupingLevel::GroupByAll => usize::MAX,
            };

            let mut root = BuilderFileNode::new("/".to_string());

            for (file_path, count) in files {
                root.modifications += count;
                let mut current_node = &mut root;

                for part in file_path
                    .split('/')
                    .filter(|p| !p.is_empty())
                    .take(max_level)
                {
                    current_node = current_node
                        .children
                        .entry(part.to_string())
                        .or_insert_with(|| BuilderFileNode::new(part.to_string()));

                    current_node.modifications += count;
                }
            }

            (StatusCode::OK, Json(FilesModifiedResponse::Tree { data: root.into_response() })).into_response()
        }
        Err(err) => {
            log::error!("Error getting files modified by team: {}", err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(format!("Error getting files modified by team: {}", err))),
            )
                .into_response()
        }
    }
}
