//! Tests for validator and auth middleware
//!
//! Ported from: backend/tests/middleware.test.js
//!
//! Tests cover:
//! - validateAdmin: Admin registration validation
//! - validateLogin: Admin login validation
//! - validateLocation: Location creation validation
//! - validateSession: Session creation validation
//! - validateAttendance: Attendance submission validation
//! - protect (auth): JWT authentication middleware

#[cfg(test)]
mod tests {
    use attendance_geotag_backend::middleware::validators::{
        is_alphanumeric, is_valid_email, is_valid_objectid, AdminLoginRequest,
        AdminRegisterRequest, AttendanceSubmitRequest, LocationCreateRequest, SessionCreateRequest,
    };
    use attendance_geotag_backend::middleware::{generate_token, verify_token, Claims};

    // Test constants matching Node.js setup
    const JWT_SECRET: &str = "test-secret-key-for-testing";
    const JWT_EXPIRE: &str = "7d";

    // ============================================
    // Helper Functions
    // ============================================

    /// Create a valid UUID string for testing
    fn create_valid_object_id() -> String {
        uuid::Uuid::new_v4().to_string()
    }

    // ============================================
    // Validator Middleware Tests - validateAdmin
    // ============================================
    mod validate_admin_tests {
        use super::*;

        /// Test: should pass valid admin data
        /// Node.js: lines 67-78
        #[test]
        fn should_pass_valid_admin_data() {
            let mut req = AdminRegisterRequest {
                username: "testadmin".to_string(),
                email: "admin@test.com".to_string(),
                password: "password123".to_string(),
                admin_secret: "secret".to_string(),
            };

            let result = req.validate_and_normalize();
            assert!(result.is_ok(), "Valid admin data should pass validation");
        }

        /// Test: should reject short username (< 3 chars)
        /// Node.js: lines 80-91
        #[test]
        fn should_reject_short_username() {
            let mut req = AdminRegisterRequest {
                username: "ab".to_string(),
                email: "admin@test.com".to_string(),
                password: "password123".to_string(),
                admin_secret: "secret".to_string(),
            };

            let result = req.validate_and_normalize();
            assert!(result.is_err(), "Username < 3 chars should be rejected");

            if let Err(err) = result {
                assert!(
                    err.message.contains("Validation failed"),
                    "Error message should contain 'Validation failed'"
                );
            }
        }

        /// Test: should reject invalid email
        /// Node.js: lines 93-103
        #[test]
        fn should_reject_invalid_email() {
            let mut req = AdminRegisterRequest {
                username: "testadmin".to_string(),
                email: "notanemail".to_string(),
                password: "password123".to_string(),
                admin_secret: "secret".to_string(),
            };

            let result = req.validate_and_normalize();
            assert!(result.is_err(), "Invalid email should be rejected");
        }

        /// Test: should reject short password (< 6 chars)
        /// Node.js: lines 105-115
        #[test]
        fn should_reject_short_password() {
            let mut req = AdminRegisterRequest {
                username: "testadmin".to_string(),
                email: "admin@test.com".to_string(),
                password: "12345".to_string(),
                admin_secret: "secret".to_string(),
            };

            let result = req.validate_and_normalize();
            assert!(result.is_err(), "Password < 6 chars should be rejected");
        }
    }

    // ============================================
    // Validator Middleware Tests - validateLogin
    // ============================================
    mod validate_login_tests {
        use super::*;

        /// Test: should pass valid login data
        /// Node.js: lines 119-128
        #[test]
        fn should_pass_valid_login_data() {
            let req = AdminLoginRequest {
                username: "testadmin".to_string(),
                password: "password123".to_string(),
            };

            let result = req.validate_request();
            assert!(result.is_ok(), "Valid login data should pass validation");
        }

        /// Test: should reject missing username
        /// Node.js: lines 130-138
        #[test]
        fn should_reject_missing_username() {
            let req = AdminLoginRequest {
                username: "".to_string(),
                password: "password123".to_string(),
            };

            let result = req.validate_request();
            assert!(result.is_err(), "Missing username should be rejected");
        }

