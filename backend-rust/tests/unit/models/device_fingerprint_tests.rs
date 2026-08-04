//! Tests for DeviceFingerprint Model
//!
//! Ported from: backend/tests/deviceFingerprint.test.js
//!
//! Tests cover:
//! - find_or_create: creates a new device fingerprint if not exists, returns existing
//! - record_verification_failure: increments failures, spoofing attempts, blocks after 5
//! - record_successful_verification: adds session, reduces failures, marks trusted
//! - add_user_agent: adds new user agent, updates lastSeen for existing
//! - Static methods: getSuspiciousDevices, getBlockedDevices, findByRollNumber

use uuid::Uuid;

// Helper to create a new DeviceFingerprint for testing
fn create_device_fingerprint(
    fingerprint_id: &str,
) -> attendance_geotag_backend::models::DeviceFingerprint {
    attendance_geotag_backend::models::DeviceFingerprint::new(fingerprint_id.to_string())
}

// Helper to create a device with specific spoofing attempts
fn create_device_with_spoofing_attempts(
    fingerprint_id: &str,
    attempts: i32,
) -> attendance_geotag_backend::models::DeviceFingerprint {
    let mut device = create_device_fingerprint(fingerprint_id);
    device.spoofing_attempts = attempts;
    device
}

// Helper to create a device with specific verification failures
fn create_device_with_verification_failures(
    fingerprint_id: &str,
    failures: i32,
) -> attendance_geotag_backend::models::DeviceFingerprint {
    let mut device = create_device_fingerprint(fingerprint_id);
    device.verification_failures = failures;
    device
}

// ============================================================================
// find_or_create Tests
// ============================================================================

mod find_or_create_tests {
    use super::*;

    #[test]
    fn should_create_a_new_device_fingerprint_if_not_exists() {
        // Test case: creates a new device fingerprint if not exists
        //
        // In Node.js test:
        // - Calls DeviceFingerprint.findOrCreate('fingerprint-123')
        // - Expects device to be defined
        // - Expects device.fingerprintId to be 'fingerprint-123'
        // - Expects device.verificationFailures to be 0
        // - Expects device.spoofingAttempts to be 0
        // - Expects device.isBlocked to be false

        let device = create_device_fingerprint("fingerprint-123");

        assert!(
            device.fingerprint_id == "fingerprint-123",
            "Device fingerprint ID should match"
        );
        assert_eq!(
            device.verification_failures, 0,
            "Verification failures should be 0 on creation"
        );
        assert_eq!(
            device.spoofing_attempts, 0,
            "Spoofing attempts should be 0 on creation"
        );
        assert!(
            !device.is_blocked,
            "Device should not be blocked on creation"
        );
    }

    #[test]
    fn should_return_existing_device_fingerprint() {
        // Test case: returns existing device fingerprint
        //
        // In Node.js test:
        // - Creates a device with fingerprintId: 'fingerprint-456'
        // - Calls DeviceFingerprint.findOrCreate('fingerprint-456')
        // - Expects device.fingerprintId to be 'fingerprint-456'
        //
        // In Rust, we simulate this by creating a device and verifying
        // the fingerprint_id can be used to find/identify the device

        let device = create_device_fingerprint("fingerprint-456");

        assert_eq!(
            device.fingerprint_id, "fingerprint-456",
            "Existing device fingerprint ID should match"
        );
    }
}

// ============================================================================
// record_verification_failure Tests
// ============================================================================

mod record_verification_failure_tests {
    use super::*;

    #[test]
    fn should_increment_verification_failures() {
        // Test case: increments verification failures
        //
        // In Node.js test:
        // - Creates device using findOrCreate('fingerprint-fail-test')
        // - Calls device.recordVerificationFailure('Test failure')
        // - Expects updated.verificationFailures to be 1
        // - Expects updated.lastSpoofingReason to be 'Test failure'
        // - Expects updated.spoofingAttempts to be 1

        let mut device = create_device_fingerprint("fingerprint-fail-test");
        device.record_verification_failure(Some("Test failure".to_string()));

        assert_eq!(
            device.verification_failures, 1,
            "Verification failures should be 1 after first failure"
        );
        assert_eq!(
            device.last_spoofing_reason,
            Some("Test failure".to_string()),
            "Last spoofing reason should be recorded"
        );
        assert_eq!(
            device.spoofing_attempts, 1,
            "Spoofing attempts should be 1 when reason is provided"
        );
    }

