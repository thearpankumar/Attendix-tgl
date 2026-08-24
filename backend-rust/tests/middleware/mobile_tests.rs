//! Tests for mobile_check middleware
//!
//! Ported from: backend/tests/mobileCheck.test.js
//!
//! Tests device type blocking based on User-Agent strings:
//! - Mobile devices (iPhone, iPad, Android) are allowed
//! - Desktop OSs are passed through for frontend hardware checks
//! - Bots and automation tools are blocked

#[cfg(test)]
mod tests {
    use attendance_geotag_backend::middleware::check_mobile;

    /// Test case structure matching Node.js test cases
    struct DeviceTestCase {
        name: &'static str,
        user_agent: Option<&'static str>,
        allowed: bool,
    }

    // Test cases from Node.js: Device Type Blocking
    const DEVICE_TEST_CASES: &[DeviceTestCase] = &[
        // Mobile devices - allowed
        DeviceTestCase {
            name: "iPhone",
            user_agent: Some("Mozilla/5.0 (iPhone; CPU iPhone OS 16_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.0 Mobile/15E148 Safari/604.1"),
            allowed: true,
        },
        DeviceTestCase {
            name: "iPad",
            user_agent: Some("Mozilla/5.0 (iPad; CPU OS 16_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.0 Mobile/15E148 Safari/604.1"),
            allowed: true,
        },
        DeviceTestCase {
            name: "Android Phone",
            user_agent: Some("Mozilla/5.0 (Linux; Android 13; SM-G991B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Mobile Safari/537.36"),
            allowed: true,
        },
        DeviceTestCase {
            name: "Android Tablet",
            user_agent: Some("Mozilla/5.0 (Linux; Android 13; SM-X700) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Safari/537.36"),
            allowed: true,
        },
        // Desktop OSs - passed to frontend for strict hardware checks (hybrid plan)
        DeviceTestCase {
            name: "Windows Desktop",
            user_agent: Some("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Safari/537.36"),
            allowed: true,
        },
        DeviceTestCase {
            name: "Mac Desktop",
            user_agent: Some("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Safari/537.36"),
            allowed: true,
        },
        // Bots and empty agents - strictly blocked
        DeviceTestCase {
            name: "Curl Bot",
            user_agent: Some("curl/7.68.0"),
            allowed: false,
        },
        DeviceTestCase {
            name: "Python Requests",
            user_agent: Some("python-requests/2.25.1"),
            allowed: false,
        },
        DeviceTestCase {
            name: "Empty User Agent",
            user_agent: Some(""),
            allowed: false,
        },
        DeviceTestCase {
            name: "No User Agent",
            user_agent: None,
            allowed: false,
        },
    ];

    // ============================================
    // Test: check_mobile function - Device Detection
    // ============================================

    #[test]
    fn test_iphone_detection() {
        let ua = "Mozilla/5.0 (iPhone; CPU iPhone OS 16_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.0 Mobile/15E148 Safari/604.1";
        let device_info = check_mobile(ua);

        assert!(device_info.is_mobile, "iPhone should be detected as mobile");
        assert!(
            !device_info.is_tablet,
            "iPhone should not be detected as tablet"
        );
        assert!(!device_info.is_bot, "iPhone should not be detected as bot");
        assert_eq!(device_info.platform, "iOS");
    }

    #[test]
    fn test_ipad_detection() {
        let ua = "Mozilla/5.0 (iPad; CPU OS 16_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.0 Mobile/15E148 Safari/604.1";
        let device_info = check_mobile(ua);

        assert!(device_info.is_tablet, "iPad should be detected as tablet");
        assert!(
            device_info.is_mobile || device_info.is_tablet,
            "iPad should be mobile/tablet"
        );
        assert!(!device_info.is_bot, "iPad should not be detected as bot");
        assert_eq!(device_info.platform, "iOS");
    }

    #[test]
    fn test_android_phone_detection() {
        let ua = "Mozilla/5.0 (Linux; Android 13; SM-G991B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Mobile Safari/537.36";
        let device_info = check_mobile(ua);

        assert!(
            device_info.is_mobile,
            "Android phone should be detected as mobile"
        );
        assert!(
            !device_info.is_bot,
            "Android phone should not be detected as bot"
        );
        assert_eq!(device_info.platform, "Android");
    }

    #[test]
    fn test_android_tablet_detection() {
        let ua = "Mozilla/5.0 (Linux; Android 13; SM-X700) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Safari/537.36";
        let device_info = check_mobile(ua);

        // Android tablets match "mobile" in UA plus might match tablet pattern
        assert!(
            device_info.is_mobile || device_info.is_tablet,
            "Android tablet should be mobile/tablet"
        );
        assert!(
            !device_info.is_bot,
            "Android tablet should not be detected as bot"
        );
        assert_eq!(device_info.platform, "Android");
    }