        /// Test: should reject missing password
        /// Node.js: lines 140-148
        #[test]
        fn should_reject_missing_password() {
            let req = AdminLoginRequest {
                username: "testadmin".to_string(),
                password: "".to_string(),
            };

            let result = req.validate_request();
            assert!(result.is_err(), "Missing password should be rejected");
        }

        /// Test: should reject whitespace-only username
        /// Additional edge case
        #[test]
        fn should_reject_whitespace_only_username() {
            let req = AdminLoginRequest {
                username: "   ".to_string(),
                password: "password123".to_string(),
            };

            let result = req.validate_request();
            assert!(
                result.is_err(),
                "Whitespace-only username should be rejected"
            );
        }
    }

    // ============================================
    // Validator Middleware Tests - validateLocation
    // ============================================
    mod validate_location_tests {
        use super::*;

        /// Test: should pass valid location data
        /// Node.js: lines 152-163
        #[test]
        fn should_pass_valid_location_data() {
            let mut req = LocationCreateRequest {
                name: "Test Location".to_string(),
                latitude: 12.971,
                longitude: 77.594,
                radius_meters: Some(100),
            };

            let result = req.validate_and_sanitize();
            assert!(result.is_ok(), "Valid location data should pass validation");
        }

        /// Test: should reject latitude > 90
        /// Node.js: lines 165-175
        #[test]
        fn should_reject_latitude_above_90() {
            let mut req = LocationCreateRequest {
                name: "Test".to_string(),
                latitude: 91.0,
                longitude: 77.594,
                radius_meters: None,
            };

            let result = req.validate_and_sanitize();
            assert!(result.is_err(), "Latitude > 90 should be rejected");
        }

        /// Test: should reject radius < 10
        /// Node.js: lines 177-188
        #[test]
        fn should_reject_radius_below_10() {
            let mut req = LocationCreateRequest {
                name: "Test".to_string(),
                latitude: 12.971,
                longitude: 77.594,
                radius_meters: Some(5),
            };

            let result = req.validate_and_sanitize();
            assert!(result.is_err(), "Radius < 10 should be rejected");
        }

        /// Test: should accept latitude at boundaries
        /// Node.js: lines 190-209
        #[test]
        fn should_accept_latitude_at_boundaries() {
            // North Pole (latitude: 90)
            let mut req1 = LocationCreateRequest {
                name: "North Pole".to_string(),
                latitude: 90.0,
                longitude: 0.0,
                radius_meters: None,
            };
            let result1 = req1.validate_and_sanitize();
            assert!(
                result1.is_ok(),
                "Latitude 90 (North Pole) should be accepted"
            );

            // South Pole (latitude: -90)
            let mut req2 = LocationCreateRequest {
                name: "South Pole".to_string(),
                latitude: -90.0,
                longitude: 0.0,
                radius_meters: None,
            };
            let result2 = req2.validate_and_sanitize();
            assert!(
                result2.is_ok(),
                "Latitude -90 (South Pole) should be accepted"
            );
        }

        /// Test: should reject latitude < -90
        #[test]
        fn should_reject_latitude_below_negative_90() {
            let mut req = LocationCreateRequest {
                name: "Test".to_string(),
                latitude: -91.0,
                longitude: 77.594,
                radius_meters: None,
            };

            let result = req.validate_and_sanitize();
            assert!(result.is_err(), "Latitude < -90 should be rejected");
        }

        /// Test: should reject longitude > 180
        #[test]
        fn should_reject_longitude_above_180() {
            let mut req = LocationCreateRequest {
                name: "Test".to_string(),
                latitude: 12.971,
                longitude: 181.0,
                radius_meters: None,
            };

            let result = req.validate_and_sanitize();
            assert!(result.is_err(), "Longitude > 180 should be rejected");
        }

        /// Test: should accept longitude at boundaries
        #[test]
        fn should_accept_longitude_at_boundaries() {
            // Longitude: 180
            let mut req1 = LocationCreateRequest {
                name: "Date Line East".to_string(),
                latitude: 0.0,
                longitude: 180.0,
                radius_meters: None,
            };
            let result1 = req1.validate_and_sanitize();
            assert!(result1.is_ok(), "Longitude 180 should be accepted");

            // Longitude: -180
            let mut req2 = LocationCreateRequest {
                name: "Date Line West".to_string(),
                latitude: 0.0,
                longitude: -180.0,
                radius_meters: None,
            };
            let result2 = req2.validate_and_sanitize();
            assert!(result2.is_ok(), "Longitude -180 should be accepted");
        }

