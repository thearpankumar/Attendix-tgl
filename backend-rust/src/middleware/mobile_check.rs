use crate::constants::{
    BROWSER_CHROME, BROWSER_EDGE, BROWSER_FIREFOX, BROWSER_OPERA, BROWSER_SAFARI, PLATFORM_ANDROID,
    PLATFORM_IOS, PLATFORM_LINUX, PLATFORM_MAC, PLATFORM_UNKNOWN, PLATFORM_WINDOWS,
};
use crate::error::AppError;
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use once_cell::sync::Lazy;
use regex::Regex;
use std::sync::Arc;

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

// OS tokens that correspond ONLY to the two documented masquerading cases:
// iPadOS 13+ "desktop-class" Safari sends a Macintosh UA; Android
// Desktop Mode / Samsung DeX-style docking sends an X11/Linux UA. Windows
// and ChromeOS ("cros") are deliberately excluded — there is no legitimate
// "phone/tablet reports itself as Windows/ChromeOS" case, so those tokens
// must never grant this allowance, with or without any other signal.
static MASQUERADE_OS_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)(macintosh|x11|linux)").unwrap());

// ChromeOS Chrome UAs present as e.g. "(X11; CrOS x86_64 14541.0.0)" — the
// "X11" token above would otherwise let ChromeOS slip through
// MASQUERADE_OS_REGEX despite there being no documented masquerading case
// for it. Checked explicitly in `is_mobile_or_masquerading` rather than
// folded into the regex above (the `regex` crate has no negative lookahead).
static CROS_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)cros").unwrap());

/// Client-reported touch-capability signal — `navigator.maxTouchPoints` as a
/// plain decimal string. Sent by the student frontend on every scan-flow
/// request; see `useDeviceVerification.ts`'s `detectEmulation()` for the
/// client-side computation this mirrors. Attacker-settable like any header —
/// it raises the bar for casual UA-string spoofing, it is not a
/// cryptographic device attestation.
pub const TOUCH_POINTS_HEADER: &str = "x-attendix-touch-points";
/// Client-reported `(any-pointer: coarse)` media-query match — "1" or "0".
pub const COARSE_POINTER_HEADER: &str = "x-attendix-coarse-pointer";
/// Comma-joined list of automation-tool tells the client detected in itself
/// (`navigator.webdriver`, Selenium's injected `document.$cdc_*` markers —
/// see `deviceEvidenceHeaders()` in `StudentScan.tsx`). A genuine mobile
/// browser essentially never sets `navigator.webdriver`/carries these
/// markers, so unlike the UA/Client-Hint checks above this is treated as a
/// hard reject, not a soft signal — same class as `BOT_REGEX`. Still just a
/// header a sufficiently determined attacker can suppress client-side before
/// it's ever sent; raises the bar against curl/reqwest/Playwright-with-
/// defaults, not a cryptographic guarantee.
pub const AUTOMATION_SIGNALS_HEADER: &str = "x-attendix-automation-signals";

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
/// 6. Touch-capability header confirmation (X-Attendix-Touch-Points / X-Attendix-Coarse-Pointer) for the Mac/Linux masquerading allowance
pub async fn mobile_check_middleware(request: Request, next: Next) -> Result<Response, AppError> {
    let Some(device_info) = evaluate_bot_and_spoofing_checks(&request)? else {
        // Dev bypass active — skip the mobile requirement entirely.
        return Ok(next.run(request).await);
    };

    let user_agent = request
        .headers()
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !is_mobile_or_masquerading(user_agent, &device_info, request.headers()) {
        return Err(build_mobile_required_response());
    }

    Ok(next.run(request).await)
}

