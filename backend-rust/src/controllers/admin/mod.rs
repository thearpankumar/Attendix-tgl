// Admin controller module - split into logical submodules
//
// This module provides admin-facing API endpoints organized by domain:
// - auth: Authentication (login, register, profile)
// - dashboard: Dashboard statistics and visualizations
// - sessions: Session attendance management
// - flags: Flagged attendance review and verification

mod auth;
mod dashboard;
mod flags;
mod sessions;

// Re-export all public items from submodules
pub use auth::*;
pub use dashboard::*;
pub use flags::*;
pub use sessions::*;