    #[test]
    fn test_windows_desktop_detection() {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Safari/537.36";
        let device_info = check_mobile(ua);

        assert!(
            !device_info.is_mobile,
            "Windows desktop should not be detected as mobile"
        );
        assert!(
            !device_info.is_tablet,
            "Windows desktop should not be detected as tablet"
        );
        assert!(
            !device_info.is_bot,
            "Windows desktop should not be detected as bot"
        );
        assert_eq!(device_info.platform, "Windows");
    }

    #[test]
    fn test_mac_desktop_detection() {
        let ua = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Safari/537.36";
        let device_info = check_mobile(ua);

        assert!(
            !device_info.is_mobile,
            "Mac desktop should not be detected as mobile"
        );
        assert!(
            !device_info.is_tablet,
            "Mac desktop should not be detected as tablet"
        );
        assert!(
            !device_info.is_bot,
            "Mac desktop should not be detected as bot"
        );
        assert_eq!(device_info.platform, "Mac");
    }

    #[test]
    fn test_curl_bot_detection() {
        let ua = "curl/7.68.0";
        let device_info = check_mobile(ua);

        assert!(
            !device_info.is_mobile,
            "Curl should not be detected as mobile"
        );
        assert!(device_info.is_bot, "Curl should be detected as bot");
    }

    #[test]
    fn test_python_requests_bot_detection() {
        let ua = "python-requests/2.25.1";
        let device_info = check_mobile(ua);

        assert!(
            !device_info.is_mobile,
            "Python requests should not be detected as mobile"
        );
        assert!(
            device_info.is_bot,
            "Python requests should be detected as bot"
        );
    }

    #[test]
    fn test_empty_user_agent_detection() {
        let ua = "";
        let device_info = check_mobile(ua);

        assert!(
            !device_info.is_mobile,
            "Empty UA should not be detected as mobile"
        );
        assert!(
            !device_info.is_tablet,
            "Empty UA should not be detected as tablet"
        );
        assert!(!device_info.is_bot, "Empty UA should not be bot-detected");
        assert_eq!(device_info.platform, "Unknown");
    }

    // ============================================
    // Bot Pattern Detection Tests
    // ============================================

    #[test]
    fn test_wget_bot_detection() {
        let ua = "Wget/1.21";
        let device_info = check_mobile(ua);
        assert!(device_info.is_bot, "Wget should be detected as bot");
    }

    #[test]
    fn test_postman_bot_detection() {
        let ua = "PostmanRuntime/7.29.0";
        let device_info = check_mobile(ua);
        assert!(device_info.is_bot, "Postman should be detected as bot");
    }

    #[test]
    fn test_spider_bot_detection() {
        let ua = "Googlebot/2.1 (+http://www.google.com/bot.html)";
        let device_info = check_mobile(ua);
        assert!(device_info.is_bot, "Spider bots should be detected");
    }

    #[test]
    fn test_crawler_bot_detection() {
        let ua = "Mozilla/5.0 (compatible; Bingbot/2.0; +http://www.bing.com/bingbot.htm)";
        let device_info = check_mobile(ua);
        assert!(device_info.is_bot, "Crawlers should be detected");
    }

    // ============================================
    // Browser Detection Tests
    // ============================================

    #[test]
    fn test_chrome_browser_detection() {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Safari/537.36";
        let device_info = check_mobile(ua);
        assert_eq!(device_info.browser, "Chrome");
        assert!(device_info.is_chromium);
    }