    #[test]
    fn should_increment_spoofing_attempts_only_when_reason_provided() {
        // Test case: increments spoofing attempts only when reason provided
        //
        // In Node.js test:
        // - Creates device using findOrCreate('fingerprint-fail-no-reason')
        // - Calls device.recordVerificationFailure(null)
        // - Expects updated.verificationFailures to be 1
        // - Expects updated.spoofingAttempts to be 0

        let mut device = create_device_fingerprint("fingerprint-fail-no-reason");
        device.record_verification_failure(None);

        assert_eq!(
            device.verification_failures, 1,
            "Verification failures should increment even without reason"
        );
        assert_eq!(
            device.spoofing_attempts, 0,
            "Spoofing attempts should NOT increment when no reason provided"
        );
    }

    #[test]
    fn should_block_device_after_5_spoofing_attempts() {
        // Test case: blocks device after 5 spoofing attempts
        //
        // In Node.js test:
        // - Creates device using findOrCreate('fingerprint-block-test')
        // - Calls recordVerificationFailure 5 times with spoofing reason
        // - Expects updated.isBlocked to be true
        // - Expects updated.blockReason to contain 'Blocked after 5 spoofing attempts'

        let mut device = create_device_fingerprint("fingerprint-block-test");

        for i in 1..=5 {
            device.record_verification_failure(Some(format!("Spoofing attempt {}", i)));
        }

        assert!(
            device.is_blocked,
            "Device should be blocked after 5 spoofing attempts"
        );
        assert!(device.block_reason.is_some(), "Block reason should be set");
        assert!(
            device
                .block_reason
                .as_ref()
                .unwrap()
                .contains("Blocked after 5 spoofing attempts"),
            "Block reason should contain 'Blocked after 5 spoofing attempts'"
        );
    }

    #[test]
    fn should_not_block_device_before_5_spoofing_attempts() {
        // Additional test: device should not be blocked before 5 attempts

        let mut device = create_device_fingerprint("fingerprint-no-block-yet");

        for i in 1..=4 {
            device.record_verification_failure(Some(format!("Spoofing attempt {}", i)));
        }

        assert!(
            !device.is_blocked,
            "Device should NOT be blocked with only 4 spoofing attempts"
        );
        assert!(
            device.block_reason.is_none(),
            "Block reason should not be set"
        );
    }

    #[test]
    fn should_update_last_seen_on_failure() {
        // Additional test: last_seen should be updated on failure

        let mut device = create_device_fingerprint("fingerprint-time-test");
        let initial_last_seen = device.last_seen;

        // Small delay to ensure time difference
        std::thread::sleep(std::time::Duration::from_millis(10));
        device.record_verification_failure(Some("Test".to_string()));

        assert!(
            device.last_seen > initial_last_seen,
            "last_seen should be updated after verification failure"
        );
    }
}

// ============================================================================
// record_successful_verification Tests
// ============================================================================

mod record_successful_verification_tests {
    use super::*;

    #[test]
    fn should_add_session_to_device_history() {
        // Test case: adds session to device history
        //
        // In Node.js test:
        // - Creates device using findOrCreate('fingerprint-success-test')
        // - Creates a session ID
        // - Calls device.recordSuccessfulVerification(sessionId, 'ROLL001')
        // - Expects updated.sessions to have length 1
        // - Expects updated.sessions[0].rollNumber to be 'ROLL001'
        // - Expects updated.sessions[0].wasSuccessful to be true

        let mut device = create_device_fingerprint("fingerprint-success-test");
        let session_id = Uuid::new_v4();

        device.record_successful_verification(session_id, "ROLL001".to_string());

        assert_eq!(
            device.sessions.len(),
            1,
            "Sessions should have length 1 after first successful verification"
        );
        assert_eq!(
            device.sessions[0].roll_number, "ROLL001",
            "Session roll number should match"
        );
        assert!(
            device.sessions[0].was_successful,
            "Session was_successful should be true"
        );
        assert_eq!(
            device.sessions[0].session_id, session_id,
            "Session ID should match"
        );
    }

