use axum::{
    middleware,
    routing::{get, patch, post},
    Router,
};
use std::sync::Arc;

use crate::middleware::{
    admin_rate_limit_middleware, auth_middleware, csrf_middleware, login_rate_limit_middleware,
    registration_rate_limit_middleware,
};
use crate::AppState;

pub fn create_routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    // /register additionally gets its own tight, dedicated bucket — it's a
    // rarer action than login and the only thing standing between a leaked
    // ADMIN_SECRET and a new admin account.
    let register_route = Router::new()
        .route("/register", post(crate::controllers::register))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            registration_rate_limit_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            login_rate_limit_middleware,
        ));

    let public_routes = Router::new()
        .route("/login", post(crate::controllers::login))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            login_rate_limit_middleware,
        ))
        .merge(register_route);

    let protected_routes = Router::new()
        .route("/logout", post(crate::controllers::logout))
        .route("/profile", get(crate::controllers::get_profile))
        .route("/dashboard", get(crate::controllers::get_dashboard_stats))
        .route(
            "/dashboard/filters",
            get(crate::controllers::get_dashboard_filters),
        )
        .route(
            "/dashboard/recent-activity",
            get(crate::controllers::get_recent_activity),
        )
        .route(
            "/dashboard/attendance-series",
            get(crate::controllers::get_attendance_series),
        )
        .route(
            "/dashboard/sessions-by-date",
            get(crate::controllers::get_sessions_by_date),
        )
        .route("/system-health", get(crate::controllers::get_system_health))
        .route(
            "/locations",
            get(crate::controllers::get_locations).post(crate::controllers::create_location),
        )
        .route(
            "/locations/{id}",
            get(crate::controllers::get_location)
                .put(crate::controllers::update_location)
                .delete(crate::controllers::delete_location),
        )
        .route(
            "/sessions",
            get(crate::controllers::get_sessions).post(crate::controllers::create_session),
        )
        .route(
            "/sessions/{id}",
            get(crate::controllers::get_session)
                .patch(crate::controllers::deactivate_session)
                .delete(crate::controllers::delete_session),
        )
        .route(
            "/sessions/{id}/rotate",
            post(crate::controllers::rotate_token),
        )
        .route(
            "/sessions/{id}/schedule",
            patch(crate::controllers::update_session_schedule),
        )
        .route(
            "/sessions/{id}/deactivate",
            post(crate::controllers::deactivate_session),
        )
        .route(
            "/sessions/{id}/attendance",
            get(crate::controllers::get_session_attendance),
        )
        .route(
            "/sessions/{id}/stats",
            get(crate::controllers::get_session_stats),
        )
        .route(
            "/sessions/{id}/totp",
            get(crate::controllers::get_session_totp),
        )
        .route(
            "/sessions/{id}/devices",
            get(crate::controllers::get_session_devices),
        )
        .route(
            "/sessions/{id}/students/{rollNumber}/behavior",
            get(crate::controllers::get_student_behavior),
        )
        .route(
            "/sessions/{id}/students/{rollNumber}/behavior/live",
            get(crate::controllers::student_behavior_live),
        )
        .route(
            "/sessions/{id}/export",
            get(crate::controllers::export_session_attendance),
        )
        .route(
            "/sessions/export-bulk",
            post(crate::controllers::export_bulk_attendance),
        )
        .route(
            "/sessions/delete-bulk",
            post(crate::controllers::bulk_delete_sessions),
        )
        .route(
            "/sessions/stats-overview",
            get(crate::controllers::get_sessions_stats_overview),
        )
        .route(
            "/sessions/export-filtered",
            post(crate::controllers::export_sessions_filtered),
        )
        .route(
            "/sessions/{id}/mentors",
            patch(crate::controllers::update_session_mentors),
        )
        .route(
            "/sessions/{id}/absent",
            get(crate::controllers::get_session_absent),
        )
        .route(
            "/sessions/{id}/attendance/bulk-verify",
            post(crate::controllers::bulk_verify_attendance),
        )
        .route("/flagged", get(crate::controllers::get_flagged_attendance))
        .route(
            "/attendance/{id}/review",
            patch(crate::controllers::review_attendance),
        )
        .route(
            "/attendance/{id}/verify",
            patch(crate::controllers::verify_attendance),
        )
        .route(
            "/shortlinks",
            get(crate::controllers::get_short_links).post(crate::controllers::create_short_link),
        )
        .route(
            "/shortlinks/available-sessions",
            get(crate::controllers::get_available_sessions),
        )
        .route(
            "/shortlinks/{shortCode}",
            get(crate::controllers::get_short_link_by_code)
                .delete(crate::controllers::delete_short_link),
        )
        .route(
            "/shortlinks/{shortCode}/attach",
            post(crate::controllers::attach_short_link),
        )
        .route(
            "/shortlinks/{shortCode}/detach",
            post(crate::controllers::detach_short_link),
        )
        .route(
            "/webauthn/reset",
            post(crate::controllers::reset_credential),
        )
        .route(
            "/webauthn/suspend",
            post(crate::controllers::suspend_credential),
        )
        .route(
            "/webauthn/unsuspend",
            post(crate::controllers::unsuspend_credential),
        )
        .route(
            "/webauthn/credentials",
            get(crate::controllers::get_credentials),
        )
        .route(
            "/webauthn/stats",
            get(crate::controllers::get_webauthn_stats),
        )
        .route(
            "/batches",
            get(crate::controllers::get_batches).post(crate::controllers::upload_batch_excel),
        )
        .route(
            "/batches/{id}",
            get(crate::controllers::get_batch).delete(crate::controllers::delete_batch),
        )
        .route(
            "/users",
            get(crate::controllers::list_admin_users).post(crate::controllers::create_admin_user),
        )
        .route(
            "/users/{id}",
            patch(crate::controllers::update_admin_user)
                .delete(crate::controllers::delete_admin_user),
        )
        .route(
            "/profile/password",
            patch(crate::controllers::change_own_password),
        )
        .route(
            "/sessions/{id}/roster",
            get(crate::controllers::get_session_roster),
        )
        .route(
            "/sessions/{id}/attendance/manual",
            post(crate::controllers::mark_attendance_manual),
        )
        .route(
            "/sessions/{id}/attendance/manual/{rollNumber}",
            axum::routing::delete(crate::controllers::undo_manual_mark),
        )
        .route(
            "/excel-sessions/parse",
            post(crate::controllers::parse_excel_sessions),
        )
        .route(
            "/excel-sessions/commit",
            post(crate::controllers::commit_excel_sessions),
        )
        .route(
            "/recurring-rules",
            get(crate::controllers::get_recurring_rules)
                .post(crate::controllers::create_recurring_rule),
        )
        .route(
            "/recurring-rules/{id}",
            patch(crate::controllers::update_recurring_rule)
                .delete(crate::controllers::delete_recurring_rule),
        )
        .route(
            "/recurring-rules/{id}/pause",
            post(crate::controllers::pause_recurring_rule),
        )
        .route(
            "/recurring-rules/{id}/resume",
            post(crate::controllers::resume_recurring_rule),
        )
        // Middleware execution order (innermost applied last, runs first):
        // 1. admin_rate_limit  — outermost, applied first, limits traffic
        // 2. auth              — validates the HttpOnly cookie / Bearer token
        // 3. csrf              — validates the x-csrf-token header against the cookie
        //                        (only for mutating methods; GETs receive a fresh cookie)
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
        ));

    Router::new().merge(public_routes).merge(protected_routes)
}