/// Same bot/spoofing checks as `mobile_check_middleware` (unconditional,
/// regardless of session kind or monitoring status), but the final "must be
/// mobile" rejection is skipped when the short code in the request path
/// resolves to a session that either IS `intern_monitoring` or has
/// `monitoring_enabled = true`. Applied only to the read-only info routes
/// (`/{shortCode}`, `/{shortCode}/session` — see
/// `routes::short_link::create_routes`'s `code_resolution_routes`): pairing
/// the browser extension for monitoring always means leaving this exact
/// page open in a desktop browser tab (see `StudentScan.tsx`'s `internMode`/
/// `monitoringMode` branches) — an intern-monitoring session needs that as
/// its *only* step (no GPS/photo/attendance at all), while an ordinary
/// monitored session needs it as a *second* step, after attendance is
/// marked from a phone. Every other scan-flow route (webauthn
/// register/authenticate, submit, upload-url, captcha) stays behind the
/// strict, session-kind-blind `mobile_check_middleware` above — passkey
/// registration/authentication (and, for an ordinary session, attendance
/// itself) still only happens from a phone, regardless of monitoring.
pub async fn resolution_mobile_check_middleware(
    State(state): State<Arc<crate::AppState>>,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let Some(device_info) = evaluate_bot_and_spoofing_checks(&request)? else {
        return Ok(next.run(request).await);
    };

    let user_agent = request
        .headers()
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if is_mobile_or_masquerading(user_agent, &device_info, request.headers()) {
        return Ok(next.run(request).await);
    }

    // Desktop, not masquerading — allowed through only when this short code
    // resolves to an intern-monitoring session or a session with monitoring
    // enabled. Any other case (ordinary un-monitored attendance session,
    // exam session, unknown/expired code) keeps the standard rejection; a
    // missing/invalid code intentionally fails closed into that same
    // rejection rather than being treated as a free pass.
    let short_code = request
        .uri()
        .path()
        .trim_start_matches('/')
        .split('/')
        .next()
        .unwrap_or("");
    let session: Option<(String, bool)> = sqlx::query_as(
        "SELECT s.session_kind, s.monitoring_enabled FROM sessions s \
         JOIN short_links sl ON sl.session_id = s.id \
         WHERE sl.short_code = $1 AND sl.is_active = true",
    )
    .bind(short_code)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if let Some((session_kind, monitoring_enabled)) = session {
        if session_kind == "intern_monitoring" || monitoring_enabled {
            return Ok(next.run(request).await);
        }
    }

    Err(build_mobile_required_response())
}