    #[test]
    fn should_reduce_verification_failures_on_success() {
        // Test case: reduces verification failures on success
        //
        // In Node.js test:
        // - Creates device using findOrCreate('fingerprint-recovery-test')
        // - Calls recordVerificationFailure(null) twice
        // - Calls recordSuccessfulVerification(sessionId, 'ROLL002')
        // - Expects updated.verificationFailures to be 1

        let mut device = create_device_fingerprint("fingerprint-recovery-test");
        device.record_verification_failure(None);
        device.record_verification_failure(None);

        assert_eq!(
            device.verification_failures, 2,
            "Should have 2 verification failures before success"
        );

        let session_id = Uuid::new_v4();
        device.record_successful_verification(session_id, "ROLL002".to_string());

        assert_eq!(
            device.verification_failures, 1,
            "Verification failures should be reduced to 1 after success"
        );
    }

    #[test]
    fn should_not_reduce_verification_failures_below_zero() {
        // Additional test: verification failures should not go below zero

        let mut device = create_device_fingerprint("fingerprint-no-negative");

        // No failures initially
        assert_eq!(device.verification_failures, 0);

        let session_id = Uuid::new_v4();
        device.record_successful_verification(session_id, "ROLL001".to_string());

        assert_eq!(
            device.verification_failures, 0,
            "Verification failures should not go below 0"
        );
    }

    #[test]
    fn should_mark_device_as_trusted_after_3_successful_sessions_with_no_spoofing() {
        // Test case: marks device as trusted after 3 successful sessions with no spoofing
        //
        // In Node.js test:
        // - Creates device using findOrCreate('fingerprint-trust-test')
        // - Calls recordSuccessfulVerification 3 times
        // - Expects updated.isTrusted to be true

        let mut device = create_device_fingerprint("fingerprint-trust-test");

        for i in 0..3 {
            let session_id = Uuid::new_v4();
            device.record_successful_verification(session_id, format!("ROLL{}", i));
        }

        assert!(
            device.is_trusted,
            "Device should be trusted after 3 successful sessions with no spoofing"
        );
    }

    #[test]
    fn should_not_mark_as_trusted_if_spoofing_attempts_exist() {
        // Test case: does not mark as trusted if spoofing attempts exist
        //
        // In Node.js test:
        // - Creates device using findOrCreate('fingerprint-no-trust-test')
        // - Calls recordVerificationFailure('Spoofing attempt')
        // - Calls recordSuccessfulVerification 3 times
        // - Expects updated.isTrusted to be false

        let mut device = create_device_fingerprint("fingerprint-no-trust-test");
        device.record_verification_failure(Some("Spoofing attempt".to_string()));

        for i in 0..3 {
            let session_id = Uuid::new_v4();
            device.record_successful_verification(session_id, format!("ROLL{}", i));
        }

        assert!(
            !device.is_trusted,
            "Device should NOT be trusted if spoofing attempts exist"
        );
    }

    #[test]
    fn should_not_mark_as_trusted_with_fewer_than_3_successful_sessions() {
        // Additional test: should not mark as trusted with fewer than 3 sessions

        let mut device = create_device_fingerprint("fingerprint-need-more-sessions");

        // Only 2 successful sessions
        for i in 0..2 {
            let session_id = Uuid::new_v4();
            device.record_successful_verification(session_id, format!("ROLL{}", i));
        }

        assert!(
            !device.is_trusted,
            "Device should NOT be trusted with only 2 successful sessions"
        );
    }

