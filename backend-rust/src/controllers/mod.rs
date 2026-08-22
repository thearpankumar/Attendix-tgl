mod admin;
mod admin_security;
mod attendance;
mod batch;
mod location;
mod public_webauthn;
mod recurring_session_rule;
mod roster_source;
mod session;
mod session_records;
mod short_link;
mod webauthn;

// Re-export all controller functions
pub use admin::*;
pub use admin_security::*;
pub use attendance::*;
pub use batch::*;
pub use location::*;
pub use recurring_session_rule::*;
pub use roster_source::*;
pub use session::*;
pub use session_records::*;
pub use short_link::*;
pub use webauthn::*;

// Re-export specific public_webauthn functions (renamed to avoid conflicts)
pub use public_webauthn::{
    finish_authentication, finish_registration, get_captcha as get_shortlink_captcha,
    get_upload_url as get_shortlink_upload_url, get_webauthn_status, start_authentication,
    start_conditional_authentication, start_registration,
};
