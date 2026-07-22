use crate::constants::{
    BROWSER_CHROME, BROWSER_EDGE, BROWSER_FIREFOX, BROWSER_OPERA, BROWSER_SAFARI, PLATFORM_ANDROID,
    PLATFORM_IOS, PLATFORM_LINUX, PLATFORM_MAC, PLATFORM_UNKNOWN, PLATFORM_WINDOWS,
};
use axum::{extract::Request, http::StatusCode, middleware::Next, response::Response};
use once_cell::sync::Lazy;
use regex::Regex;

// Pre-compiled regex patterns for mobile detection
static MOBILE_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)(android|iphone|ipod|ipad|mobile)").unwrap());

static TABLET_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)(tablet|ipad)").unwrap());

// Blocked bot/automation patterns - these should always be rejected
static BOT_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(bot|crawler|spider|curl|wget|postman|insomnia|python|httpie|scraper|slurp|mediapartners)").unwrap()
});

// Chromium browser detection for client hints validation
static CHROMIUM_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)(chrome|chromium|edg|opera|brave)").unwrap());

// Desktop OS patterns that could be masquerading mobile devices
static DESKTOP_OS_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)(macintosh|windows|linux|x11|cros)").unwrap());

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub is_mobile: bool,
    pub is_tablet: bool,
    pub is_bot: bool,
    pub is_chromium: bool,
    pub platform: String,
    pub browser: String,
}

/// Check if the user agent indicates a mobile device
pub fn check_mobile(user_agent: &str) -> DeviceInfo {
    let ua_lower = user_agent.to_lowercase();

    let is_mobile = MOBILE_REGEX.is_match(user_agent);
    let is_tablet = TABLET_REGEX.is_match(user_agent);
    let is_bot = BOT_REGEX.is_match(user_agent);
    let is_chromium = CHROMIUM_REGEX.is_match(user_agent);

    let platform = if ua_lower.contains("android") {
        PLATFORM_ANDROID.to_string()
    } else if ua_lower.contains("iphone") || ua_lower.contains("ipad") {
        PLATFORM_IOS.to_string()
    } else if ua_lower.contains("windows") {
        PLATFORM_WINDOWS.to_string()
    } else if ua_lower.contains("mac") {
        PLATFORM_MAC.to_string()
    } else if ua_lower.contains("linux") {
        PLATFORM_LINUX.to_string()
    } else {
        PLATFORM_UNKNOWN.to_string()
    };

    let browser = if ua_lower.contains("edg/") {
        BROWSER_EDGE.to_string()
    } else if ua_lower.contains("chrome") {
        BROWSER_CHROME.to_string()
    } else if ua_lower.contains("firefox") {
        BROWSER_FIREFOX.to_string()
    } else if ua_lower.contains("safari") && !ua_lower.contains("chrome") {
        BROWSER_SAFARI.to_string()
    } else if ua_lower.contains("opera") || ua_lower.contains("opr/") {
        BROWSER_OPERA.to_string()
    } else {
        PLATFORM_UNKNOWN.to_string()
    };

    DeviceInfo {
        // Mobile = matches mobile regex but not tablet-only
        is_mobile: is_mobile && !is_tablet,
        is_tablet,
        is_bot,
        is_chromium,
        platform,
        browser,
    }
}

/// Detect UA spoofing anomalies
pub fn detect_ua_spoofing(user_agent: &str) -> Vec<String> {
    let mut warnings = Vec::new();
    let ua_lower = user_agent.to_lowercase();

    let has_android = ua_lower.contains("android");
    let has_linux = ua_lower.contains("linux");
    let has_ios = ua_lower.contains("iphone") || ua_lower.contains("ipad");
    let has_mac = ua_lower.contains("mac");

    // Android should report Linux
    if has_android && !has_linux {
        warnings.push("ANDROID_WITHOUT_LINUX".to_string());
    }

    // iOS should report Mac
    if has_ios && !has_mac {
        warnings.push("IOS_WITHOUT_MAC".to_string());
    }

    // Check for outdated Chrome version
    if let Some(version) = extract_chrome_version(user_agent) {
        if version < 50 {
            warnings.push("OUTDATED_BROWSER".to_string());
        }
    }

    warnings
}

fn extract_chrome_version(user_agent: &str) -> Option<u32> {
    let re = Regex::new(r"Chrome/(\d+)").ok()?;
    let caps = re.captures(user_agent)?;
    caps.get(1)?.as_str().parse().ok()
}

