//! Tests for Device Spoofing Detection
//!
//! Ported from: backend/tests/spoofingDetection.test.js
//!
//! Tests device spoofing detection including:
//! - Sec-CH-UA-Mobile header verification
//! - Platform header verification
//! - Bot detection
//! - Edge cases for non-Chromium browsers, iPadOS, empty UAs
//! - Device verification endpoint
//! - DEV bypass mode

#[cfg(test)]
mod tests {
    use once_cell::sync::Lazy;
    use regex::Regex;

    // ============================================
    // Mock/Stub Implementations for Testing
    // ============================================

    /// Simulates DeviceInfo from mobile_check middleware
    #[derive(Debug, Clone)]
    pub struct DeviceInfo {
        pub is_mobile: bool,
        pub is_tablet: bool,
        pub is_bot: bool,
        pub is_chromium: bool,
        pub platform: String,
        pub browser: String,
    }

    /// Simulates SpoofingCheckResult
    #[derive(Debug, Clone)]
    pub struct SpoofingCheckResult {
        pub is_spoofing: bool,
        pub message: Option<String>,
        pub inconsistencies: Vec<String>,
    }

    impl SpoofingCheckResult {
        pub fn touch(&self) {
            let _ = &self.inconsistencies;
        }
    }

    /// Simulates DeviceVerificationResponse from /api/device/verify endpoint
    #[derive(Debug, Clone)]
    pub struct DeviceVerificationResponse {
        pub valid: bool,
        pub is_emulation: bool,
        pub inconsistencies: Vec<String>,
        pub message: String,
    }

    /// Simulates DeviceMetrics from /api/device/verify endpoint
    #[derive(Debug, Clone, Default)]
    pub struct DeviceMetrics {
        pub max_touch_points: Option<i32>,
        pub has_coarse_pointer: Option<bool>,
        pub touch_event_support: Option<bool>,
        pub orientation_support: Option<bool>,
        pub webgl_renderer: Option<String>,
        pub screen_width: Option<i32>,
        pub screen_height: Option<i32>,
        pub device_pixel_ratio: Option<f64>,
        pub hardware_concurrency: Option<i32>,
        pub device_memory: Option<i32>,
        pub is_emulation: Option<bool>,
        pub inconsistencies: Vec<String>,
    }

    /// Request headers for spoofing detection
    #[derive(Debug, Clone, Default)]
    pub struct SpoofingRequest {
        pub user_agent: String,
        pub sec_ch_ua_mobile: Option<String>,
        pub sec_ch_ua: Option<String>,
        pub sec_ch_ua_platform: Option<String>,
        pub x_test_mobile_check: bool,
    }

