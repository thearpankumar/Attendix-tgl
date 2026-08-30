use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tower::ServiceBuilder;

use crate::middleware::{
    device_check_middleware, mobile_check_middleware, resolution_mobile_check_middleware,
    security_analysis_middleware, short_link_guess_rate_limit_middleware,
    student_rate_limit_middleware,
};
use crate::AppState;

pub fn create_routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    // The two routes that resolve a bare short code get an extra, tighter
    // guess-limiter, and a session-kind-aware mobile check instead of the
    // strict `mobile_check_middleware` used by `scan_flow_routes` below:
    // both are read-only lookups (a redirect and a session-info fetch), and
    // for an intern-monitoring session specifically, the student-frontend
    // page this resolves to needs to load in a desktop browser tab too — an
    // intern-monitoring session has no GPS/photo/attendance step, only a
    // passkey registration/pairing flow, and the extension-pairing half of
    // that requires leaving this exact page open on the laptop being paired
    // (see `StudentScan.tsx`'s `internMode` branch). Every *other* session
    // kind, and every other scan-flow route (webauthn register/authenticate,
    // submit, upload-url, captcha) — including for an intern-monitoring
    // session — stays strictly phone-only; see
    // `resolution_mobile_check_middleware`'s doc comment for the exact
    // split.
    let code_resolution_routes = Router::new()
        .route("/{shortCode}", get(crate::controllers::resolve_short_link))
        .route(
            "/{shortCode}/session",
            get(crate::controllers::get_short_link_session),
        )
        .layer(
            ServiceBuilder::new()
                .layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    resolution_mobile_check_middleware,
                ))
                .layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    short_link_guess_rate_limit_middleware,
                )),
        );

    // Telemetry ingestion (Phase 2 of session monitoring) is not a scan-flow
    // endpoint like the routes below — it's called by a desktop browser
    // extension, not necessarily a mobile browser, so it deliberately skips
    // `mobile_check_middleware`/`device_check_middleware`/
    // `security_analysis_middleware` and only gets rate limiting. Its own
    // handler enforces the real access control (the request must match the
    // session's active device lock). Merged in AFTER the scan-flow router
    // below is fully assembled and layered, not before — `Router::layer`
    // wraps everything merged into it so far, so merging this in earlier
    // would still catch it in the scan-flow's mobile/device checks despite
    // its own separate `.layer(...)` here.
    let telemetry_routes = Router::new()
        .route(
            "/{shortCode}/extension/events",
            post(crate::controllers::ingest_telemetry),
        )
        .route(
            "/{shortCode}/extension/pair/start",
            post(crate::controllers::start_pairing),
        )
        .route(
            "/{shortCode}/extension/pair/status/{pairingCode}",
            get(crate::controllers::pairing_status),
        )
        .route(
            "/{shortCode}/extension/pair/authenticate/finish",
            post(crate::controllers::finish_pairing_authentication),
        )
        .route(
            "/{shortCode}/extension/pair/finish",
            post(crate::controllers::finish_pairing),
        )
        .route(
            "/{shortCode}/extension/telemetry/token/refresh",
            post(crate::controllers::refresh_telemetry_token),
        )
        // Not shortCode-scoped like the routes above — see
        // `uninstall_extension`'s doc comment for why.
        .route(
            "/extension/uninstall",
            get(crate::controllers::uninstall_extension),
        )
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            student_rate_limit_middleware,
        ));

    let scan_flow_routes = Router::new()
        .route(
            "/{shortCode}/upload-url",
            get(crate::controllers::get_shortlink_upload_url),
        )
        .route(
            "/{shortCode}/captcha",
            get(crate::controllers::get_shortlink_captcha),
        )
        .route(
            "/{shortCode}/submit",
            post(crate::controllers::submit_attendance),
        )
        .route(
            "/{shortCode}/webauthn/status/{rollNumber}",
            get(crate::controllers::get_webauthn_status),
        )
        .route(
            "/{shortCode}/webauthn/register/start",
            post(crate::controllers::start_registration),
        )
        .route(
            "/{shortCode}/webauthn/register/finish",
            post(crate::controllers::finish_registration),
        )
        .route(
            "/{shortCode}/webauthn/authenticate/start",
            post(crate::controllers::start_authentication),
        )
        .route(
            "/{shortCode}/webauthn/authenticate/conditional",
            post(crate::controllers::start_conditional_authentication),
        )
        .route(
            "/{shortCode}/webauthn/authenticate/finish",
            post(crate::controllers::finish_authentication),
        )
        .layer(
            ServiceBuilder::new()
                .layer(axum::middleware::from_fn(mobile_check_middleware))
                .layer(axum::middleware::from_fn(security_analysis_middleware))
                .layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    device_check_middleware,
                ))
                .layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    student_rate_limit_middleware,
                )),
        );

    Router::new()
        .merge(code_resolution_routes)
        .merge(scan_flow_routes)
        .merge(telemetry_routes)
}