        /// Test: should reject radius > 10000
        #[test]
        fn should_reject_radius_above_10000() {
            let mut req = LocationCreateRequest {
                name: "Test".to_string(),
                latitude: 12.971,
                longitude: 77.594,
                radius_meters: Some(15000),
            };

            let result = req.validate_and_sanitize();
            assert!(result.is_err(), "Radius > 10000 should be rejected");
        }
    }

    // ============================================
    // Validator Middleware Tests - validateSession
    // ============================================
    mod validate_session_tests {
        use super::*;

        /// Test: should pass valid session data
        /// Node.js: lines 213-221
        #[test]
        fn should_pass_valid_session_data() {
            let req = SessionCreateRequest {
                location_id: Some(create_valid_object_id()),
                duration_minutes: None,
                batch_id: Some(create_valid_object_id()),
                assigned_admin_ids: vec![create_valid_object_id()],
                college_name: Some("Test College".to_string()),
                starts_at: Some("2026-08-14T09:00:00Z".to_string()),
                description: None,
            };

            let result = req.validate_with_objectids();
            assert!(result.is_ok(), "Valid session data should pass validation");
        }

        /// Test: should reject invalid UUID
        /// Node.js: lines 223-231
        ///
        /// Location is only validated for a normal session — an exam session
        /// (assigned_admin_ids present) doesn't use a location at all, so
        /// this must NOT be an exam session for the invalid location_id to
        /// actually be checked.
        #[test]
        fn should_reject_invalid_uuid() {
            let req = SessionCreateRequest {
                location_id: Some("invalid-id".to_string()),
                duration_minutes: None,
                batch_id: None,
                assigned_admin_ids: vec![],
                college_name: None,
                starts_at: None,
                description: None,
            };

            let result = req.validate_with_objectids();
            assert!(
                result.is_err(),
                "Invalid location UUID should be rejected on a normal session"
            );
        }

        /// Test: should reject duration < 5 minutes
        /// Node.js: lines 233-242
        #[test]
        fn should_reject_duration_below_5_minutes() {
            let req = SessionCreateRequest {
                location_id: Some(create_valid_object_id()),
                duration_minutes: Some(4),
                batch_id: Some(create_valid_object_id()),
                assigned_admin_ids: vec![create_valid_object_id()],
                college_name: Some("Test College".to_string()),
                starts_at: Some("2026-08-14T09:00:00Z".to_string()),
                description: None,
            };

            let result = req.validate_with_objectids();
            assert!(result.is_err(), "Duration < 5 minutes should be rejected");
        }

        /// Test: should reject duration > 480 minutes
        #[test]
        fn should_reject_duration_above_480_minutes() {
            let req = SessionCreateRequest {
                location_id: Some(create_valid_object_id()),
                duration_minutes: Some(500),
                batch_id: Some(create_valid_object_id()),
                assigned_admin_ids: vec![create_valid_object_id()],
                college_name: Some("Test College".to_string()),
                starts_at: Some("2026-08-14T09:00:00Z".to_string()),
                description: None,
            };

            let result = req.validate_with_objectids();
            assert!(result.is_err(), "Duration > 480 minutes should be rejected");
        }

        /// Test: should accept valid batch_id
        #[test]
        fn should_accept_valid_batch_id() {
            let req = SessionCreateRequest {
                location_id: Some(create_valid_object_id()),
                duration_minutes: Some(30),
                batch_id: Some(create_valid_object_id()),
                assigned_admin_ids: vec![create_valid_object_id()],
                college_name: Some("Test College".to_string()),
                starts_at: Some("2026-08-14T09:00:00Z".to_string()),
                description: None,
            };

            let result = req.validate_with_objectids();
            assert!(result.is_ok(), "Valid batch_id should be accepted");
        }

