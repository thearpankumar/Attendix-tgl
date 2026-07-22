use axum::{
    middleware,
    routing::{get, post, put},
    Router,
};
use std::sync::Arc;

use crate::middleware::auth_middleware;
use crate::AppState;

pub fn create_routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/sessions/:sessionId/security-summary",
            get(crate::controllers::get_security_summary),
        )
        .route(
            "/sessions/:sessionId/flagged",
            get(crate::controllers::get_flagged_submissions),
        )
        .route(
            "/attendance/:attendanceId/details",
            get(crate::controllers::get_submission_details),
        )
        .route(
            "/attendance/:attendanceId/review",
            post(crate::controllers::review_submission),
        )
        .route("/settings", get(crate::controllers::get_security_settings))
        .route(
            "/settings",
            put(crate::controllers::update_security_settings),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
}
