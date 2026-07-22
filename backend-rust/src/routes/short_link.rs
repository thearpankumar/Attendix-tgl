use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tower::ServiceBuilder;

use crate::middleware::mobile_check_middleware;
use crate::AppState;

pub fn create_routes() -> Router<Arc<AppState>> {
    Router::new()
        // Public routes that require mobile device check
        .route("/:shortCode", get(crate::controllers::resolve_short_link))
        .route(
            "/:shortCode/session",
            get(crate::controllers::get_short_link_session),
        )
        .route(
            "/:shortCode/upload-url",
            get(crate::controllers::get_shortlink_upload_url),
        )
        .route(
            "/:shortCode/captcha",
            get(crate::controllers::get_shortlink_captcha),
        )
        .route(
            "/:shortCode/submit",
            post(crate::controllers::submit_shortlink_attendance),
        )
        .route(
            "/:shortCode/webauthn/status/:rollNumber",
            get(crate::controllers::get_webauthn_status),
        )
        .route(
            "/:shortCode/webauthn/register/start",
            post(crate::controllers::start_registration),
        )
        .route(
            "/:shortCode/webauthn/register/finish",
            post(crate::controllers::finish_registration),
        )
        .route(
            "/:shortCode/webauthn/authenticate/start",
            post(crate::controllers::start_authentication),
        )
        .route(
            "/:shortCode/webauthn/authenticate/conditional",
            post(crate::controllers::start_conditional_authentication),
        )
        .route(
            "/:shortCode/webauthn/authenticate/finish",
            post(crate::controllers::finish_authentication),
        )
        .layer(ServiceBuilder::new().layer(axum::middleware::from_fn(mobile_check_middleware)))
}
