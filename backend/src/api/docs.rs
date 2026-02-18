use crate::api::app_state::AppState;
use aide::openapi::Tag;
use aide::swagger::Swagger;
use aide::transform::TransformOpenApi;
use aide::{
    axum::{
        routing::{get, get_with},
        ApiRouter, IntoApiResponse,
    },
    openapi::OpenApi,
    redoc::Redoc,
    scalar::Scalar,
};
use axum::{response::IntoResponse, Extension, Json};
use std::sync::Arc;

pub fn docs_routes(state: AppState) -> ApiRouter {
    aide::generate::infer_responses(true);

    let router: ApiRouter = ApiRouter::new()
        .api_route_with(
            "/",
            get_with(
                Scalar::new("/docs/private/api.json")
                    .with_title("Ranal Redoc API documentation")
                    .axum_handler(),
                |op| op.description("This documentation page."),
            ),
            |p| p.security_requirement("ApiKey"),
        )
        .api_route_with(
            "/redoc",
            get_with(
                Redoc::new("/docs/private/api.json")
                    .with_title("Ranal Redoc API documentation")
                    .axum_handler(),
                |op| op.description("This documentation page."),
            ),
            |p| p.security_requirement("ApiKey"),
        )
        .api_route_with(
            "/swagger",
            get_with(
                Swagger::new("/docs/private/api.json")
                    .with_title("Ranal Swagger API documentation")
                    .axum_handler(),
                |op| op.description("This documentation page."),
            ),
            |p| p.security_requirement("ApiKey"),
        )
        .route("/private/api.json", get(serve_docs))
        .with_state(state);

    // Afterwards we disable response inference because
    // it might be incorrect for other routes.
    aide::generate::infer_responses(false);

    router
}

pub fn api_docs(api: TransformOpenApi) -> TransformOpenApi {
    api.title("Aide axum Open API for ranal")
        .summary("Rust compiler repository analyzer")
        .description(include_str!("../../readme.md"))
        .tag(Tag {
            name: "todo".into(),
            description: Some("Todo Management".into()),
            ..Default::default()
        })
}

async fn serve_docs(Extension(api): Extension<Arc<OpenApi>>) -> impl IntoApiResponse {
    Json(api).into_response()
}