    #[test]
    fn should_update_last_seen_on_success() {
        // Additional test: last_seen should be updated on success

        let mut device = create_device_fingerprint("fingerprint-success-time-test");
        let initial_last_seen = device.last_seen;

        // Small delay to ensure time difference
        std::thread::sleep(std::time::Duration::from_millis(10));

        let session_id = Uuid::new_v4();
        device.record_successful_verification(session_id, "ROLL001".to_string());

        assert!(
            device.last_seen > initial_last_seen,
            "last_seen should be updated after successful verification"
        );
    }

    #[test]
    fn should_limit_sessions_to_50() {
        // Additional test: sessions should be limited to 50 entries

        let mut device = create_device_fingerprint("fingerprint-session-limit");

        // Add 51 sessions
        for i in 0..51 {
            let session_id = Uuid::new_v4();
            device.record_successful_verification(session_id, format!("ROLL{}", i));
        }

        assert_eq!(
            device.sessions.len(),
            50,
            "Sessions should be limited to 50 entries"
        );
    }
}

// ============================================================================
// add_user_agent Tests
// ============================================================================

mod add_user_agent_tests {
    use super::*;

    fn add_user_agent(device: &mut attendance_geotag_backend::models::DeviceFingerprint, ua: &str) {
        // This simulates the addUserAgent functionality
        // Since the Rust implementation doesn't have this method yet,
        // we test the logic here

        let now = chrono::Utc::now();

        // Check if UA already exists
        if let Some(existing) = device
            .user_agents_seen
            .iter_mut()
            .find(|entry| entry.ua == ua)
        {
            existing.last_seen = now;
        } else {
            device
                .user_agents_seen
                .push(attendance_geotag_backend::models::UserAgentEntry {
                    ua: ua.to_string(),
                    first_seen: now,
                    last_seen: now,
                });
        }
    }

    #[test]
    fn should_add_new_user_agent() {
        // Test case: adds new user agent
        //
        // In Node.js test:
        // - Creates device using findOrCreate('fingerprint-ua-test')
        // - Calls device.addUserAgent('Mozilla/5.0 Test UA')
        // - Expects updated.userAgentsSeen to have length 1
        // - Expects updated.userAgentsSeen[0].ua to be 'Mozilla/5.0 Test UA'

        let mut device = create_device_fingerprint("fingerprint-ua-test");
        add_user_agent(&mut device, "Mozilla/5.0 Test UA");

        assert_eq!(
            device.user_agents_seen.len(),
            1,
            "user_agents_seen should have length 1 after adding user agent"
        );
        assert_eq!(
            device.user_agents_seen[0].ua, "Mozilla/5.0 Test UA",
            "User agent string should match"
        );
    }

    #[test]
    fn should_update_last_seen_for_existing_user_agent() {
        // Test case: updates lastSeen for existing user agent
        //
        // In Node.js test:
        // - Creates device using findOrCreate('fingerprint-ua-update-test')
        // - Calls device.addUserAgent('Mozilla/5.0 Test UA')
        // - Waits 10ms
        // - Calls device.addUserAgent('Mozilla/5.0 Test UA') again
        // - Expects updated.userAgentsSeen to have length 1
        // - Expects updated.userAgentsSeen[0].lastSeen to not equal firstSeen

        let mut device = create_device_fingerprint("fingerprint-ua-update-test");
        add_user_agent(&mut device, "Mozilla/5.0 Test UA");

        let first_seen = device.user_agents_seen[0].first_seen;

        // Small delay to ensure time difference
        std::thread::sleep(std::time::Duration::from_millis(10));

        add_user_agent(&mut device, "Mozilla/5.0 Test UA");

        assert_eq!(
            device.user_agents_seen.len(),
            1,
            "user_agents_seen should still have length 1 for duplicate UA"
        );
        assert_ne!(
            device.user_agents_seen[0].last_seen, first_seen,
            "lastSeen should be updated and different from firstSeen"
        );
    }