        /// Test: should reject invalid batch_id
        #[test]
        fn should_reject_invalid_batch_id() {
            let req = SessionCreateRequest {
                location_id: Some(create_valid_object_id()),
                duration_minutes: Some(30),
                batch_id: Some("invalid-batch-id".to_string()),
                assigned_admin_ids: vec![create_valid_object_id()],
                college_name: Some("Test College".to_string()),
                starts_at: Some("2026-08-14T09:00:00Z".to_string()),
                description: None,
            };

            let result = req.validate_with_objectids();
            assert!(result.is_err(), "Invalid batch_id should be rejected");
        }
    }

    // ============================================
    // Validator Middleware Tests - validateAttendance
    // ============================================
    mod validate_attendance_tests {
        use super::*;

        /// Test: should pass valid attendance with photo
        /// Node.js: lines 246-258
        #[test]
        fn should_pass_valid_attendance_with_photo() {
            let req = AttendanceSubmitRequest {
                student_name: "John Doe".to_string(),
                roll_number: "21CS101".to_string(),
                photo: Some("data:image/jpeg;base64,test".to_string()),
                latitude: 12.971,
                longitude: 77.594,
                direct_upload: None,
                public_id: None,
                face_detected: None,
                captcha_answer: None,
                captcha_id: None,
                device_fingerprint: None,
                user_agent: None,
                gps_data: None,
            };

            let result = req.validate();
            assert!(
                result.is_ok(),
                "Valid attendance with photo should pass validation"
            );
        }

        /// Test: should pass valid attendance with direct upload
        /// Node.js: lines 260-273
        #[test]
        fn should_pass_valid_attendance_with_direct_upload() {
            let req = AttendanceSubmitRequest {
                student_name: "John Doe".to_string(),
                roll_number: "21CS101".to_string(),
                photo: None,
                latitude: 12.971,
                longitude: 77.594,
                direct_upload: Some(true),
                public_id: Some("attendance-photos/test".to_string()),
                face_detected: None,
                captcha_answer: None,
                captcha_id: None,
                device_fingerprint: None,
                user_agent: None,
                gps_data: None,
            };

            let result = req.validate();
            assert!(
                result.is_ok(),
                "Valid attendance with direct upload should pass validation"
            );
        }

        /// Test: should reject invalid photo format
        /// Node.js: lines 275-287
        #[test]
        fn should_reject_invalid_photo_format() {
            let req = AttendanceSubmitRequest {
                student_name: "John Doe".to_string(),
                roll_number: "21CS101".to_string(),
                photo: Some("not-a-valid-photo".to_string()),
                latitude: 12.971,
                longitude: 77.594,
                direct_upload: None,
                public_id: None,
                face_detected: None,
                captcha_answer: None,
                captcha_id: None,
                device_fingerprint: None,
                user_agent: None,
                gps_data: None,
            };

            let result = req.validate();
            assert!(result.is_err(), "Invalid photo format should be rejected");
        }

        /// Test: should reject invalid roll number format
        /// Node.js: lines 289-301
        #[test]
        fn should_reject_invalid_roll_number_format() {
            let req = AttendanceSubmitRequest {
                student_name: "John Doe".to_string(),
                roll_number: "21-CS-101".to_string(), // Contains dashes
                photo: Some("data:image/jpeg;base64,test".to_string()),
                latitude: 12.971,
                longitude: 77.594,
                direct_upload: None,
                public_id: None,
                face_detected: None,
                captcha_answer: None,
                captcha_id: None,
                device_fingerprint: None,
                user_agent: None,
                gps_data: None,
            };

            let result = req.validate();
            assert!(
                result.is_err(),
                "Invalid roll number format should be rejected"
            );
        }

        /// Test: should reject missing photo and direct_upload
        #[test]
        fn should_reject_missing_photo_without_direct_upload() {
            let req = AttendanceSubmitRequest {
                student_name: "John Doe".to_string(),
                roll_number: "21CS101".to_string(),
                photo: None,
                latitude: 12.971,
                longitude: 77.594,
                direct_upload: None,
                public_id: None,
                face_detected: None,
                captcha_answer: None,
                captcha_id: None,
                device_fingerprint: None,
                user_agent: None,
                gps_data: None,
            };

            let result = req.validate();
            assert!(
                result.is_err(),
                "Missing photo without direct_upload should be rejected"
            );
        }