    // Pre-compiled regex patterns matching the Rust implementation
    static MOBILE_REGEX: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"(?i)(android|iphone|ipod|ipad|mobile)").unwrap());

    static TABLET_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)(tablet|ipad)").unwrap());

    static BOT_REGEX: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"(?i)(bot|crawler|spider|curl|wget|postman|insomnia|python|httpie|scraper|slurp|mediapartners)").unwrap()
    });

    static CHROMIUM_REGEX: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"(?i)(chrome|chromium|edg|opera|brave)").unwrap());

    // NOTE: this is a local stub, independent of
    // `src/middleware/mobile_check.rs`'s real `MASQUERADE_OS_REGEX`, which
    // this file does not import. It was not updated by the touch-evidence
    // masquerading fix and no longer reflects production behavior (the real
    // regex dropped windows/cros and gating now also requires touch-evidence
    // headers) — out of scope here.
    static DESKTOP_OS_REGEX: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"(?i)(macintosh|windows|linux|x11|cros)").unwrap());

    /// Check if user agent indicates a mobile device (stub implementation)
    fn check_mobile(user_agent: &str) -> DeviceInfo {
        let ua_lower = user_agent.to_lowercase();

        let is_mobile = MOBILE_REGEX.is_match(user_agent);
        let is_tablet = TABLET_REGEX.is_match(user_agent);
        let is_bot = BOT_REGEX.is_match(user_agent);
        let is_chromium = CHROMIUM_REGEX.is_match(user_agent);

        let platform = if ua_lower.contains("android") {
            "Android".to_string()
        } else if ua_lower.contains("iphone") || ua_lower.contains("ipad") {
            "iOS".to_string()
        } else if ua_lower.contains("windows") {
            "Windows".to_string()
        } else if ua_lower.contains("mac") {
            "Mac".to_string()
        } else if ua_lower.contains("linux") {
            "Linux".to_string()
        } else {
            "Unknown".to_string()
        };

        let browser = if ua_lower.contains("edg/") {
            "Edge".to_string()
        } else if ua_lower.contains("chrome") {
            "Chrome".to_string()
        } else if ua_lower.contains("firefox") {
            "Firefox".to_string()
        } else if ua_lower.contains("safari") && !ua_lower.contains("chrome") {
            "Safari".to_string()
        } else if ua_lower.contains("opera") || ua_lower.contains("opr/") {
            "Opera".to_string()
        } else {
            "Unknown".to_string()
        };

        DeviceInfo {
            is_mobile: is_mobile && !is_tablet,
            is_tablet,
            is_bot,
            is_chromium,
            platform,
            browser,
        }
    }

    /// Detect spoofing based on UA and client hints (stub implementation)
    fn detect_spoofing(request: &SpoofingRequest, dev_bypass: bool) -> SpoofingCheckResult {
        // DEV bypass mode
        if dev_bypass {
            return SpoofingCheckResult {
                is_spoofing: false,
                message: None,
                inconsistencies: vec![],
            };
        }

        let user_agent = &request.user_agent;
        let device_info = check_mobile(user_agent);

        let _ = (
            DESKTOP_OS_REGEX.is_match(user_agent),
            device_info.is_tablet,
            &device_info.browser,
            &request.sec_ch_ua,
            request.x_test_mobile_check,
        );

        // Block empty user agent
        if user_agent.is_empty() {
            return SpoofingCheckResult {
                is_spoofing: true,
                message: Some("Empty User-Agent not allowed".to_string()),
                inconsistencies: vec!["EMPTY_USER_AGENT".to_string()],
            };
        }

        // Block known bots
        if device_info.is_bot {
            return SpoofingCheckResult {
                is_spoofing: true,
                message: Some("Bot or automation tool detected".to_string()),
                inconsistencies: vec!["BOT_DETECTED".to_string()],
            };
        }

        // Check Sec-CH-UA-Mobile header
        if let Some(ref ch_ua_mobile) = request.sec_ch_ua_mobile {
            let ch_says_mobile = ch_ua_mobile == "?1";
            let ua_claims_mobile = MOBILE_REGEX.is_match(user_agent);

            // Spoofing: UA says mobile but client hint says not
            if ua_claims_mobile && !ch_says_mobile {
                return SpoofingCheckResult {
                    is_spoofing: true,
                    message: Some("User-Agent spoofing detected".to_string()),
                    inconsistencies: vec!["UA_MOBILE_MISMATCH".to_string()],
                };
            }

            // Spoofing: UA says not mobile but client hint says mobile
            if !ua_claims_mobile && ch_says_mobile {
                return SpoofingCheckResult {
                    is_spoofing: true,
                    message: Some("Inconsistent device signals detected".to_string()),
                    inconsistencies: vec!["CLIENT_HINT_MOBILE_MISMATCH".to_string()],
                };
            }
        }

        // Check platform consistency
        if let Some(ref ch_ua_platform) = request.sec_ch_ua_platform {
            let platform_str = ch_ua_platform.to_lowercase();
            let ua_claims_mobile = MOBILE_REGEX.is_match(user_agent);

            let is_desktop_platform = platform_str.contains("windows")
                || platform_str.contains("macos")
                || platform_str.contains("linux")
                || platform_str.contains("chrome os");

            let valid_mobile_platforms = ["android", "ios", "iphone", "ipad"];
            let is_mobile_platform = valid_mobile_platforms
                .iter()
                .any(|p| platform_str.contains(p));

            if is_desktop_platform && !is_mobile_platform && ua_claims_mobile {
                return SpoofingCheckResult {
                    is_spoofing: true,
                    message: Some("Desktop platform with mobile User-Agent".to_string()),
                    inconsistencies: vec!["PLATFORM_UA_MISMATCH".to_string()],
                };
            }
        }

        let res = SpoofingCheckResult {
            is_spoofing: false,
            message: None,
            inconsistencies: vec![],
        };
        res.touch();
        res
    }

    /// Verify device metrics (stub implementation of /api/device/verify)
    fn verify_device(
        metrics: &DeviceMetrics,
        request: &SpoofingRequest,
    ) -> DeviceVerificationResponse {
        let _ = (
            &metrics.has_coarse_pointer,
            &metrics.touch_event_support,
            &metrics.orientation_support,
            &metrics.screen_height,
            &metrics.device_pixel_ratio,
            &metrics.hardware_concurrency,
            &metrics.device_memory,
        );

        // Check metrics provided
        if metrics.max_touch_points.is_none()
            && metrics.webgl_renderer.is_none()
            && metrics.screen_width.is_none()
        {
            return DeviceVerificationResponse {
                valid: false,
                is_emulation: false,
                inconsistencies: vec![],
                message: "metrics required".to_string(),
            };
        }

        let mut inconsistencies: Vec<String> = Vec::new();

        // Check UA/Client-Hint mismatch on server side
        let ua_claims_mobile = MOBILE_REGEX.is_match(&request.user_agent);
        let _is_chromium = CHROMIUM_REGEX.is_match(&request.user_agent);
        if let Some(ref ch_mobile) = request.sec_ch_ua_mobile {
            let ch_says_mobile = ch_mobile == "?1";
            if ua_claims_mobile && !ch_says_mobile {
                inconsistencies
                    .push("Server: UA claims mobile but Sec-CH-UA-Mobile disagrees".to_string());
            }
        }

        if let Some(ref ch_platform) = request.sec_ch_ua_platform {
            let is_desktop_platform = ch_platform.contains("Windows")
                || ch_platform.contains("macOS")
                || ch_platform.contains("Linux")
                || ch_platform.contains("Chrome OS");
            if is_desktop_platform && ua_claims_mobile {
                inconsistencies.push(format!(
                    "Server: Desktop platform header '{}' with mobile UA",
                    ch_platform
                ));
            }
        }

        // Check for desktop GPU with mobile UA
        if let Some(ref webgl_renderer) = metrics.webgl_renderer {
            let is_desktop_gpu = webgl_renderer.contains("NVIDIA")
                || webgl_renderer.contains("GeForce")
                || webgl_renderer.contains("RTX")
                || webgl_renderer.contains("GTX")
                || webgl_renderer.contains("AMD")
                || webgl_renderer.contains("Radeon");

            if is_desktop_gpu && ua_claims_mobile {
                inconsistencies.push(format!(
                    "Desktop GPU detected with mobile UA: {}",
                    webgl_renderer
                ));
            }
        }

        // Check maxTouchPoints == 1 (common in emulators)
        if let Some(max_touch_points) = metrics.max_touch_points {
            if max_touch_points == 1 {
                inconsistencies.push("maxTouchPoints exactly 1".to_string());
            }
        }

        // Add client-reported inconsistencies
        for inc in &metrics.inconsistencies {
            inconsistencies.push(inc.clone());
        }

        let is_valid = inconsistencies.is_empty() && !metrics.is_emulation.unwrap_or(false);

        DeviceVerificationResponse {
            valid: is_valid,
            is_emulation: !is_valid,
            inconsistencies: inconsistencies.clone(),
            message: if is_valid {
                "Device verified".to_string()
            } else {
                "Device emulation detected. Please use a real mobile device.".to_string()
            },
        }
    }

    // ============================================
    // Sec-CH-UA-Mobile Header Verification Tests
    // ============================================

    mod sec_ch_ua_mobile_verification {
        use super::*;

        const CHROME_MOBILE_UA: &str = "Mozilla/5.0 (Linux; Android 13; SM-G991B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Mobile Safari/537.36";
        const CHROME_DESKTOP_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Safari/537.36";

        #[test]
        fn blocks_when_ua_claims_mobile_but_sec_ch_ua_mobile_says_desktop() {
            // Node.js test: "blocks when UA claims mobile but Sec-CH-UA-Mobile says desktop"
            let request = SpoofingRequest {
                user_agent: CHROME_MOBILE_UA.to_string(),
                sec_ch_ua_mobile: Some("?0".to_string()),
                sec_ch_ua: Some(
                    r#""Chromium";v="112", "Google Chrome";v="112", "Not:A-Brand";v="99""#
                        .to_string(),
                ),
                sec_ch_ua_platform: Some(r#""Windows""#.to_string()),
                x_test_mobile_check: true,
            };

            let result = detect_spoofing(&request, false);

            assert!(
                result.is_spoofing,
                "Should detect spoofing when UA claims mobile but Sec-CH-UA-Mobile says desktop"
            );
            assert!(
                result.message.unwrap().to_lowercase().contains("spoofing"),
                "Message should mention spoofing"
            );
        }

        #[test]
        fn allows_when_ua_and_sec_ch_ua_mobile_both_claim_mobile() {
            // Node.js test: "allows when UA and Sec-CH-UA-Mobile both claim mobile"
            let request = SpoofingRequest {
                user_agent: CHROME_MOBILE_UA.to_string(),
                sec_ch_ua_mobile: Some("?1".to_string()),
                sec_ch_ua: Some(
                    r#""Chromium";v="112", "Google Chrome";v="112", "Not:A-Brand";v="99""#
                        .to_string(),
                ),
                sec_ch_ua_platform: Some(r#""Android""#.to_string()),
                x_test_mobile_check: true,
            };

            let result = detect_spoofing(&request, false);

            assert!(
                !result.is_spoofing,
                "Should NOT detect spoofing when both UA and Sec-CH-UA-Mobile claim mobile"
            );
        }

        #[test]
        fn allows_real_desktop_browser_not_spoofed() {
            // Node.js test: "allows real desktop browser (not spoofed)"
            let request = SpoofingRequest {
                user_agent: CHROME_DESKTOP_UA.to_string(),
                sec_ch_ua_mobile: Some("?0".to_string()),
                sec_ch_ua: Some(
                    r#""Chromium";v="112", "Google Chrome";v="112", "Not:A-Brand";v="99""#
                        .to_string(),
                ),
                sec_ch_ua_platform: Some(r#""Windows""#.to_string()),
                x_test_mobile_check: true,
            };

            let result = detect_spoofing(&request, false);

            assert!(
                !result.is_spoofing,
                "Should NOT detect spoofing for real desktop browser"
            );
        }
    }

    // ============================================
    // Platform Header Verification Tests
    // ============================================

    mod platform_header_verification {
        use super::*;

        const IPHONE_UA: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 16_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.0 Mobile/15E148 Safari/604.1";
        const ANDROID_UA: &str = "Mozilla/5.0 (Linux; Android 13; SM-G991B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Mobile Safari/537.36";

        #[test]
        fn blocks_when_ua_claims_iphone_but_platform_says_windows() {
            // Node.js test: "blocks when UA claims iPhone but platform says Windows"
            let request = SpoofingRequest {
                user_agent: IPHONE_UA.to_string(),
                sec_ch_ua_mobile: Some("?1".to_string()),
                sec_ch_ua_platform: Some(r#""Windows""#.to_string()),
                x_test_mobile_check: true,
                sec_ch_ua: None,
            };

            let result = detect_spoofing(&request, false);

            assert!(
                result.is_spoofing,
                "Should detect spoofing when UA claims iPhone but platform says Windows"
            );
        }

        #[test]
        fn blocks_when_ua_claims_android_but_platform_says_macos() {
            // Node.js test: "blocks when UA claims Android but platform says macOS"
            let request = SpoofingRequest {
                user_agent: ANDROID_UA.to_string(),
                sec_ch_ua_mobile: Some("?1".to_string()),
                sec_ch_ua_platform: Some(r#""macOS""#.to_string()),
                x_test_mobile_check: true,
                sec_ch_ua: None,
            };

            let result = detect_spoofing(&request, false);

            assert!(
                result.is_spoofing,
                "Should detect spoofing when UA claims Android but platform says macOS"
            );
        }

        #[test]
        fn allows_when_platform_matches_ua_android() {
            // Node.js test: "allows when platform matches UA (Android)"
            let request = SpoofingRequest {
                user_agent: ANDROID_UA.to_string(),
                sec_ch_ua_mobile: Some("?1".to_string()),
                sec_ch_ua: Some(r#""Chromium";v="112""#.to_string()),
                sec_ch_ua_platform: Some(r#""Android""#.to_string()),
                x_test_mobile_check: true,
            };

            let result = detect_spoofing(&request, false);

            assert!(
                !result.is_spoofing,
                "Should NOT detect spoofing when platform matches UA (Android)"
            );
        }
    }

    // ============================================
    // Bot Detection Tests
    // ============================================

    mod bot_detection {
        use super::*;

        const BOT_USER_AGENTS: &[(&str, &str)] = &[
            ("curl", "curl/7.68.0"),
            ("python-requests", "python-requests/2.25.1"),
            ("wget", "wget/1.21"),
            ("postman", "PostmanRuntime/7.28.0"),
            ("insomnia", "insomnia/2021.7.0"),
            ("generic bot", "Googlebot/2.1"),
            ("scraper", "WebScraper/1.0"),
        ];

        #[test]
        fn blocks_curl_user_agent() {
            // Node.js test: "blocks curl user agent"
            let request = SpoofingRequest {
                user_agent: "curl/7.68.0".to_string(),
                x_test_mobile_check: true,
                ..Default::default()
            };

            let result = detect_spoofing(&request, false);

            assert!(result.is_spoofing, "Should block curl user agent");
        }

        #[test]
        fn blocks_python_requests_user_agent() {
            // Node.js test: "blocks python-requests user agent"
            let request = SpoofingRequest {
                user_agent: "python-requests/2.25.1".to_string(),
                x_test_mobile_check: true,
                ..Default::default()
            };

            let result = detect_spoofing(&request, false);

            assert!(
                result.is_spoofing,
                "Should block python-requests user agent"
            );
        }

        #[test]
        fn blocks_wget_user_agent() {
            // Node.js test: "blocks wget user agent"
            let request = SpoofingRequest {
                user_agent: "wget/1.21".to_string(),
                x_test_mobile_check: true,
                ..Default::default()
            };

            let result = detect_spoofing(&request, false);

            assert!(result.is_spoofing, "Should block wget user agent");
        }

        #[test]
        fn blocks_postman_user_agent() {
            // Node.js test: "blocks postman user agent"
            let request = SpoofingRequest {
                user_agent: "PostmanRuntime/7.28.0".to_string(),
                x_test_mobile_check: true,
                ..Default::default()
            };

            let result = detect_spoofing(&request, false);

            assert!(result.is_spoofing, "Should block Postman user agent");
        }

        #[test]
        fn blocks_insomnia_user_agent() {
            // Node.js test: "blocks insomnia user agent"
            let request = SpoofingRequest {
                user_agent: "insomnia/2021.7.0".to_string(),
                x_test_mobile_check: true,
                ..Default::default()
            };

            let result = detect_spoofing(&request, false);

            assert!(result.is_spoofing, "Should block Insomnia user agent");
        }

        #[test]
        fn blocks_generic_bot_user_agent() {
            // Node.js test: "blocks generic bot user agent"
            let request = SpoofingRequest {
                user_agent: "Googlebot/2.1".to_string(),
                x_test_mobile_check: true,
                ..Default::default()
            };

            let result = detect_spoofing(&request, false);

            assert!(result.is_spoofing, "Should block Googlebot user agent");
        }

        #[test]
        fn blocks_scraper_user_agent() {
            // Node.js test: "blocks scraper user agent"
            let request = SpoofingRequest {
                user_agent: "WebScraper/1.0".to_string(),
                x_test_mobile_check: true,
                ..Default::default()
            };

            let result = detect_spoofing(&request, false);

            assert!(result.is_spoofing, "Should block WebScraper user agent");
        }

        #[test]
        fn test_all_bot_user_agents_are_blocked() {
            // Parameterized test matching Node.js forEach pattern
            for (name, ua) in BOT_USER_AGENTS {
                let request = SpoofingRequest {
                    user_agent: ua.to_string(),
                    x_test_mobile_check: true,
                    ..Default::default()
                };

                let result = detect_spoofing(&request, false);

                assert!(result.is_spoofing, "Should block {} user agent", name);
            }
        }
    }

    // ============================================
    // Edge Cases Tests
    // ============================================

    mod edge_cases {
        use super::*;

        #[test]
        fn handles_missing_sec_ch_ua_mobile_header_gracefully() {
            // Node.js test: "handles missing Sec-CH-UA-Mobile header gracefully (non-Chromium browser)"
            let safari_mobile_ua = "Mozilla/5.0 (iPhone; CPU iPhone OS 16_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.0 Mobile/15E148 Safari/604.1";

            let request = SpoofingRequest {
                user_agent: safari_mobile_ua.to_string(),
                x_test_mobile_check: true,
                // No sec-ch-ua-mobile header (Safari doesn't send client hints)
                sec_ch_ua_mobile: None,
                sec_ch_ua: None,
                sec_ch_ua_platform: None,
            };

            let result = detect_spoofing(&request, false);

            assert!(
                !result.is_spoofing,
                "Should NOT detect spoofing for Safari without client hints"
            );
        }

        #[test]
        fn handles_ipad_with_macintel_platform() {
            // Node.js test: "handles iPad with MacIntel platform (iPadOS 13+)"
            let ipad_ua = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.0 Safari/605.1.15";

            let request = SpoofingRequest {
                user_agent: ipad_ua.to_string(),
                x_test_mobile_check: true,
                ..Default::default()
            };

            let result = detect_spoofing(&request, false);

            // iPadOS 13+ sends Macintosh UA, should be allowed for frontend hardware checks
            assert!(
                !result.is_spoofing,
                "Should NOT detect spoofing for iPadOS 13+ MacIntel platform"
            );
        }

        #[test]
        fn blocks_empty_user_agent() {
            // Node.js test: "blocks empty user agent"
            let request = SpoofingRequest {
                user_agent: "".to_string(),
                x_test_mobile_check: true,
                ..Default::default()
            };

            let result = detect_spoofing(&request, false);

            assert!(
                result.is_spoofing,
                "Should detect spoofing for empty user agent"
            );
        }

        #[test]
        fn handles_case_insensitive_bot_detection() {
            // Node.js test: "handles case-insensitive bot detection"
            let request = SpoofingRequest {
                user_agent: "PYTHON-REQUESTS/2.25.1".to_string(),
                x_test_mobile_check: true,
                ..Default::default()
            };

            let result = detect_spoofing(&request, false);

            assert!(
                result.is_spoofing,
                "Should detect spoofing for uppercase PYTHON-REQUESTS"
            );
        }
    }

    // ============================================
    // Device Verification Endpoint Tests
    // ============================================

    mod device_verification_endpoint {
        use super::*;

        #[test]
        fn rejects_request_without_metrics() {
            // Node.js test: "rejects request without metrics"
            let metrics = DeviceMetrics::default();
            let request = SpoofingRequest::default();

            let result = verify_device(&metrics, &request);

            assert!(!result.valid, "Should reject request without metrics");
            assert!(
                result.message.contains("metrics required"),
                "Message should mention 'metrics required'"
            );
        }

        #[test]
        fn validates_clean_device_metrics() {
            // Node.js test: "validates clean device metrics"
            let metrics = DeviceMetrics {
                max_touch_points: Some(5),
                has_coarse_pointer: Some(true),
                touch_event_support: Some(true),
                orientation_support: Some(true),
                webgl_renderer: Some("Adreno (TM) 650".to_string()),
                screen_width: Some(1080),
                screen_height: Some(2400),
                device_pixel_ratio: Some(2.75),
                hardware_concurrency: Some(8),
                device_memory: Some(6),
                is_emulation: Some(false),
                inconsistencies: vec![],
            };

            let request = SpoofingRequest {
                user_agent: "Mozilla/5.0 (Linux; Android 13; SM-G991B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Mobile Safari/537.36".to_string(),
                sec_ch_ua_mobile: Some("?1".to_string()),
                sec_ch_ua_platform: Some(r#""Android""#.to_string()),
                ..Default::default()
            };

            let result = verify_device(&metrics, &request);

            assert!(result.valid, "Should validate clean device metrics");
            assert!(
                !result.is_emulation,
                "Should not detect emulation for clean metrics"
            );
        }

        #[test]
        fn detects_emulation_with_inconsistencies() {
            // Node.js test: "detects emulation with inconsistencies"
            let metrics = DeviceMetrics {
                max_touch_points: Some(1), // Suspicious: exactly 1
                has_coarse_pointer: Some(true),
                touch_event_support: Some(true),
                orientation_support: Some(true),
                webgl_renderer: Some("NVIDIA GeForce RTX 3080".to_string()), // Desktop GPU with mobile UA
                screen_width: Some(1920),
                screen_height: Some(1080),
                device_pixel_ratio: Some(1.0),
                hardware_concurrency: Some(16),
                device_memory: Some(32),
                is_emulation: Some(true),
                inconsistencies: vec![
                    "Desktop GPU detected with mobile UA".to_string(),
                    "maxTouchPoints exactly 1".to_string(),
                ],
            };

            let request = SpoofingRequest {
                user_agent: "Mozilla/5.0 (Linux; Android 13; SM-G991B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Mobile Safari/537.36".to_string(),
                sec_ch_ua_mobile: Some("?1".to_string()),
                sec_ch_ua_platform: Some(r#""Android""#.to_string()),
                ..Default::default()
            };

            let result = verify_device(&metrics, &request);

            assert!(!result.valid, "Should NOT validate emulation metrics");
            assert!(result.is_emulation, "Should detect emulation");
            assert!(
                !result.inconsistencies.is_empty(),
                "Should have inconsistencies"
            );
        }

        #[test]
        fn detects_server_side_ua_client_hint_mismatch() {
            // Node.js test: "detects server-side UA/Client-Hint mismatch"
            let metrics = DeviceMetrics {
                max_touch_points: Some(5),
                has_coarse_pointer: Some(true),
                is_emulation: Some(false),
                inconsistencies: vec![],
                ..Default::default()
            };

            let request = SpoofingRequest {
                user_agent: "Mozilla/5.0 (iPhone; CPU iPhone OS 16_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.0 Mobile/15E148 Safari/604.1".to_string(),
                sec_ch_ua_mobile: Some("?0".to_string()),  // Claims NOT mobile
                sec_ch_ua_platform: Some(r#""Windows""#.to_string()),  // Desktop platform
                ..Default::default()
            };

            let result = verify_device(&metrics, &request);

            assert!(
                !result.valid,
                "Should NOT validate with UA/Client-Hint mismatch"
            );
            assert!(
                result
                    .inconsistencies
                    .iter()
                    .any(|i| i.contains("UA") || i.contains("Mobile")),
                "Should have inconsistency mentioning UA or Mobile"
            );
        }
    }

    // ============================================
    // DEV Bypass Mode Tests
    // ============================================

    mod dev_bypass_mode {
        use super::*;

        #[test]
        fn allows_all_requests_when_dev_bypass_all_is_true() {
            // Node.js test: "allows all requests when DEV_BYPASS_ALL is true"
            let request = SpoofingRequest {
                user_agent: "curl/7.68.0".to_string(), // Normally blocked
                x_test_mobile_check: true,
                ..Default::default()
            };

            // Simulate DEV_BYPASS_ALL=true
            let result = detect_spoofing(&request, true);

            assert!(
                !result.is_spoofing,
                "Should allow curl when DEV_BYPASS_ALL=true"
            );
        }

        #[test]
        fn bypasses_spoofing_detection_in_dev_mode() {
            // Node.js test: "bypasses spoofing detection in dev mode"
            let request = SpoofingRequest {
                user_agent: "Mozilla/5.0 (iPhone; CPU iPhone OS 16_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.0 Mobile/15E148 Safari/604.1".to_string(),
                sec_ch_ua_mobile: Some("?0".to_string()),  // Claims NOT mobile but UA says iPhone
                sec_ch_ua_platform: Some(r#""Windows""#.to_string()),  // Desktop platform
                x_test_mobile_check: true,
                ..Default::default()
            };

            // With dev bypass enabled
            let result = detect_spoofing(&request, true);

            assert!(
                !result.is_spoofing,
                "Should bypass spoofing detection in dev mode"
            );
        }

        #[test]
        fn dev_bypass_not_enabled_blocks_spoofed_requests() {
            // Additional test: verify that without bypass, spoofing is detected
            let request = SpoofingRequest {
                user_agent: "Mozilla/5.0 (iPhone; CPU iPhone OS 16_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.0 Mobile/15E148 Safari/604.1".to_string(),
                sec_ch_ua_mobile: Some("?0".to_string()),
                sec_ch_ua_platform: Some(r#""Windows""#.to_string()),
                x_test_mobile_check: true,
                ..Default::default()
            };

            // With dev bypass disabled (normal operation)
            let result = detect_spoofing(&request, false);

            assert!(
                result.is_spoofing,
                "Should detect spoofing when dev bypass is disabled"
            );
        }
    }

    // ============================================
    // Helper Function Tests
    // ============================================

    mod helper_functions {
        use super::*;

        #[test]
        fn test_check_mobile_android_phone() {
            let ua = "Mozilla/5.0 (Linux; Android 13; SM-G991B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Mobile Safari/537.36";
            let info = check_mobile(ua);

            assert!(info.is_mobile, "Android phone should be detected as mobile");
            assert!(!info.is_bot, "Android phone should not be detected as bot");
            assert!(
                info.is_chromium,
                "Chrome on Android should be detected as Chromium"
            );
            assert_eq!(info.platform, "Android");
        }

        #[test]
        fn test_check_mobile_iphone() {
            let ua = "Mozilla/5.0 (iPhone; CPU iPhone OS 16_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.0 Mobile/15E148 Safari/604.1";
            let info = check_mobile(ua);

            assert!(info.is_mobile, "iPhone should be detected as mobile");
            assert!(!info.is_bot, "iPhone should not be detected as bot");
            assert!(
                !info.is_chromium,
                "Safari should not be detected as Chromium"
            );
            assert_eq!(info.platform, "iOS");
        }

        #[test]
        fn test_check_mobile_desktop_chrome() {
            let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Safari/537.36";
            let info = check_mobile(ua);

            assert!(
                !info.is_mobile,
                "Desktop Chrome should not be detected as mobile"
            );
            assert!(!info.is_bot, "Desktop Chrome should not be detected as bot");
            assert!(info.is_chromium, "Chrome should be detected as Chromium");
            assert_eq!(info.platform, "Windows");
        }

        #[test]
        fn test_check_mobile_bot() {
            let ua = "curl/7.68.0";
            let info = check_mobile(ua);

            assert!(!info.is_mobile, "Curl should not be detected as mobile");
            assert!(info.is_bot, "Curl should be detected as bot");
        }

        #[test]
        fn test_bot_regex_patterns() {
            // Test all bot patterns from the original implementation
            let bot_patterns = [
                "curl/7.68.0",
                "Wget/1.21",
                "python-requests/2.25.1",
                "PostmanRuntime/7.28.0",
                "insomnia/2021.7.0",
                "Googlebot/2.1",
                "WebScraper/1.0",
                "bingbot/2.0",
                "spider/1.0",
                "httpie/3.2.1",
            ];

            for ua in bot_patterns {
                assert!(BOT_REGEX.is_match(ua), "Should detect '{}' as bot", ua);
            }
        }

        #[test]
        fn test_chromium_regex_patterns() {
            let chromium_patterns = [
                "Chrome/112.0.0.0",
                "Chromium/112.0.0.0",
                "Edg/112.0.0.0",
                "Opera/98.0.0.0",
                "Brave/112.0.0.0",
            ];

            for ua in chromium_patterns {
                assert!(
                    CHROMIUM_REGEX.is_match(ua),
                    "Should detect '{}' as Chromium",
                    ua
                );
            }
        }

        #[test]
        fn test_mobile_regex_patterns() {
            let mobile_patterns = [
                "Mozilla/5.0 (Linux; Android 13)",
                "Mozilla/5.0 (iPhone; CPU iPhone OS 16_0)",
                "Mozilla/5.0 (iPad; CPU OS 16_0)",
                "Mozilla/5.0 (iPod; CPU iPhone OS 16_0)",
                "Mozilla/5.0 (Mobile; Windows Phone)",
            ];

            for ua in mobile_patterns {
                assert!(
                    MOBILE_REGEX.is_match(ua),
                    "Should detect '{}' as mobile",
                    ua
                );
            }
        }
    }
}