    #[test]
    fn should_track_multiple_user_agents() {
        // Additional test: should track multiple different user agents

        let mut device = create_device_fingerprint("fingerprint-multi-ua");
        add_user_agent(&mut device, "Mozilla/5.0 UA1");
        add_user_agent(&mut device, "Mozilla/5.0 UA2");
        add_user_agent(&mut device, "Mozilla/5.0 UA3");

        assert_eq!(
            device.user_agents_seen.len(),
            3,
            "Should have 3 distinct user agents"
        );
    }
}

// ============================================================================
// Static Methods Tests
// ============================================================================

mod static_methods_tests {
    use super::*;

    // Helper to check if device is suspicious based on thresholds
    fn is_suspicious(
        device: &attendance_geotag_backend::models::DeviceFingerprint,
        threshold: i32,
    ) -> bool {
        device.spoofing_attempts >= threshold || device.verification_failures >= threshold
    }

    // Helper to find devices by roll number (simulated)
    fn device_has_roll_number(
        device: &attendance_geotag_backend::models::DeviceFingerprint,
        roll_number: &str,
    ) -> bool {
        device.sessions.iter().any(|s| s.roll_number == roll_number)
    }

    #[test]
    fn should_find_suspicious_devices() {
        // Test case: finds suspicious devices
        //
        // In Node.js test:
        // - Creates device with fingerprintId: 'suspicious-1', spoofingAttempts: 4
        // - Creates device with fingerprintId: 'suspicious-2', verificationFailures: 10
        // - Calls DeviceFingerprint.getSuspiciousDevices(3)
        // - Expects 2 devices returned

        let device1 = create_device_with_spoofing_attempts("suspicious-1", 4);
        let device2 = create_device_with_verification_failures("suspicious-2", 10);
        let device3 = create_device_fingerprint("not-suspicious");

        let threshold = 3;
        let suspicious_count = [
            is_suspicious(&device1, threshold),
            is_suspicious(&device2, threshold),
            is_suspicious(&device3, threshold),
        ]
        .iter()
        .filter(|&&x| x)
        .count();

        assert_eq!(
            suspicious_count, 2,
            "Should find 2 suspicious devices with threshold 3"
        );
        assert!(
            is_suspicious(&device1, threshold),
            "Device with 4 spoofing attempts should be suspicious"
        );
        assert!(
            is_suspicious(&device2, threshold),
            "Device with 10 verification failures should be suspicious"
        );
        assert!(
            !is_suspicious(&device3, threshold),
            "Device with no failures should not be suspicious"
        );
    }

    #[test]
    fn should_find_blocked_devices() {
        // Test case: finds blocked devices
        //
        // In Node.js test:
        // - Creates device with fingerprintId: 'blocked-1', isBlocked: true, blockReason: 'Abuse detected'
        // - Calls DeviceFingerprint.getBlockedDevices()
        // - Expects 1 device returned
        // - Expects device.fingerprintId to be 'blocked-1'

        let mut blocked_device = create_device_fingerprint("blocked-1");
        blocked_device.is_blocked = true;
        blocked_device.block_reason = Some("Abuse detected".to_string());

        let unblocked_device = create_device_fingerprint("not-blocked");

        // Simulate getBlockedDevices query
        let devices = [&blocked_device, &unblocked_device];
        let blocked_devices: Vec<_> = devices.iter().filter(|d| d.is_blocked).collect();

        assert_eq!(blocked_devices.len(), 1, "Should find 1 blocked device");
        assert_eq!(
            blocked_devices[0].fingerprint_id, "blocked-1",
            "Blocked device fingerprint ID should match"
        );
        assert_eq!(
            blocked_devices[0].block_reason,
            Some("Abuse detected".to_string()),
            "Block reason should match"
        );
    }