        /// Test: should reject direct_upload without public_id
        #[test]
        fn should_reject_direct_upload_without_public_id() {
            let req = AttendanceSubmitRequest {
                student_name: "John Doe".to_string(),
                roll_number: "21CS101".to_string(),
                photo: None,
                latitude: 12.971,
                longitude: 77.594,
                direct_upload: Some(true),
                public_id: None,
                face_detected: None,
                captcha_answer: None,
                captcha_id: None,
                device_fingerprint: None,
                user_agent: None,
                gps_data: None,
            };

            let result = req.validate();
            assert!(
                result.is_err(),
                "Direct upload without public_id should be rejected"
            );
        }

        /// Test: should reject invalid latitude
        #[test]
        fn should_reject_invalid_latitude() {
            let req = AttendanceSubmitRequest {
                student_name: "John Doe".to_string(),
                roll_number: "21CS101".to_string(),
                photo: Some("data:image/jpeg;base64,test".to_string()),
                latitude: 100.0, // Invalid
                longitude: 77.594,
                direct_upload: None,
                public_id: None,
                face_detected: None,
                captcha_answer: None,
                captcha_id: None,
                device_fingerprint: None,
                user_agent: None,
                gps_data: None,
            };

            let result = req.validate();
            assert!(result.is_err(), "Invalid latitude should be rejected");
        }

        /// Test: should reject invalid longitude
        #[test]
        fn should_reject_invalid_longitude() {
            let req = AttendanceSubmitRequest {
                student_name: "John Doe".to_string(),
                roll_number: "21CS101".to_string(),
                photo: Some("data:image/jpeg;base64,test".to_string()),
                latitude: 12.971,
                longitude: 200.0, // Invalid
                direct_upload: None,
                public_id: None,
                face_detected: None,
                captcha_answer: None,
                captcha_id: None,
                device_fingerprint: None,
                user_agent: None,
                gps_data: None,
            };

            let result = req.validate();
            assert!(result.is_err(), "Invalid longitude should be rejected");
        }

        /// Test: should reject short student name
        #[test]
        fn should_reject_short_student_name() {
            let req = AttendanceSubmitRequest {
                student_name: "J".to_string(), // Too short
                roll_number: "21CS101".to_string(),
                photo: Some("data:image/jpeg;base64,test".to_string()),
                latitude: 12.971,
                longitude: 77.594,
                direct_upload: None,
                public_id: None,
                face_detected: None,
                captcha_answer: None,
                captcha_id: None,
                device_fingerprint: None,
                user_agent: None,
                gps_data: None,
            };

            let result = req.validate();
            assert!(result.is_err(), "Short student name should be rejected");
        }
    }

    // ============================================
    // Auth Middleware Tests
    // ============================================
    mod auth_middleware_tests {
        use super::*;
        use uuid::Uuid;

        /// Helper: Generate a valid test token
        fn create_test_token() -> (Uuid, String) {
            let admin_id = Uuid::new_v4();
            let token = generate_token(&admin_id, JWT_SECRET, JWT_EXPIRE)
                .expect("Failed to generate token");
            (admin_id, token)
        }

        /// Test: should reject request without token
        /// Node.js: lines 306-311
        #[test]
        fn should_reject_request_without_token() {
            // No token provided - verification should fail
            let result = verify_token("", JWT_SECRET);
            assert!(result.is_err(), "Request without token should be rejected");
        }

        /// Test: should reject request with invalid token format
        /// Node.js: lines 313-319
        #[test]
        fn should_reject_request_with_invalid_token_format() {
            // Invalid token format - "InvalidToken" without Bearer prefix
            let result = verify_token("InvalidToken", JWT_SECRET);
            assert!(result.is_err(), "Invalid token format should be rejected");
        }

        /// Test: should accept request with valid token
        /// Node.js: lines 321-328
        #[test]
        fn should_accept_request_with_valid_token() {
            let (admin_id, token) = create_test_token();

            let result = verify_token(&token, JWT_SECRET);
            assert!(result.is_ok(), "Valid token should be accepted");

            let claims = result.unwrap();
            assert_eq!(
                claims.id,
                admin_id.to_string(),
                "Claims should contain correct admin ID"
            );
        }