/// Bot-block plus the Sec-CH-UA-Mobile/platform spoofing checks shared by
/// both middlewares above. Returns the parsed `DeviceInfo` to make the
/// caller's own "must be mobile" decision with, or `None` if
/// `DEV_BYPASS_MOBILE_CHECK`/`DEV_BYPASS_ALL` is active (in which case no
/// further mobile-related check applies at all).
fn evaluate_bot_and_spoofing_checks(request: &Request) -> Result<Option<DeviceInfo>, AppError> {
    let bypass = std::env::var("DEV_BYPASS_MOBILE_CHECK")
        .map(|v| v == "true")
        .unwrap_or_else(|_| {
            // Also check DEV_BYPASS_ALL for backwards compatibility
            std::env::var("DEV_BYPASS_ALL")
                .map(|v| v == "true")
                .unwrap_or(false)
        });

    if bypass {
        return Ok(None);
    }

    let user_agent = request
        .headers()
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    // Block known bots and automation tools
    if BOT_REGEX.is_match(user_agent) {
        tracing::debug!(%user_agent, "Mobile check rejected an automated client");
        return Err(AppError::Forbidden(
            "Attendance must be marked from a mobile browser. Automated clients are not permitted."
                .to_string(),
        ));
    }

    // Same class of reject as BOT_REGEX above (not the flag-only anomaly
    // system): a genuine mobile browser never sets `navigator.webdriver` or
    // carries Selenium's injected markers, so the false-positive risk for a
    // real user is effectively zero. Closes the gap where a scripted client
    // (Playwright/Puppeteer with default settings) sends a consistent,
    // real-looking UA + Client-Hints and would otherwise sail through.
    if has_automation_signal(request.headers()) {
        tracing::debug!("Mobile check rejected an automation-tool signal (webdriver/CDC marker)");
        return Err(AppError::Forbidden(
            "Attendance must be marked from a real mobile browser.".to_string(),
        ));
    }

    // Detect-and-log only, deliberately NOT a hard reject yet: unlike the
    // webdriver/CDC check above (near-zero false-positive risk by
    // construction), this heuristic could plausibly misfire on an
    // uncommon/older real browser that skips Sec-Fetch-*/Client-Hints
    // without being Safari. Ship one release observing real traffic through
    // this log line before promoting it to a rejection.
    if missing_expected_sec_fetch_headers(user_agent, request.headers()) {
        tracing::warn!(
            %user_agent,
            "Mobile check: request has none of Sec-Fetch-Mode/Sec-CH-UA*/Safari-UA \
             (would reject as likely-scripted once promoted past observation)"
        );
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

    Ok(Some(device_info))
}

/// True if the device looks mobile/tablet outright, or presents as one of
/// the two documented masquerading cases (iPadOS 13+ sends Macintosh,
/// Android Desktop Mode sends X11/Linux) *and* backs that up with a
/// client-reported touch-capability signal that a UA-string/Client-Hint
/// override cannot fake (see `TOUCH_POINTS_HEADER`/`COARSE_POINTER_HEADER`).
/// A desktop OS token alone — Windows, ChromeOS, or an unconfirmed
/// Mac/Linux — is no longer sufficient: neither the UA string nor any
/// User-Agent Client Hint can reliably separate a real Mac/Linux desktop
/// from these two masquerading cases (Safari sends no Client Hints at all;
/// Chrome's own "Request Desktop Site" feature deliberately rewrites its
/// Client Hints too), so a hardware-derived signal is required instead.
fn is_mobile_or_masquerading(
    user_agent: &str,
    device_info: &DeviceInfo,
    headers: &axum::http::HeaderMap,
) -> bool {
    if device_info.is_mobile || device_info.is_tablet {
        return true;
    }

    // Real ChromeOS Chrome UAs also carry an "X11" token (e.g. "Mozilla/5.0
    // (X11; CrOS x86_64 14541.0.0) ..."), so MASQUERADE_OS_REGEX alone would
    // match ChromeOS too. There is no documented masquerading case for
    // ChromeOS (see MASQUERADE_OS_REGEX's doc comment), so it must be
    // excluded explicitly rather than relying on the OS-token regex alone.
    if CROS_REGEX.is_match(user_agent) {
        return false;
    }

    if !MASQUERADE_OS_REGEX.is_match(user_agent) {
        return false;
    }

    has_touch_evidence(headers)
}

/// Reads the client-reported touch-capability headers. Missing or malformed
/// values default to "no evidence" (fail closed) rather than erroring —
/// these headers are best-effort telemetry, not a required field.
fn has_touch_evidence(headers: &axum::http::HeaderMap) -> bool {
    let touch_points = headers
        .get(TOUCH_POINTS_HEADER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(0);

    let coarse_pointer = headers
        .get(COARSE_POINTER_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim() == "1")
        .unwrap_or(false);

    touch_points > 0 || coarse_pointer
}

/// True if the client itself reported a hard automation tell —
/// `navigator.webdriver === true` or a Selenium-injected `document.$cdc_*`/
/// `$wdc_*` marker. Deliberately narrower than the full signal set
/// `deviceEvidenceHeaders()` can compute (it also collects
/// `no-plugins-chrome`/`empty-languages`, which have legitimate false-
/// positive cases on hardened/privacy browsers and are sent for submit-time
/// scoring instead, not this hard-reject gate — see `useDeviceVerification.ts`).
fn has_automation_signal(headers: &axum::http::HeaderMap) -> bool {
    let Some(value) = headers
        .get(AUTOMATION_SIGNALS_HEADER)
        .and_then(|v| v.to_str().ok())
    else {
        return false;
    };

    value
        .split(',')
        .any(|s| matches!(s.trim(), "webdriver" | "cdc-markers"))
}

/// True only when EVERY real-browser signal is simultaneously absent: no
/// `Sec-Fetch-Mode` (auto-attached by `fetch()`/navigation in Chromium and
/// modern Firefox) AND no `Sec-CH-UA*` client hints AND the UA isn't Safari/
/// iOS (older Safari sends neither). A naive scripted client
/// (`reqwest`/`curl`/`python requests` with defaults) sets none of these; a
/// real browser — including older Safari, via the UA carve-out — always has
/// at least one. Supporting evidence only, folded into the same hard-reject
/// path as the other checks in `evaluate_bot_and_spoofing_checks` rather
/// than the flag-only anomaly system, for the same reason as
/// `has_automation_signal`: false-positive risk for genuine mobile traffic
/// is intentionally kept near zero by requiring ALL signals absent at once.
fn missing_expected_sec_fetch_headers(user_agent: &str, headers: &axum::http::HeaderMap) -> bool {
    let has_sec_fetch = headers.contains_key("sec-fetch-mode");
    let has_client_hints = headers.contains_key("sec-ch-ua")
        || headers.contains_key("sec-ch-ua-mobile")
        || headers.contains_key("sec-ch-ua-platform");

    let ua_lower = user_agent.to_lowercase();
    let is_safari_or_ios = (ua_lower.contains("safari") && !ua_lower.contains("chrome"))
        || ua_lower.contains("iphone")
        || ua_lower.contains("ipad");

    !has_sec_fetch && !has_client_hints && !is_safari_or_ios
}

/// Build a 403 response for spoofing detection.
///
/// These used to return a bare `StatusCode`, so the client got a 403 with an
/// empty body and no indication of why — a student on an unsupported device
/// saw a blank failure, and the same silence made the check easy to
/// misdiagnose as a fault in whatever handler sat behind it. Every other
/// rejection in this service returns `{"message": ...}`; these now match.
fn build_spoofing_response(message: &str) -> AppError {
    tracing::warn!("Mobile check spoofing detected: {}", message);
    AppError::Forbidden(message.to_string())
}

/// Build a 403 response for non-mobile devices.
fn build_mobile_required_response() -> AppError {
    AppError::Forbidden(
        "This page must be opened on a mobile device. Please scan the QR code with your phone."
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue};

    const MAC_SAFARI_UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15";
    const WINDOWS_CHROME_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
    const LINUX_FIREFOX_UA: &str =
        "Mozilla/5.0 (X11; Linux x86_64; rv:120.0) Gecko/20100101 Firefox/120.0";
    // Android Chrome after "Request Desktop Site" — Chrome rewrites both the
    // UA string and, per developer.chrome.com/blog/desktop-mode, the
    // Sec-CH-UA-Platform/-Mobile hints, to look like a real Linux desktop.
    const ANDROID_DESKTOP_MODE_UA: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
    const CROS_UA: &str = "Mozilla/5.0 (X11; CrOS x86_64 14541.0.0) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
    const IPHONE_UA: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1";

    fn headers_with_touch(points: &str, coarse: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(TOUCH_POINTS_HEADER, HeaderValue::from_str(points).unwrap());
        h.insert(
            COARSE_POINTER_HEADER,
            HeaderValue::from_str(coarse).unwrap(),
        );
        h
    }

    #[test]
    fn ipad_masquerading_as_mac_allowed_with_touch_evidence() {
        let info = check_mobile(MAC_SAFARI_UA);
        let headers = headers_with_touch("5", "1");
        assert!(is_mobile_or_masquerading(MAC_SAFARI_UA, &info, &headers));
    }

    #[test]
    fn mac_ua_without_touch_evidence_is_rejected() {
        // This is the actual gap being closed: previously this returned true
        // unconditionally via the bare Macintosh token match.
        let info = check_mobile(MAC_SAFARI_UA);
        let headers = HeaderMap::new();
        assert!(!is_mobile_or_masquerading(MAC_SAFARI_UA, &info, &headers));
    }

    #[test]
    fn real_windows_desktop_rejected_even_with_touch_headers() {
        // No masquerading story for Windows at all — a touchscreen Windows
        // laptop must still be rejected.
        let info = check_mobile(WINDOWS_CHROME_UA);
        let headers = headers_with_touch("10", "1");
        assert!(!is_mobile_or_masquerading(
            WINDOWS_CHROME_UA,
            &info,
            &headers
        ));
    }

    #[test]
    fn real_linux_desktop_without_touch_evidence_is_rejected() {
        let info = check_mobile(LINUX_FIREFOX_UA);
        let headers = HeaderMap::new();
        assert!(!is_mobile_or_masquerading(
            LINUX_FIREFOX_UA,
            &info,
            &headers
        ));
    }

    #[test]
    fn android_desktop_mode_allowed_with_touch_evidence() {
        let info = check_mobile(ANDROID_DESKTOP_MODE_UA);
        let headers = headers_with_touch("5", "1");
        assert!(is_mobile_or_masquerading(
            ANDROID_DESKTOP_MODE_UA,
            &info,
            &headers
        ));
    }

    #[test]
    fn chromeos_rejected_even_with_touch_headers() {
        // "cros" was deliberately dropped from the allowance regex — no
        // documented masquerading case for ChromeOS.
        let info = check_mobile(CROS_UA);
        let headers = headers_with_touch("10", "1");
        assert!(!is_mobile_or_masquerading(CROS_UA, &info, &headers));
    }

    #[test]
    fn real_iphone_still_allowed_with_no_headers_at_all() {
        // Regression guard: ordinary mobile UAs must not start depending on
        // the new headers.
        let info = check_mobile(IPHONE_UA);
        let headers = HeaderMap::new();
        assert!(is_mobile_or_masquerading(IPHONE_UA, &info, &headers));
    }

    #[test]
    fn malformed_touch_points_header_defaults_to_no_evidence() {
        let info = check_mobile(MAC_SAFARI_UA);
        let headers = headers_with_touch("not-a-number", "yes"); // "yes" != "1"
        assert!(!is_mobile_or_masquerading(MAC_SAFARI_UA, &info, &headers));
    }

    #[test]
    fn coarse_pointer_alone_is_sufficient_evidence() {
        // OR semantics: a coarse-pointer match with zero/absent touch points
        // still counts (belt-and-braces, not both-required).
        let info = check_mobile(MAC_SAFARI_UA);
        let headers = headers_with_touch("0", "1");
        assert!(is_mobile_or_masquerading(MAC_SAFARI_UA, &info, &headers));
    }

    #[test]
    fn touch_points_alone_is_sufficient_evidence() {
        let info = check_mobile(MAC_SAFARI_UA);
        let headers = headers_with_touch("5", "0");
        assert!(is_mobile_or_masquerading(MAC_SAFARI_UA, &info, &headers));
    }

    // ============ has_automation_signal ============

    fn headers_with_automation_signal(value: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(AUTOMATION_SIGNALS_HEADER, HeaderValue::from_str(value).unwrap());
        h
    }

    #[test]
    fn webdriver_signal_is_detected() {
        assert!(has_automation_signal(&headers_with_automation_signal("webdriver")));
    }

    #[test]
    fn cdc_markers_signal_is_detected() {
        assert!(has_automation_signal(&headers_with_automation_signal("cdc-markers")));
    }

    #[test]
    fn combined_signals_still_detected() {
        assert!(has_automation_signal(&headers_with_automation_signal(
            "no-plugins-chrome,webdriver,empty-languages"
        )));
    }

    #[test]
    fn informational_only_signals_alone_are_not_a_hard_signal() {
        // no-plugins-chrome / empty-languages are intentionally excluded from
        // the hard-reject set (real false-positive cases exist for those on
        // hardened/privacy browsers) — see the header's own doc comment.
        assert!(!has_automation_signal(&headers_with_automation_signal(
            "no-plugins-chrome,empty-languages"
        )));
    }

    #[test]
    fn no_header_at_all_is_not_a_signal() {
        assert!(!has_automation_signal(&HeaderMap::new()));
    }

    // ============ missing_expected_sec_fetch_headers ============

    #[test]
    fn scripted_client_with_nothing_set_is_flagged() {
        // A bare reqwest/curl/python-requests call with a spoofed mobile UA
        // but none of the headers a real browser auto-attaches. Deliberately
        // NOT an iPhone/Safari UA — those hit the carve-out below, which is
        // exactly the point of that separate test.
        const ANDROID_CHROME_UA: &str = "Mozilla/5.0 (Linux; Android 13; SM-G991B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Mobile Safari/537.36";
        assert!(missing_expected_sec_fetch_headers(
            ANDROID_CHROME_UA,
            &HeaderMap::new()
        ));
    }

    #[test]
    fn sec_fetch_mode_alone_is_sufficient_evidence_of_a_real_browser() {
        let mut h = HeaderMap::new();
        h.insert("sec-fetch-mode", HeaderValue::from_static("navigate"));
        assert!(!missing_expected_sec_fetch_headers(IPHONE_UA, &h));
    }

    #[test]
    fn a_client_hint_alone_is_sufficient_evidence_of_a_real_browser() {
        let mut h = HeaderMap::new();
        h.insert("sec-ch-ua-mobile", HeaderValue::from_static("?1"));
        assert!(!missing_expected_sec_fetch_headers(IPHONE_UA, &h));
    }

    #[test]
    fn safari_ios_carve_out_applies_even_with_zero_headers() {
        // Older Safari doesn't send Sec-Fetch-*/Client-Hints at all — must
        // not be penalized for that.
        assert!(!missing_expected_sec_fetch_headers(IPHONE_UA, &HeaderMap::new()));
        assert!(!missing_expected_sec_fetch_headers(MAC_SAFARI_UA, &HeaderMap::new()));
    }

    #[test]
    fn non_safari_desktop_ua_with_zero_headers_is_flagged() {
        // Chrome-on-Windows always sends at least one of these in practice;
        // a request claiming to be it with neither is suspicious.
        assert!(missing_expected_sec_fetch_headers(
            WINDOWS_CHROME_UA,
            &HeaderMap::new()
        ));
    }
}