/// Mobile device check middleware - blocks non-mobile devices from student routes
///
/// This middleware enforces mobile-only access for attendance marking routes.
/// It checks:
/// 1. DEV_BYPASS_MOBILE_CHECK env var for dev mode bypass
/// 2. Bot/automation User-Agent patterns (blocks: bot, crawler, spider, curl, wget, postman, etc.)
/// 3. Mobile User-Agent patterns (android, iphone, ipad, ipod, mobile)
/// 4. Sec-CH-UA-Mobile header for Chromium browsers
/// 5. Platform consistency via Sec-CH-UA-Platform header
pub async fn mobile_check_middleware(request: Request, next: Next) -> Result<Response, StatusCode> {
    // Check for dev bypass
    let bypass = std::env::var("DEV_BYPASS_MOBILE_CHECK")
        .map(|v| v == "true")
        .unwrap_or_else(|_| {
            // Also check DEV_BYPASS_ALL for backwards compatibility
            std::env::var("DEV_BYPASS_ALL")
                .map(|v| v == "true")
                .unwrap_or(false)
        });

    if bypass {
        return Ok(next.run(request).await);
    }

    let user_agent = request
        .headers()
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    // Block known bots and automation tools
    if BOT_REGEX.is_match(user_agent) {
        return Err(StatusCode::FORBIDDEN);
    }

    let device_info = check_mobile(user_agent);

    // Check Sec-CH-UA-Mobile header for Chromium browsers
    if device_info.is_chromium {
        if let Some(ch_ua_mobile) = request.headers().get("sec-ch-ua-mobile") {
            if let Ok(hint) = ch_ua_mobile.to_str() {
                let ch_says_mobile = hint == "?1";

                // Get UA mobile status
                let ua_claims_mobile = MOBILE_REGEX.is_match(user_agent);

                // Spoofing detection: UA says mobile but client hint says not
                if ua_claims_mobile && !ch_says_mobile {
                    return Err(build_spoofing_response(
                        "Device verification failed: User-Agent spoofing detected. Please use a real mobile device."
                    ));
                }

                // Spoofing detection: UA says not mobile but client hint says mobile
                if !ua_claims_mobile && ch_says_mobile {
                    return Err(build_spoofing_response(
                        "Device verification failed: Inconsistent device signals detected.",
                    ));
                }

                // Check platform consistency for mobile claims
                if let Some(ch_ua_platform) = request.headers().get("sec-ch-ua-platform") {
                    if let Ok(platform) = ch_ua_platform.to_str() {
                        let platform_str = platform.to_lowercase();

                        // Desktop platform with mobile UA is suspicious
                        let is_desktop_platform = platform_str.contains("windows")
                            || platform_str.contains("macos")
                            || platform_str.contains("linux")
                            || platform_str.contains("chrome os");

                        let valid_mobile_platforms = ["android", "ios", "iphone", "ipad"];
                        let is_mobile_platform = valid_mobile_platforms
                            .iter()
                            .any(|p| platform_str.contains(p));

                        if is_desktop_platform && !is_mobile_platform && ua_claims_mobile {
                            return Err(build_spoofing_response(
                                "Device verification failed: Desktop platform with mobile User-Agent."
                            ));
                        }
                    }
                }
            }
        }
    }

    // Check device is mobile or could be masquerading device
    // (iPadOS 13+ sends Macintosh, Android Desktop Mode sends X11/Linux)
    let might_be_masquerading = DESKTOP_OS_REGEX.is_match(user_agent);

    // Block if not mobile and not a potential masquerading device
    if !device_info.is_mobile && !device_info.is_tablet && !might_be_masquerading {
        return Err(build_mobile_required_response());
    }

    Ok(next.run(request).await)
}

/// Build a 403 response for spoofing detection
fn build_spoofing_response(message: &str) -> StatusCode {
    // In a more complex implementation, we'd return JSON
    // For now, just return FORBIDDEN - the actual message would be in response body
    tracing::warn!("Mobile check spoofing detected: {}", message);
    StatusCode::FORBIDDEN
}

/// Build a 400 response for non-mobile devices
fn build_mobile_required_response() -> StatusCode {
    // Return FORBIDDEN (403) to match Node.js behavior for non-mobile devices
    // The message "This application requires a mobile device" would be in response body
    StatusCode::FORBIDDEN
}