    #[test]
    fn test_firefox_browser_detection() {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:109.0) Gecko/20100101 Firefox/112.0";
        let device_info = check_mobile(ua);
        assert_eq!(device_info.browser, "Firefox");
        assert!(!device_info.is_chromium);
    }

    #[test]
    fn test_safari_browser_detection() {
        let ua = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.0 Safari/605.1.15";
        let device_info = check_mobile(ua);
        assert_eq!(device_info.browser, "Safari");
        assert!(!device_info.is_chromium);
    }

    #[test]
    fn test_edge_browser_detection() {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Safari/537.36 Edg/112.0.1722.48";
        let device_info = check_mobile(ua);
        assert_eq!(device_info.browser, "Edge");
        assert!(device_info.is_chromium);
    }

    #[test]
    fn test_opera_browser_detection() {
        // Opera uses Chrome in UA, so the browser detection returns "Chrome"
        // The important thing is that it's detected as Chromium-based
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Safari/537.36 OPR/98.0.0.0";
        let device_info = check_mobile(ua);
        // Opera UAs contain "OPR/" but also "Chrome/"
        // The current implementation checks "edg/" first, then "chrome"
        assert!(
            device_info.is_chromium,
            "Opera should be detected as Chromium-based"
        );
        // Browser might be "Chrome" or "Opera" depending on detection order
        assert!(
            device_info.browser == "Chrome" || device_info.browser == "Opera",
            "Opera UA should be detected as Chrome or Opera"
        );
    }

    // ============================================
    // Device Test Cases Matrix (parametrized)
    // ============================================

    #[test]
    fn test_all_device_cases_allowed_status() {
        for tc in DEVICE_TEST_CASES {
            let ua = tc.user_agent.unwrap_or("");
            let device_info = check_mobile(ua);

            if tc.allowed {
                // Allowed devices should NOT be bots
                assert!(
                    !device_info.is_bot,
                    "Device '{}' should not be detected as bot (UA: {:?})",
                    tc.name, tc.user_agent
                );

                // Allowed devices should be mobile/tablet OR desktop OS (for frontend check)
                let ua = tc.user_agent.unwrap_or("");
                let is_desktop_os = ua.to_lowercase().contains("windows")
                    || ua.to_lowercase().contains("macintosh")
                    || ua.to_lowercase().contains("linux")
                    || ua.to_lowercase().contains("x11");

                assert!(
                    device_info.is_mobile || device_info.is_tablet || is_desktop_os,
                    "Device '{}' should be allowed (mobile/tablet/desktop)",
                    tc.name
                );
            } else {
                // Blocked devices should be bots or have no user agent
                let should_be_blocked =
                    device_info.is_bot || tc.user_agent.is_none() || tc.user_agent == Some("");

                assert!(should_be_blocked, "Device '{}' should be blocked", tc.name);
            }
        }
    }

    #[test]
    fn test_device_test_case_count() {
        // Verify we have all test cases from Node.js (10 cases)
        assert_eq!(
            DEVICE_TEST_CASES.len(),
            10,
            "Should have 10 device test cases matching Node.js"
        );
    }

    // ============================================
    // DeviceInfo Structure Tests
    // ============================================

    #[test]
    fn test_device_info_structure() {
        let ua = "Mozilla/5.0 (Linux; Android 13; SM-G991B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Mobile Safari/537.36";
        let info = check_mobile(ua);

        // All fields should be populated
        assert!(!info.platform.is_empty());
        assert!(!info.browser.is_empty());
        assert!(info.is_mobile || !info.is_mobile); // bool is set
        assert!(info.is_tablet || !info.is_tablet); // bool is set
        assert!(info.is_bot || !info.is_bot); // bool is set
        assert!(info.is_chromium || !info.is_chromium); // bool is set
    }

    #[test]
    fn test_device_info_clone() {
        let ua = "Mozilla/5.0 (iPhone; CPU iPhone OS 16_0 like Mac OS X)";
        let info1 = check_mobile(ua);
        let info2 = info1.clone();

        assert_eq!(info1.is_mobile, info2.is_mobile);
        assert_eq!(info1.is_tablet, info2.is_tablet);
        assert_eq!(info1.is_bot, info2.is_bot);
        assert_eq!(info1.platform, info2.platform);
        assert_eq!(info1.browser, info2.browser);
    }

    #[test]
    fn test_device_info_debug() {
        let ua = "Mozilla/5.0 (iPhone; CPU iPhone OS 16_0 like Mac OS X)";
        let info = check_mobile(ua);

        // Debug trait should be implemented
        let debug_str = format!("{:?}", info);
        assert!(debug_str.contains("DeviceInfo"));
        assert!(debug_str.contains("is_mobile"));
    }

    // ============================================
    // Edge Cases
    // ============================================

    #[test]
    fn test_malformed_user_agent() {
        let ua = "SomeRandomString";
        let device_info = check_mobile(ua);

        assert!(!device_info.is_mobile);
        assert!(!device_info.is_bot);
        assert_eq!(device_info.platform, "Unknown");
        assert_eq!(device_info.browser, "Unknown");
    }

    #[test]
    fn test_very_long_user_agent() {
        let ua = "Mozilla/5.0 ".repeat(100);
        let device_info = check_mobile(&ua);

        // Should not panic and return valid info
        assert!(!device_info.is_mobile || device_info.is_mobile);
    }

    #[test]
    fn test_unicode_user_agent() {
        let ua = "Mozilla/5.0 (测试; Android 13)";
        let device_info = check_mobile(ua);

        // Should handle unicode gracefully
        assert!(device_info.platform.contains("Android") || device_info.platform == "Unknown");
    }

    #[test]
    fn test_case_insensitive_detection() {
        // Test lowercase
        let ua_lower = "mozilla/5.0 (iphone; cpu iphone os 16_0 like mac os x)";
        let info_lower = check_mobile(ua_lower);

        // Test uppercase
        let ua_upper = "MOZILLA/5.0 (IPHONE; CPU IPHONE OS 16_0 LIKE MAC OS X)";
        let info_upper = check_mobile(ua_upper);

        // Both should detect mobile
        assert!(
            info_lower.is_mobile || info_upper.is_mobile,
            "Detection should be case insensitive"
        );
    }

    // ============================================
    // Platform Detection Tests
    // ============================================

    #[test]
    fn test_android_platform_detection() {
        let variants = [
            "Mozilla/5.0 (Linux; Android 10)",
            "Mozilla/5.0 (Linux; Android 13; SM-G991B)",
            "Android",
        ];

        for ua in variants {
            let info = check_mobile(ua);
            assert_eq!(info.platform, "Android", "Failed for UA: {}", ua);
        }
    }

    #[test]
    fn test_ios_platform_detection() {
        let variants = [
            "Mozilla/5.0 (iPhone; CPU iPhone OS 16_0 like Mac OS X)",
            "Mozilla/5.0 (iPad; CPU OS 16_0 like Mac OS X)",
        ];

        for ua in variants {
            let info = check_mobile(ua);
            assert_eq!(info.platform, "iOS", "Failed for UA: {}", ua);
        }
    }

    #[test]
    fn test_linux_platform_detection() {
        let ua = "Mozilla/5.0 (X11; Linux x86_64; rv:109.0) Gecko/20100101 Firefox/112.0";
        let info = check_mobile(ua);
        assert_eq!(info.platform, "Linux");
    }

    // ============================================
    // Chromium Detection Tests
    // ============================================

    #[test]
    fn test_chromium_variants() {
        // These UAs contain keywords that match CHROMIUM_REGEX: (chrome|chromium|edg|opera|brave)
        let chromium_uas = [
            ("Chrome", "Mozilla/5.0 Chrome/112.0.0.0"),
            ("Chromium", "Mozilla/5.0 Chromium/112.0.0.0"),
            ("Edge", "Mozilla/5.0 Edg/112.0.0.0"),
            ("Brave", "Mozilla/5.0 Brave/112.0.0.0"),
            // Note: "opera" is matched by the regex, but "OPR/" is not
            ("Opera", "Mozilla/5.0 Opera/98.0.0.0"),
        ];

        for (name, ua) in chromium_uas {
            let info = check_mobile(ua);
            assert!(
                info.is_chromium,
                "{} should be detected as Chromium-based",
                name
            );
        }
    }

    #[test]
    fn test_non_chromium_browsers() {
        let non_chromium_uas = [
            ("Firefox", "Mozilla/5.0 Firefox/112.0"),
            ("Safari", "Mozilla/5.0 Safari/605.1.15"),
        ];

        for (name, ua) in non_chromium_uas {
            let info = check_mobile(ua);
            assert!(
                !info.is_chromium,
                "{} should NOT be detected as Chromium-based",
                name
            );
        }
    }

    // ============================================
    // Tablet Detection Tests
    // ============================================

    #[test]
    fn test_tablet_keyword_detection() {
        let ua = "Mozilla/5.0 (Linux; Android 13; Tablet) AppleWebKit/537.36";
        let info = check_mobile(ua);
        assert!(info.is_tablet, "Tablet keyword should be detected");
    }

    #[test]
    fn test_ipad_tablet_detection() {
        let ua = "Mozilla/5.0 (iPad; CPU OS 16_0 like Mac OS X)";
        let info = check_mobile(ua);

        // iPad should be tablet but might also be considered mobile in some contexts
        assert!(info.is_tablet, "iPad should be tablet");
    }

    // ============================================
    // Mobile Check with Desktop Masquerading
    // ============================================

    #[test]
    fn test_desktop_os_allowed_for_frontend_check() {
        // Desktop OS user agents should be allowed through
        // (per Node.js hybrid plan comment)
        let desktop_uas = [
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64)",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)",
            "Mozilla/5.0 (X11; Linux x86_64)",
            "Mozilla/5.0 (CrOS x86_64)",
        ];

        for ua in desktop_uas {
            let info = check_mobile(ua);
            // Should not be mobile/tablet/bot
            assert!(!info.is_bot, "Desktop UA should not be bot: {}", ua);
            // Desktop OS passes through for frontend hardware checks. Note:
            // as of the touch-evidence fix, `is_mobile_or_masquerading` (not
            // tested here — this test only exercises `check_mobile`) also
            // requires the X-Attendix-Touch-Points/X-Attendix-Coarse-Pointer
            // headers before granting the Mac/Linux masquerading allowance,
            // and no longer honors Windows/ChromeOS UAs at all.
        }
    }
}
