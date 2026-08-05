use axum::{
    middleware,
    routing::{get, post, put},
    Router,
};
use std::sync::Arc;

use crate::middleware::{admin_rate_limit_middleware, auth_middleware, csrf_middleware};
use crate::AppState;

pub fn create_routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/sessions/{sessionId}/security-summary",
            get(crate::controllers::get_security_summary),
        )
        .route(
            "/sessions/{sessionId}/flagged",
            get(crate::controllers::get_flagged_submissions),
        )
        .route(
            "/attendance/{attendanceId}/details",
            get(crate::controllers::get_submission_details),
        )
        .route(
            "/attendance/{attendanceId}/review",
            post(crate::controllers::review_submission),
        )
        .route("/settings", get(crate::controllers::get_security_settings))
        .route(
            "/settings",
            put(crate::controllers::update_security_settings),
        )
        .route(
            "/audit-log/verify",
            get(crate::controllers::verify_audit_log),
        )
        // Same ordering/rationale as admin::create_routes: this router is
        // nested under /api/admin/security, so the HttpOnly admin_token
        // cookie (Path=/api/admin) is auto-attached by the browser here too.
        // Without csrf_middleware, PUT /settings and POST .../review were a
        // live CSRF hole for cookie-authenticated admins.
        .layer(middleware::from_fn_with_state(
            state.clone(),
            csrf_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            admin_rate_limit_middleware,
        ))
}
