// Admin controller module - split into logical submodules
//
// This module provides admin-facing API endpoints organized by domain:
// - auth: Authentication (login, register, profile)
// - dashboard: Dashboard statistics and visualizations
// - sessions: Session attendance management
// - flags: Flagged attendance review and verification
// - users: User Management (super-admin CRUD over admin/mentor accounts)
// - manual_attendance: Mentor roster + manual present/absent marking
// - excel_sessions: Bulk session creation from an uploaded master roster spreadsheet

mod auth;
mod dashboard;
mod excel_sessions;
mod flags;
mod manual_attendance;
mod sessions;
mod users;

// Re-export all public items from submodules
pub use auth::*;
pub use dashboard::*;
pub use excel_sessions::*;
pub use flags::*;
pub use manual_attendance::*;
pub use sessions::*;
pub use users::*;
