use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
};

pub async fn fallback_handler(req: Request<Body>) -> Response {
    (
        StatusCode::NOT_FOUND,
        format!("404 Not Found: {}", req.uri()),
    )
        .into_response()
}