        /// Test: should reject request with expired token
        /// Node.js: lines 330-343
        #[test]
        fn should_reject_request_with_expired_token() {
            use chrono::Utc;
            use jsonwebtoken::{encode, EncodingKey, Header};

            let admin_id = Uuid::new_v4();
            let now = Utc::now().timestamp() as usize;

            // Create an expired token (expired 1 hour ago)
            let expired_claims = Claims {
                id: admin_id.to_string(),
                exp: now - 3600, // Expired 1 hour ago
                iat: now - 7200, // Issued 2 hours ago
                jti: Uuid::new_v4().to_string(),
            };

            let expired_token = encode(
                &Header::default(),
                &expired_claims,
                &EncodingKey::from_secret(JWT_SECRET.as_bytes()),
            )
            .expect("Failed to encode expired token");

            let result = verify_token(&expired_token, JWT_SECRET);
            assert!(result.is_err(), "Expired token should be rejected");
        }

        /// Test: should reject request with malformed token
        /// Node.js: lines 345-351
        #[test]
        fn should_reject_request_with_malformed_token() {
            let result = verify_token("invalid.token.here", JWT_SECRET);
            assert!(result.is_err(), "Malformed token should be rejected");
        }

        /// Test: should reject token with wrong secret
        #[test]
        fn should_reject_token_with_wrong_secret() {
            let (admin_id, _) = create_test_token();
            let token = generate_token(&admin_id, JWT_SECRET, JWT_EXPIRE)
                .expect("Failed to generate token");

            // Verify with wrong secret
            let result = verify_token(&token, "wrong-secret");
            assert!(
                result.is_err(),
                "Token with wrong secret should be rejected"
            );
        }

        /// Test: token should contain correct claims structure
        #[test]
        fn token_should_contain_correct_claims_structure() {
            let (_admin_id, token) = create_test_token();

            let claims = verify_token(&token, JWT_SECRET).expect("Valid token should parse");

            // Verify claims structure
            assert!(!claims.id.is_empty(), "Subject should not be empty");
            assert!(claims.exp > 0, "Expiration should be positive");
            assert!(claims.iat > 0, "Issued at should be positive");
            assert!(
                claims.exp > claims.iat,
                "Expiration should be after issued at"
            );
        }

        /// Test: generate_token should produce valid JWT
        #[test]
        fn generate_token_should_produce_valid_jwt() {
            let admin_id = Uuid::new_v4();
            let token = generate_token(&admin_id, JWT_SECRET, JWT_EXPIRE)
                .expect("Token generation should succeed");

            // Token should have 3 parts (header.payload.signature)
            let parts: Vec<&str> = token.split('.').collect();
            assert_eq!(parts.len(), 3, "JWT should have 3 parts");

            // Should be verifiable
            let claims =
                verify_token(&token, JWT_SECRET).expect("Generated token should be verifiable");
            assert_eq!(claims.id, admin_id.to_string());
        }

        /// Test: different expiry formats should work
        #[test]
        fn different_expiry_formats_should_work() {
            let admin_id = Uuid::new_v4();

            // Test various expiry formats
            let expiry_formats = vec!["1d", "7d", "1h", "60m", "3600s"];

            for expiry in expiry_formats {
                let token = generate_token(&admin_id, JWT_SECRET, expiry)
                    .unwrap_or_else(|_| panic!("Failed with expiry: {}", expiry));
                let claims = verify_token(&token, JWT_SECRET)
                    .unwrap_or_else(|_| panic!("Failed to verify token with expiry: {}", expiry));
                assert_eq!(claims.id, admin_id.to_string());
            }
        }
    }

    // ============================================
    // Utility Function Tests
    // ============================================
    mod utility_tests {
        use super::*;

        /// Test: is_valid_email should work correctly
        #[test]
        fn test_is_valid_email() {
            assert!(
                is_valid_email("test@example.com"),
                "Valid email should pass"
            );
            assert!(
                is_valid_email("test.user@example.com"),
                "Email with dots should pass"
            );
            assert!(!is_valid_email("notanemail"), "Invalid email should fail");
            assert!(!is_valid_email("test@"), "Incomplete email should fail");
        }