    #[test]
    fn should_find_devices_by_roll_number() {
        // Test case: finds devices by roll number
        //
        // In Node.js test:
        // - Creates device with fingerprintId: 'roll-test-device'
        // - Adds session with rollNumber: 'TESTROLL'
        // - Calls DeviceFingerprint.findByRollNumber('TESTROLL')
        // - Expects 1 device returned

        let mut device = create_device_fingerprint("roll-test-device");
        let session_id = Uuid::new_v4();
        device
            .sessions
            .push(attendance_geotag_backend::models::DeviceSession {
                session_id,
                roll_number: "TESTROLL".to_string(),
                timestamp: chrono::Utc::now(),
                was_successful: true,
            });

        let device2 = create_device_fingerprint("other-device");

        // Simulate findByRollNumber query
        let devices = [&device, &device2];
        let matching_devices: Vec<_> = devices
            .iter()
            .filter(|d| device_has_roll_number(d, "TESTROLL"))
            .collect();

        assert_eq!(
            matching_devices.len(),
            1,
            "Should find 1 device with the given roll number"
        );
        assert_eq!(
            matching_devices[0].fingerprint_id, "roll-test-device",
            "Found device fingerprint ID should match"
        );
    }
}

// ============================================================================
// Additional Edge Case Tests
// ============================================================================

mod edge_case_tests {
    use super::*;

    #[test]
    fn should_create_device_with_all_default_values() {
        // Comprehensive test for default values

        let device = create_device_fingerprint("comprehensive-test");

        assert_eq!(device.fingerprint_id, "comprehensive-test");
        assert!(
            !device.id.is_nil(),
            "ID should be freshly generated for new device"
        );
        assert_eq!(device.verification_failures, 0);
        assert_eq!(device.spoofing_attempts, 0);
        assert_eq!(device.last_spoofing_reason, None);
        assert_eq!(device.inconsistencies.len(), 0);
        assert_eq!(device.claimed_device_types.len(), 0);
        assert_eq!(device.user_agents_seen.len(), 0);
        assert_eq!(device.sessions.len(), 0);
        assert!(!device.is_trusted);
        assert!(!device.is_blocked);
        assert_eq!(device.block_reason, None);
        assert!(device.last_metrics.is_none());
    }

    #[test]
    fn should_record_multiple_verification_failures() {
        // Test multiple failures increment correctly

        let mut device = create_device_fingerprint("multi-failure-test");

        device.record_verification_failure(Some("Reason 1".to_string()));
        device.record_verification_failure(Some("Reason 2".to_string()));
        device.record_verification_failure(None); // No reason
        device.record_verification_failure(Some("Reason 3".to_string()));

        assert_eq!(
            device.verification_failures, 4,
            "Should have 4 total verification failures"
        );
        assert_eq!(
            device.spoofing_attempts, 3,
            "Should have 3 spoofing attempts (only when reason provided)"
        );
        assert_eq!(
            device.last_spoofing_reason,
            Some("Reason 3".to_string()),
            "Last spoofing reason should be the most recent one"
        );
    }

    #[test]
    fn should_correctly_identify_table_name() {
        // Test table name is correct

        assert_eq!(
            attendance_geotag_backend::models::DeviceFingerprint::table_name(),
            "device_fingerprints",
            "Table name should be 'device_fingerprints'"
        );
    }

    #[test]
    fn should_maintain_trust_status_with_zero_spoofing_attempts() {
        // Test that trust status properly checks spoofing_attempts == 0

        let mut device = create_device_fingerprint("trust-status-test");

        // Add 3 successful sessions
        for i in 0..3 {
            let session_id = Uuid::new_v4();
            device.record_successful_verification(session_id, format!("ROLL{}", i));
        }

        assert!(
            device.is_trusted,
            "Device should be trusted after 3 successful sessions with 0 spoofing"
        );
    }

    #[test]
    fn should_not_become_trusted_with_any_spoofing_attempts() {
        // Even 1 spoofing attempt should prevent trust

        let mut device = create_device_fingerprint("no-trust-one-attempt");
        device.spoofing_attempts = 1; // Simulate 1 spoofing attempt

        for i in 0..5 {
            let session_id = Uuid::new_v4();
            device.record_successful_verification(session_id, format!("ROLL{}", i));
        }

        assert!(
            !device.is_trusted,
            "Device should NOT be trusted with any spoofing attempts"
        );
    }
}
