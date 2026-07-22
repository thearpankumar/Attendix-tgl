use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tower::ServiceBuilder;

use crate::middleware::{
    device_check_middleware, device_integrity_middleware, emulator_detection_middleware,
    gps_validation_middleware, mobile_check_middleware, student_rate_limit_middleware,
};
use crate::AppState;

pub fn create_routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/:token", get(crate::controllers::validate_token))
        .route(
            "/:token/status",
            get(crate::controllers::check_attendance_status),
        )
        .route(
            "/:token/upload-url",
            get(crate::controllers::get_upload_url),
        )
        .route("/:token/captcha", get(crate::controllers::get_captcha))
        .route("/:token", post(crate::controllers::submit_attendance))
        .layer(
            ServiceBuilder::new()
                .layer(axum::middleware::from_fn(mobile_check_middleware))
                .layer(axum::middleware::from_fn(gps_validation_middleware))
                .layer(axum::middleware::from_fn(emulator_detection_middleware))
                .layer(axum::middleware::from_fn(device_integrity_middleware))
                .layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    device_check_middleware,
                ))
                .layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    student_rate_limit_middleware,
                )),
        )
}