        /// Test: is_valid_objectid should work correctly
        #[test]
        fn test_is_valid_objectid() {
            assert!(
                is_valid_objectid("550e8400-e29b-41d4-a716-446655440000"),
                "Valid Uuid should pass"
            );
            assert!(!is_valid_objectid("invalid-id"), "Invalid Uuid should fail");
            assert!(
                !is_valid_objectid("550e8400-e29b-41d4-a716-44665544000"),
                "Truncated UUID should fail"
            );
            assert!(
                !is_valid_objectid("507f1f77bcf86cd799439011"),
                "Legacy 24-char hex ObjectId should no longer pass"
            );
        }

        /// Test: is_alphanumeric should work correctly
        #[test]
        fn test_is_alphanumeric() {
            assert!(is_alphanumeric("testuser123"), "Alphanumeric should pass");
            assert!(is_alphanumeric("TESTUSER"), "Uppercase should pass");
            assert!(!is_alphanumeric("test-user"), "Hyphen should fail");
            assert!(!is_alphanumeric("test@user"), "@ should fail");
        }
    }

    // ============================================
    // XSS Sanitization Tests
    // ============================================
    mod xss_sanitization_tests {
        use super::*;

        /// Test: Location name should be sanitized
        #[test]
        fn location_name_should_be_sanitized() {
            let mut req = LocationCreateRequest {
                name: "<script>alert('xss')</script>".to_string(),
                latitude: 12.971,
                longitude: 77.594,
                radius_meters: Some(100),
            };

            req.validate_and_sanitize().expect("Validation should pass");

            assert!(req.name.contains("&lt;"), "Opening tag should be escaped");
            assert!(req.name.contains("&gt;"), "Closing tag should be escaped");
            assert!(!req.name.contains('<'), "Raw < should not exist");
        }

        /// Test: Attendance student name should be sanitized
        #[test]
        fn attendance_student_name_should_be_sanitized() {
            let mut req = AttendanceSubmitRequest {
                student_name: "<script>alert('xss')</script>".to_string(),
                roll_number: "21CS101".to_string(),
                photo: Some("data:image/jpeg;base64,test".to_string()),
                latitude: 12.971,
                longitude: 77.594,
                direct_upload: None,
                public_id: None,
                face_detected: None,
                captcha_answer: None,
                captcha_id: None,
                device_fingerprint: None,
                user_agent: None,
                gps_data: None,
            };

            req.validate_and_sanitize().expect("Validation should pass");

            assert!(
                req.student_name.contains("&lt;"),
                "Opening tag should be escaped"
            );
            assert!(!req.student_name.contains('<'), "Raw < should not exist");
        }
    }

    // ============================================
    // Edge Cases Tests
    // ============================================
    mod edge_case_tests {
        use super::*;

        /// Test: Admin with boundary username length (3 chars)
        #[test]
        fn admin_username_at_min_boundary() {
            let mut req = AdminRegisterRequest {
                username: "abc".to_string(), // Exactly 3 chars
                email: "admin@test.com".to_string(),
                password: "password123".to_string(),
                admin_secret: "secret".to_string(),
            };

            let result = req.validate_and_normalize();
            assert!(
                result.is_ok(),
                "Username at min boundary (3 chars) should pass"
            );
        }

        /// Test: Admin with boundary password length (6 chars)
        #[test]
        fn admin_password_at_min_boundary() {
            let mut req = AdminRegisterRequest {
                username: "testadmin".to_string(),
                email: "admin@test.com".to_string(),
                password: "123456".to_string(), // Exactly 6 chars
                admin_secret: "secret".to_string(),
            };

            let result = req.validate_and_normalize();
            assert!(
                result.is_ok(),
                "Password at min boundary (6 chars) should pass"
            );
        }

