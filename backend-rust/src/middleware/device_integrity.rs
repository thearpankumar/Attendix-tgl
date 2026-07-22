use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityCheckResult {
    pub check_type: String,
    pub details: String,
}

pub fn check_timing_manipulation(
    performance_now: f64,
    date_now: i64,
) -> Option<IntegrityCheckResult> {
    let expected_now = (performance_now * 1_000_000.0) as i64;
    let diff = (date_now * 1_000_000 - expected_now).abs();

    if diff > 5_000_000 {
        return Some(IntegrityCheckResult {
            check_type: "TIMING_MANIPULATION".to_string(),
            details: format!("Timing drift detected: {} microseconds", diff),
        });
    }
    None
}

pub fn check_browser_api_consistency(
    platform: &str,
    touch_support: bool,
    max_touch_points: i32,
    _screen_width: i32,
) -> Option<IntegrityCheckResult> {
    let platform_lower = platform.to_lowercase();

    if (platform_lower.contains("android") || platform_lower.contains("iphone"))
        && max_touch_points == 0
    {
        return Some(IntegrityCheckResult {
            check_type: "BROWSER_API_INCONSISTENCY".to_string(),
            details: "Mobile platform reports no touch support".to_string(),
        });
    }

    if touch_support && max_touch_points == 0 {
        return Some(IntegrityCheckResult {
            check_type: "BROWSER_API_INCONSISTENCY".to_string(),
            details: "Touch API inconsistencies detected".to_string(),
        });
    }

    None
}

pub fn check_pointer_events(
    pointer_events_supported: bool,
    touch_events_supported: bool,
    is_mobile: bool,
) -> Option<IntegrityCheckResult> {
    if is_mobile && !pointer_events_supported && !touch_events_supported {
        return Some(IntegrityCheckResult {
            check_type: "POINTER_EVENTS_SUSPICIOUS".to_string(),
            details: "Mobile device without touch support".to_string(),
        });
    }
    None
}