        /// Test: Session with boundary duration (5 minutes)
        #[test]
        fn session_duration_at_min_boundary() {
            let req = SessionCreateRequest {
                location_id: Some(create_valid_object_id()),
                duration_minutes: Some(5), // Exactly 5 minutes
                batch_id: Some(create_valid_object_id()),
                assigned_admin_ids: vec![create_valid_object_id()],
                college_name: Some("Test College".to_string()),
                starts_at: Some("2026-08-14T09:00:00Z".to_string()),
                description: None,
            };

            let result = req.validate_with_objectids();
            assert!(
                result.is_ok(),
                "Duration at min boundary (5 min) should pass"
            );
        }

        /// Test: Session with maximum duration (480 minutes)
        #[test]
        fn session_duration_at_max_boundary() {
            let req = SessionCreateRequest {
                location_id: Some(create_valid_object_id()),
                duration_minutes: Some(480), // Exactly 480 minutes
                batch_id: Some(create_valid_object_id()),
                assigned_admin_ids: vec![create_valid_object_id()],
                college_name: Some("Test College".to_string()),
                starts_at: Some("2026-08-14T09:00:00Z".to_string()),
                description: None,
            };

            let result = req.validate_with_objectids();
            assert!(
                result.is_ok(),
                "Duration at max boundary (480 min) should pass"
            );
        }

        /// Test: Location radius at boundaries
        #[test]
        fn location_radius_at_boundaries() {
            // Min radius (10)
            let mut req1 = LocationCreateRequest {
                name: "Test".to_string(),
                latitude: 12.971,
                longitude: 77.594,
                radius_meters: Some(10),
            };
            assert!(
                req1.validate_and_sanitize().is_ok(),
                "Radius 10 should be accepted"
            );

            // Max radius (10000)
            let mut req2 = LocationCreateRequest {
                name: "Test".to_string(),
                latitude: 12.971,
                longitude: 77.594,
                radius_meters: Some(10000),
            };
            assert!(
                req2.validate_and_sanitize().is_ok(),
                "Radius 10000 should be accepted"
            );
        }

        /// Test: Attendance with empty optional fields
        #[test]
        fn attendance_with_empty_optionals() {
            let req = AttendanceSubmitRequest {
                student_name: "John Doe".to_string(),
                roll_number: "21CS101".to_string(),
                photo: Some("data:image/jpeg;base64,test".to_string()),
                latitude: 12.971,
                longitude: 77.594,
                direct_upload: None,
                public_id: None,
                face_detected: None,
                captcha_answer: None,
                captcha_id: None,
                device_fingerprint: None,
                user_agent: None,
                gps_data: None,
            };

            let result = req.validate();
            assert!(
                result.is_ok(),
                "Attendance with empty optionals should pass"
            );
        }

        /// Test: batch_id is mandatory for an exam session (assigned_admin_ids
        /// present) — an empty batch_id must be rejected there, even though
        /// it's still allowed for a normal session (see
        /// `session_with_empty_batch_id_allowed_for_normal_session` below).
        #[test]
        fn session_with_empty_batch_id_rejected_for_exam_session() {
            let req = SessionCreateRequest {
                location_id: Some(create_valid_object_id()),
                duration_minutes: Some(30),
                batch_id: Some("".to_string()),
                assigned_admin_ids: vec![create_valid_object_id()],
                college_name: Some("Test College".to_string()),
                starts_at: Some("2026-08-14T09:00:00Z".to_string()),
                description: None,
            };

            let result = req.validate_with_objectids();
            assert!(
                result.is_err(),
                "Empty batch_id should be rejected for an exam session"
            );
        }

        /// Test: a normal session (no assigned_admin_ids) keeps the original
        /// optional-batch behavior — this is the exact scenario the user
        /// flagged: exam sessions require a batch, normal sessions don't.
        #[test]
        fn session_with_empty_batch_id_allowed_for_normal_session() {
            let req = SessionCreateRequest {
                location_id: Some(create_valid_object_id()),
                duration_minutes: Some(30),
                batch_id: None,
                assigned_admin_ids: vec![],
                college_name: None,
                starts_at: None,
                description: None,
            };

            assert!(!req.is_exam_session());
            let result = req.validate_with_objectids();
            assert!(
                result.is_ok(),
                "Empty/absent batch_id should be allowed for a normal session"
            );
        }
    }
}
