//! Tests for Attendance Model Edge Cases
//!
//! Ported from: backend/tests/attendance.test.js
//!
//! Tests cover:
//! - Roll Number Handling (uppercase, numeric-only, mixed)
//! - Duplicate Prevention (same session, different sessions)
//! - Student Name Handling (special characters, long names, short name validation)
//! - Coordinate Validation (boundaries, invalid values, Null Island)
//! - Verification Status (verified/unverified)
//! - Face Detection Status (default, explicit false)
//! - Distance Handling (store, large distances, zero distance)
//! - Network Provider and Org Handling
//! - Timestamp Handling (capturedAt auto-generation)
//! - DEV Bypass Audit Logging (flagged status, defaults)
//! - Session Expiry Tests

use chrono::{Duration, Utc};
use uuid::Uuid;

// Mock implementations for testing purposes
// These simulate the behavior of the actual models

/// Mock Admin for testing
pub struct MockAdmin {
    pub id: Uuid,
    pub _username: String,
    pub _email: String,
    pub _password: String,
}

impl MockAdmin {
    pub fn new(username: &str, email: &str, password: &str) -> Self {
        Self {
            id: Uuid::new_v4(),
            _username: username.to_string(),
            _email: email.to_string(),
            _password: password.to_string(),
        }
    }
}

/// Mock Location for testing
pub struct MockLocation {
    pub id: Uuid,
    pub _name: String,
    pub latitude: f64,
    pub longitude: f64,
    pub _radius_meters: f64,
    pub _created_by: Uuid,
}

impl MockLocation {
    pub fn new(
        name: &str,
        latitude: f64,
        longitude: f64,
        radius_meters: f64,
        created_by: Uuid,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            _name: name.to_string(),
            latitude,
            longitude,
            _radius_meters: radius_meters,
            _created_by: created_by,
        }
    }
}

/// Mock Session for testing
pub struct MockSession {
    pub id: Uuid,
    pub location_id: Uuid,
    pub token_hash: String,
    pub token_prefix: String,
    pub created_by: Uuid,
    pub expires_at: chrono::DateTime<Utc>,
    pub is_active: bool,
}

impl MockSession {
    pub fn generate_token() -> String {
        use rand::Rng;
        let mut rng = rand::rng();
        let mut bytes = [0u8; 16];
        rng.fill_bytes(&mut bytes);
        hex::encode(bytes)
    }

    pub fn hash_token(token: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        hex::encode(hasher.finalize())
    }

    pub fn new(location_id: Uuid, created_by: Uuid, expires_at: chrono::DateTime<Utc>) -> Self {
        let token = Self::generate_token();
        let s = Self {
            id: Uuid::new_v4(),
            location_id,
            token_hash: Self::hash_token(&token),
            token_prefix: token.chars().take(4).collect(),
            created_by,
            expires_at,
            is_active: true,
        };
        let _ = (
            &s.location_id,
            &s.token_hash,
            &s.token_prefix,
            &s.created_by,
        );
        s
    }

    pub fn new_with_active_status(
        location_id: Uuid,
        created_by: Uuid,
        expires_at: chrono::DateTime<Utc>,
        is_active: bool,
    ) -> Self {
        let token = Self::generate_token();
        Self {
            id: Uuid::new_v4(),
            location_id,
            token_hash: Self::hash_token(&token),
            token_prefix: token.chars().take(4).collect(),
            created_by,
            expires_at,
            is_active,
        }
    }
}

/// Mock Attendance for testing
#[derive(Debug, Clone)]
pub struct MockAttendance {
    pub _id: Uuid,
    pub session_id: Uuid,
    pub student_name: String,
    pub roll_number: String,
    pub _photo_url: String,
    pub _photo_public_id: String,
    pub student_latitude: f64,
    pub student_longitude: f64,
    pub distance_from_location: f64,
    pub verified: bool,
    pub face_detected: bool,
    pub network_provider: Option<String>,
    pub network_org: Option<String>,
    pub captured_at: chrono::DateTime<Utc>,
    pub flagged: bool,
    pub flag_reason: Option<String>,
    pub flag_details: Option<String>,
}

impl MockAttendance {
    /// Create a new attendance record with roll number validation
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        session_id: Uuid,
        student_name: &str,
        roll_number: &str,
        photo_url: &str,
        photo_public_id: &str,
        student_latitude: f64,
        student_longitude: f64,
        distance_from_location: f64,
        verified: bool,
    ) -> Result<Self, String> {
        // Validate student name length
        if student_name.len() < 2 {
            return Err("Student name must be at least 2 characters".to_string());
        }
        if student_name.len() > 100 {
            return Err("Student name must be at most 100 characters".to_string());
        }

        // Validate coordinates
        if !(-90.0..=90.0).contains(&student_latitude) {
            return Err("Latitude must be between -90 and 90".to_string());
        }
        if !(-180.0..=180.0).contains(&student_longitude) {
            return Err("Longitude must be between -180 and 180".to_string());
        }

        // Uppercase roll number (matches Node.js behavior)
        let roll_number = roll_number.to_uppercase();

        Ok(Self {
            _id: Uuid::new_v4(),
            session_id,
            student_name: student_name.to_string(),
            roll_number,
            _photo_url: photo_url.to_string(),
            _photo_public_id: photo_public_id.to_string(),
            student_latitude,
            student_longitude,
            distance_from_location,
            verified,
            face_detected: true, // Default value
            network_provider: None,
            network_org: None,
            captured_at: Utc::now(),
            flagged: false,
            flag_reason: None,
            flag_details: None,
        })
    }

    /// Create attendance with optional fields
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_options(
        session_id: Uuid,
        student_name: &str,
        roll_number: &str,
        photo_url: &str,
        photo_public_id: &str,
        student_latitude: f64,
        student_longitude: f64,
        distance_from_location: f64,
        verified: bool,
        face_detected: bool,
        flagged: bool,
        flag_reason: Option<String>,
        flag_details: Option<String>,
        network_provider: Option<String>,
        network_org: Option<String>,
    ) -> Result<Self, String> {
        let mut attendance = Self::new(
            session_id,
            student_name,
            roll_number,
            photo_url,
            photo_public_id,
            student_latitude,
            student_longitude,
            distance_from_location,
            verified,
        )?;

        attendance.face_detected = face_detected;
        attendance.flagged = flagged;
        attendance.flag_reason = flag_reason;
        attendance.flag_details = flag_details;
        attendance.network_provider = network_provider;
        attendance.network_org = network_org;

        Ok(attendance)
    }
}

/// In-memory storage for testing duplicate prevention
pub struct MockAttendanceStore {
    records: Vec<MockAttendance>,
}

impl MockAttendanceStore {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    /// Insert attendance, returns error if duplicate roll number in same session
    pub fn insert(&mut self, attendance: MockAttendance) -> Result<MockAttendance, String> {
        // Check for duplicate roll number in same session
        let normalized_roll = attendance.roll_number.to_uppercase();
        for existing in &self.records {
            if existing.session_id == attendance.session_id
                && existing.roll_number.to_uppercase() == normalized_roll
            {
                return Err("Duplicate roll number in same session".to_string());
            }
        }

        let result = attendance.clone();
        self.records.push(attendance);
        Ok(result)
    }
}

// ============================================================================
// Attendance Model Edge Cases Tests
// ============================================================================

mod roll_number_handling {
    use super::*;

    fn setup_test_data() -> (MockAdmin, MockLocation, MockSession) {
        let admin = MockAdmin::new("testadmin", "admin@test.com", "password123");
        let location = MockLocation::new("Test Location", 12.9715987, 77.5945627, 100.0, admin.id);
        let session = MockSession::new(location.id, admin.id, Utc::now() + Duration::minutes(30));
        (admin, location, session)
    }

    /// Test: should uppercase roll number on save
    ///
    /// Original Node.js test (lines 43-57):
    /// ```js
    /// test('should uppercase roll number on save', async () => {
    ///   const attendance = await Attendance.create({
    ///     sessionId: session._id,
    ///     studentName: 'John Doe',
    ///     rollNumber: '21cs101',
    ///     ...
    ///   });
    ///   expect(attendance.rollNumber).toBe('21CS101');
    /// });
    /// ```
    #[test]
    fn should_uppercase_roll_number_on_save() {
        let (_, _, session) = setup_test_data();

        let attendance = MockAttendance::new(
            session.id,
            "John Doe",
            "21cs101", // lowercase input
            "https://example.com/photo.jpg",
            "photo123",
            12.971,
            77.594,
            50.0,
            true,
        )
        .expect("Should create attendance");

        assert_eq!(
            attendance.roll_number, "21CS101",
            "Roll number should be uppercased"
        );
    }

    /// Test: should accept numeric-only roll numbers
    ///
    /// Original Node.js test (lines 59-73):
    /// ```js
    /// test('should accept numeric-only roll numbers', async () => {
    ///   const attendance = await Attendance.create({
    ///     rollNumber: '12345678',
    ///     ...
    ///   });
    ///   expect(attendance.rollNumber).toBe('12345678');
    /// });
    /// ```
    #[test]
    fn should_accept_numeric_only_roll_numbers() {
        let (_, _, session) = setup_test_data();

        let attendance = MockAttendance::new(
            session.id,
            "Number Student",
            "12345678", // numeric only
            "https://example.com/photo.jpg",
            "photo123",
            12.971,
            77.594,
            50.0,
            true,
        )
        .expect("Should create attendance with numeric roll number");

        assert_eq!(attendance.roll_number, "12345678");
    }

    /// Test: should accept mixed roll numbers
    ///
    /// Original Node.js test (lines 75-89):
    /// ```js
    /// test('should accept mixed roll numbers', async () => {
    ///   const attendance = await Attendance.create({
    ///     rollNumber: '21CS1A001',
    ///     ...
    ///   });
    ///   expect(attendance.rollNumber).toBe('21CS1A001');
    /// });
    /// ```
    #[test]
    fn should_accept_mixed_roll_numbers() {
        let (_, _, session) = setup_test_data();

        let attendance = MockAttendance::new(
            session.id,
            "Mixed Student",
            "21CS1A001", // mixed alphanumeric
            "https://example.com/photo.jpg",
            "photo123",
            12.971,
            77.594,
            50.0,
            true,
        )
        .expect("Should create attendance with mixed roll number");

        assert_eq!(attendance.roll_number, "21CS1A001");
    }
}

mod duplicate_prevention {
    use super::*;

    fn setup_test_data() -> (MockAdmin, MockLocation, MockSession) {
        let admin = MockAdmin::new("testadmin", "admin@test.com", "password123");
        let location = MockLocation::new("Test Location", 12.9715987, 77.5945627, 100.0, admin.id);
        let session = MockSession::new(location.id, admin.id, Utc::now() + Duration::minutes(30));
        (admin, location, session)
    }

    /// Test: should prevent duplicate roll number in same session
    ///
    /// Original Node.js test (lines 93-119):
    /// ```js
    /// test('should prevent duplicate roll number in same session', async () => {
    ///   await Attendance.create({ rollNumber: '21CS101', ... });
    ///   await expect(
    ///     Attendance.create({ rollNumber: '21CS101', ... })
    ///   ).rejects.toThrow();
    /// });
    /// ```
    #[test]
    fn should_prevent_duplicate_roll_number_in_same_session() {
        let (_, _, session) = setup_test_data();
        let mut store = MockAttendanceStore::new();

        // First attendance should succeed
        let attendance1 = MockAttendance::new(
            session.id,
            "John Doe",
            "21CS101",
            "https://example.com/photo.jpg",
            "photo123",
            12.971,
            77.594,
            50.0,
            true,
        )
        .expect("Should create first attendance");

        let result1 = store.insert(attendance1);
        assert!(result1.is_ok(), "First attendance should succeed");

        // Second attendance with same roll number in same session should fail
        let attendance2 = MockAttendance::new(
            session.id,
            "Jane Doe",
            "21CS101", // Same roll number
            "https://example.com/photo2.jpg",
            "photo124",
            12.972,
            77.595,
            60.0,
            true,
        )
        .expect("Should create attendance struct");

        let result2 = store.insert(attendance2);
        assert!(
            result2.is_err(),
            "Should reject duplicate roll number in same session"
        );
    }

    /// Test: should allow same roll number in different sessions
    ///
    /// Original Node.js test (lines 121-156):
    /// ```js
    /// test('should allow same roll number in different sessions', async () => {
    ///   await Attendance.create({ rollNumber: '21CS101', ... });
    ///   const session2 = await Session.create({ ... });
    ///   const attendance2 = await Attendance.create({
    ///     sessionId: session2._id, rollNumber: '21CS101', ...
    ///   });
    ///   expect(attendance2.rollNumber).toBe('21CS101');
    /// });
    /// ```
    #[test]
    fn should_allow_same_roll_number_in_different_sessions() {
        let (admin, location, session) = setup_test_data();
        let mut store = MockAttendanceStore::new();

        // First attendance in session 1
        let attendance1 = MockAttendance::new(
            session.id,
            "John Doe",
            "21CS101",
            "https://example.com/photo.jpg",
            "photo123",
            12.971,
            77.594,
            50.0,
            true,
        )
        .expect("Should create first attendance");

        let result1 = store.insert(attendance1);
        assert!(result1.is_ok(), "First attendance should succeed");

        // Create second session
        let session2 = MockSession::new(location.id, admin.id, Utc::now() + Duration::minutes(30));

        // Same roll number in different session should succeed
        let attendance2 = MockAttendance::new(
            session2.id, // Different session
            "John Doe",
            "21CS101", // Same roll number
            "https://example.com/photo2.jpg",
            "photo124",
            12.971,
            77.594,
            50.0,
            true,
        )
        .expect("Should create attendance struct");

        let result2 = store.insert(attendance2);
        assert!(
            result2.is_ok(),
            "Should allow same roll number in different session"
        );

        let saved_attendance = result2.unwrap();
        assert_eq!(saved_attendance.roll_number, "21CS101");
    }
}

mod student_name_handling {
    use super::*;

    fn setup_test_data() -> MockSession {
        let admin = MockAdmin::new("testadmin", "admin@test.com", "password123");
        let location = MockLocation::new("Test Location", 12.9715987, 77.5945627, 100.0, admin.id);
        MockSession::new(location.id, admin.id, Utc::now() + Duration::minutes(30))
    }

    /// Test: should handle special characters in name
    ///
    /// Original Node.js test (lines 160-174):
    /// ```js
    /// test('should handle special characters in name', async () => {
    ///   const attendance = await Attendance.create({
    ///     studentName: "John O'Brien-Smith Jr.",
    ///     ...
    ///   });
    ///   expect(attendance.studentName).toBe("John O'Brien-Smith Jr.");
    /// });
    /// ```
    #[test]
    fn should_handle_special_characters_in_name() {
        let session = setup_test_data();

        let attendance = MockAttendance::new(
            session.id,
            "John O'Brien-Smith Jr.", // Name with special characters
            "21CS101",
            "https://example.com/photo.jpg",
            "photo123",
            12.971,
            77.594,
            50.0,
            true,
        )
        .expect("Should create attendance with special characters in name");

        assert_eq!(attendance.student_name, "John O'Brien-Smith Jr.");
    }

    /// Test: should handle very long names (up to 100 chars)
    ///
    /// Original Node.js test (lines 176-191):
    /// ```js
    /// test('should handle very long names (up to 100 chars)', async () => {
    ///   const longName = 'A'.repeat(100);
    ///   const attendance = await Attendance.create({ studentName: longName, ... });
    ///   expect(attendance.studentName).toHaveLength(100);
    /// });
    /// ```
    #[test]
    fn should_handle_very_long_names_up_to_100_chars() {
        let session = setup_test_data();
        let long_name: String = "A".repeat(100);

        let attendance = MockAttendance::new(
            session.id,
            &long_name,
            "21CS101",
            "https://example.com/photo.jpg",
            "photo123",
            12.971,
            77.594,
            50.0,
            true,
        )
        .expect("Should create attendance with 100 char name");

        assert_eq!(attendance.student_name.len(), 100);
    }

    /// Test: should reject names shorter than 2 characters
    ///
    /// Original Node.js test (lines 193-207):
    /// ```js
    /// test('should reject names shorter than 2 characters', async () => {
    ///   await expect(
    ///     Attendance.create({ studentName: 'J', ... })
    ///   ).rejects.toThrow();
    /// });
    /// ```
    #[test]
    fn should_reject_names_shorter_than_2_characters() {
        let session = setup_test_data();

        let result = MockAttendance::new(
            session.id,
            "J", // Too short (1 character)
            "21CS101",
            "https://example.com/photo.jpg",
            "photo123",
            12.971,
            77.594,
            50.0,
            true,
        );

        assert!(
            result.is_err(),
            "Should reject name shorter than 2 characters"
        );
    }
}

mod coordinate_validation {
    use super::*;

    fn setup_test_data() -> MockSession {
        let admin = MockAdmin::new("testadmin", "admin@test.com", "password123");
        let location = MockLocation::new("Test Location", 12.9715987, 77.5945627, 100.0, admin.id);
        MockSession::new(location.id, admin.id, Utc::now() + Duration::minutes(30))
    }

    /// Test: should accept coordinates at exact boundaries
    ///
    /// Original Node.js test (lines 211-226):
    /// ```js
    /// test('should accept coordinates at exact boundaries', async () => {
    ///   const attendance = await Attendance.create({
    ///     studentLatitude: 90,
    ///     studentLongitude: 180,
    ///     ...
    ///   });
    ///   expect(attendance.studentLatitude).toBe(90);
    ///   expect(attendance.studentLongitude).toBe(180);
    /// });
    /// ```
    #[test]
    fn should_accept_coordinates_at_exact_boundaries() {
        let session = setup_test_data();

        let attendance = MockAttendance::new(
            session.id,
            "Boundary Test",
            "21CS101",
            "https://example.com/photo.jpg",
            "photo123",
            90.0,  // Max latitude
            180.0, // Max longitude
            50.0,
            true,
        )
        .expect("Should accept coordinates at exact boundaries");

        assert_eq!(attendance.student_latitude, 90.0);
        assert_eq!(attendance.student_longitude, 180.0);
    }

    /// Test: should accept coordinates at negative boundaries
    ///
    /// Original Node.js test (lines 228-243):
    /// ```js
    /// test('should accept coordinates at negative boundaries', async () => {
    ///   const attendance = await Attendance.create({
    ///     studentLatitude: -90,
    ///     studentLongitude: -180,
    ///     ...
    ///   });
    ///   expect(attendance.studentLatitude).toBe(-90);
    ///   expect(attendance.studentLongitude).toBe(-180);
    /// });
    /// ```
    #[test]
    fn should_accept_coordinates_at_negative_boundaries() {
        let session = setup_test_data();

        let attendance = MockAttendance::new(
            session.id,
            "Boundary Test",
            "21CS102",
            "https://example.com/photo.jpg",
            "photo124",
            -90.0,  // Min latitude
            -180.0, // Min longitude
            50.0,
            true,
        )
        .expect("Should accept coordinates at negative boundaries");

        assert_eq!(attendance.student_latitude, -90.0);
        assert_eq!(attendance.student_longitude, -180.0);
    }

    /// Test: should reject latitude > 90
    ///
    /// Original Node.js test (lines 245-259):
    /// ```js
    /// test('should reject latitude > 90', async () => {
    ///   await expect(
    ///     Attendance.create({ studentLatitude: 91, ... })
    ///   ).rejects.toThrow();
    /// });
    /// ```
    #[test]
    fn should_reject_latitude_greater_than_90() {
        let session = setup_test_data();

        let result = MockAttendance::new(
            session.id,
            "Invalid Test",
            "21CS103",
            "https://example.com/photo.jpg",
            "photo125",
            91.0, // Invalid: > 90
            77.594,
            50.0,
            true,
        );

        assert!(result.is_err(), "Should reject latitude > 90");
    }

    /// Test: should reject longitude > 180
    ///
    /// Original Node.js test (lines 261-275):
    /// ```js
    /// test('should reject longitude > 180', async () => {
    ///   await expect(
    ///     Attendance.create({ studentLongitude: 181, ... })
    ///   ).rejects.toThrow();
    /// });
    /// ```
    #[test]
    fn should_reject_longitude_greater_than_180() {
        let session = setup_test_data();

        let result = MockAttendance::new(
            session.id,
            "Invalid Test",
            "21CS104",
            "https://example.com/photo.jpg",
            "photo126",
            12.971,
            181.0, // Invalid: > 180
            50.0,
            true,
        );

        assert!(result.is_err(), "Should reject longitude > 180");
    }

    /// Test: should handle coordinates at 0,0 (Null Island)
    ///
    /// Original Node.js test (lines 277-292):
    /// ```js
    /// test('should handle coordinates at 0,0 (Null Island)', async () => {
    ///   const attendance = await Attendance.create({
    ///     studentLatitude: 0,
    ///     studentLongitude: 0,
    ///     ...
    ///   });
    ///   expect(attendance.studentLatitude).toBe(0);
    ///   expect(attendance.studentLongitude).toBe(0);
    /// });
    /// ```
    #[test]
    fn should_handle_coordinates_at_null_island() {
        let session = setup_test_data();

        let attendance = MockAttendance::new(
            session.id,
            "Null Island Test",
            "21CS105",
            "https://example.com/photo.jpg",
            "photo127",
            0.0, // Null Island
            0.0,
            50.0,
            true,
        )
        .expect("Should accept coordinates at 0,0");

        assert_eq!(attendance.student_latitude, 0.0);
        assert_eq!(attendance.student_longitude, 0.0);
    }
}

mod verification_status {
    use super::*;

    fn setup_test_data() -> MockSession {
        let admin = MockAdmin::new("testadmin", "admin@test.com", "password123");
        let location = MockLocation::new("Test Location", 12.9715987, 77.5945627, 100.0, admin.id);
        MockSession::new(location.id, admin.id, Utc::now() + Duration::minutes(30))
    }

    /// Test: should store verified status correctly
    ///
    /// Original Node.js test (lines 296-310):
    /// ```js
    /// test('should store verified status correctly', async () => {
    ///   const attendance = await Attendance.create({ verified: true, ... });
    ///   expect(attendance.verified).toBe(true);
    /// });
    /// ```
    #[test]
    fn should_store_verified_status_correctly() {
        let session = setup_test_data();

        let attendance = MockAttendance::new(
            session.id,
            "Verified Test",
            "21CS106",
            "https://example.com/photo.jpg",
            "photo128",
            12.971,
            77.594,
            50.0,
            true, // verified
        )
        .expect("Should create attendance");

        assert!(attendance.verified, "Verified status should be true");
    }

    /// Test: should store unverified status correctly
    ///
    /// Original Node.js test (lines 312-326):
    /// ```js
    /// test('should store unverified status correctly', async () => {
    ///   const attendance = await Attendance.create({ verified: false, ... });
    ///   expect(attendance.verified).toBe(false);
    /// });
    /// ```
    #[test]
    fn should_store_unverified_status_correctly() {
        let session = setup_test_data();

        let attendance = MockAttendance::new(
            session.id,
            "Unverified Test",
            "21CS107",
            "https://example.com/photo.jpg",
            "photo129",
            12.980,
            77.600,
            1500.0,
            false, // unverified
        )
        .expect("Should create attendance");

        assert!(!attendance.verified, "Verified status should be false");
    }
}

mod face_detection_status {
    use super::*;

    fn setup_test_data() -> MockSession {
        let admin = MockAdmin::new("testadmin", "admin@test.com", "password123");
        let location = MockLocation::new("Test Location", 12.9715987, 77.5945627, 100.0, admin.id);
        MockSession::new(location.id, admin.id, Utc::now() + Duration::minutes(30))
    }

    /// Test: should default faceDetected to true
    ///
    /// Original Node.js test (lines 330-344):
    /// ```js
    /// test('should default faceDetected to true', async () => {
    ///   const attendance = await Attendance.create({ ... });
    ///   expect(attendance.faceDetected).toBe(true);
    /// });
    /// ```
    #[test]
    fn should_default_face_detected_to_true() {
        let session = setup_test_data();

        let attendance = MockAttendance::new(
            session.id,
            "Face Default Test",
            "21CS108",
            "https://example.com/photo.jpg",
            "photo130",
            12.971,
            77.594,
            50.0,
            true,
        )
        .expect("Should create attendance");

        // face_detected defaults to true in MockAttendance::new
        assert!(
            attendance.face_detected,
            "faceDetected should default to true"
        );
    }

    /// Test: should store faceDetected as false when passed
    ///
    /// Original Node.js test (lines 346-361):
    /// ```js
    /// test('should store faceDetected as false when passed', async () => {
    ///   const attendance = await Attendance.create({
    ///     faceDetected: false, ...
    ///   });
    ///   expect(attendance.faceDetected).toBe(false);
    /// });
    /// ```
    #[test]
    fn should_store_face_detected_as_false_when_passed() {
        let session = setup_test_data();

        let attendance = MockAttendance::new_with_options(
            session.id,
            "Face False Test",
            "21CS109",
            "https://example.com/photo.jpg",
            "photo131",
            12.971,
            77.594,
            50.0,
            true,
            false, // faceDetected: false
            false,
            None,
            None,
            None,
            None,
        )
        .expect("Should create attendance");

        assert!(!attendance.face_detected, "faceDetected should be false");
    }
}

mod distance_handling {
    use super::*;

    fn setup_test_data() -> (MockAdmin, MockLocation, MockSession) {
        let admin = MockAdmin::new("testadmin", "admin@test.com", "password123");
        let location = MockLocation::new("Test Location", 12.9715987, 77.5945627, 100.0, admin.id);
        let session = MockSession::new(location.id, admin.id, Utc::now() + Duration::minutes(30));
        (admin, location, session)
    }

    /// Test: should store distance correctly
    ///
    /// Original Node.js test (lines 365-379):
    /// ```js
    /// test('should store distance correctly', async () => {
    ///   const attendance = await Attendance.create({
    ///     distanceFromLocation: 50, ...
    ///   });
    ///   expect(attendance.distanceFromLocation).toBe(50);
    /// });
    /// ```
    #[test]
    fn should_store_distance_correctly() {
        let (_, _, session) = setup_test_data();

        let attendance = MockAttendance::new(
            session.id,
            "Distance Test",
            "21CS108",
            "https://example.com/photo.jpg",
            "photo130",
            12.971,
            77.594,
            50.0,
            true,
        )
        .expect("Should create attendance");

        assert_eq!(attendance.distance_from_location, 50.0);
    }

    /// Test: should handle large distances
    ///
    /// Original Node.js test (lines 381-395):
    /// ```js
    /// test('should handle large distances', async () => {
    ///   const attendance = await Attendance.create({
    ///     distanceFromLocation: 1750000, ...
    ///   });
    ///   expect(attendance.distanceFromLocation).toBe(1750000);
    /// });
    /// ```
    #[test]
    fn should_handle_large_distances() {
        let (_, _, session) = setup_test_data();

        let attendance = MockAttendance::new(
            session.id,
            "Far Away Test",
            "21CS109",
            "https://example.com/photo.jpg",
            "photo131",
            28.7041,
            77.1025,
            1750000.0, // Large distance
            false,
        )
        .expect("Should create attendance");

        assert_eq!(attendance.distance_from_location, 1750000.0);
    }

    /// Test: should handle zero distance
    ///
    /// Original Node.js test (lines 397-411):
    /// ```js
    /// test('should handle zero distance', async () => {
    ///   const attendance = await Attendance.create({
    ///     distanceFromLocation: 0, ...
    ///   });
    ///   expect(attendance.distanceFromLocation).toBe(0);
    /// });
    /// ```
    #[test]
    fn should_handle_zero_distance() {
        let (_, location, session) = setup_test_data();

        let attendance = MockAttendance::new(
            session.id,
            "Exact Location Test",
            "21CS110",
            "https://example.com/photo.jpg",
            "photo132",
            location.latitude, // Same as location
            location.longitude,
            0.0, // Zero distance
            true,
        )
        .expect("Should create attendance");

        assert_eq!(attendance.distance_from_location, 0.0);
    }
}

mod network_provider_and_org_handling {
    use super::*;

    fn setup_test_data() -> MockSession {
        let admin = MockAdmin::new("testadmin", "admin@test.com", "password123");
        let location = MockLocation::new("Test Location", 12.9715987, 77.5945627, 100.0, admin.id);
        MockSession::new(location.id, admin.id, Utc::now() + Duration::minutes(30))
    }

    /// Test: should store networkProvider and networkOrg correctly
    ///
    /// Original Node.js test (lines 415-432):
    /// ```js
    /// test('should store networkProvider and networkOrg correctly', async () => {
    ///   const attendance = await Attendance.create({
    ///     networkProvider: 'Reliance Jio Infocomm',
    ///     networkOrg: 'Jio',
    ///     ...
    ///   });
    ///   expect(attendance.networkProvider).toBe('Reliance Jio Infocomm');
    ///   expect(attendance.networkOrg).toBe('Jio');
    /// });
    /// ```
    #[test]
    fn should_store_network_provider_and_network_org_correctly() {
        let session = setup_test_data();

        let attendance = MockAttendance::new_with_options(
            session.id,
            "Network Student",
            "21CS112",
            "https://example.com/photo.jpg",
            "photo134",
            12.971,
            77.594,
            50.0,
            true,
            true,
            false,
            None,
            None,
            Some("Reliance Jio Infocomm".to_string()),
            Some("Jio".to_string()),
        )
        .expect("Should create attendance");

        assert_eq!(
            attendance.network_provider,
            Some("Reliance Jio Infocomm".to_string())
        );
        assert_eq!(attendance.network_org, Some("Jio".to_string()));
    }

    /// Test: should default networkProvider and networkOrg to undefined/not present if omitted
    ///
    /// Original Node.js test (lines 434-449):
    /// ```js
    /// test('should default networkProvider and networkOrg to undefined/not present if omitted', async () => {
    ///   const attendance = await Attendance.create({ ... });
    ///   expect(attendance.networkProvider).toBeUndefined();
    ///   expect(attendance.networkOrg).toBeUndefined();
    /// });
    /// ```
    #[test]
    fn should_default_network_provider_and_network_org_to_undefined_if_omitted() {
        let session = setup_test_data();

        let attendance = MockAttendance::new(
            session.id,
            "No Network Info Student",
            "21CS113",
            "https://example.com/photo.jpg",
            "photo135",
            12.971,
            77.594,
            50.0,
            true,
        )
        .expect("Should create attendance");

        // Network provider and org should be None by default
        assert!(
            attendance.network_provider.is_none(),
            "networkProvider should be None"
        );
        assert!(
            attendance.network_org.is_none(),
            "networkOrg should be None"
        );
    }
}

mod timestamp_handling {
    use super::*;

    fn setup_test_data() -> MockSession {
        let admin = MockAdmin::new("testadmin", "admin@test.com", "password123");
        let location = MockLocation::new("Test Location", 12.9715987, 77.5945627, 100.0, admin.id);
        MockSession::new(location.id, admin.id, Utc::now() + Duration::minutes(30))
    }

    /// Test: should auto-generate capturedAt timestamp
    ///
    /// Original Node.js test (lines 453-471):
    /// ```js
    /// test('should auto-generate capturedAt timestamp', async () => {
    ///   const beforeCreate = new Date();
    ///   const attendance = await Attendance.create({ ... });
    ///   const afterCreate = new Date();
    ///   
    ///   expect(attendance.capturedAt).toBeDefined();
    ///   expect(attendance.capturedAt.getTime()).toBeGreaterThanOrEqual(beforeCreate.getTime() - 1000);
    ///   expect(attendance.capturedAt.getTime()).toBeLessThanOrEqual(afterCreate.getTime() + 1000);
    /// });
    /// ```
    #[test]
    fn should_auto_generate_captured_at_timestamp() {
        let session = setup_test_data();

        let before_create = Utc::now();
        let attendance = MockAttendance::new(
            session.id,
            "Timestamp Test",
            "21CS111",
            "https://example.com/photo.jpg",
            "photo133",
            12.971,
            77.594,
            50.0,
            true,
        )
        .expect("Should create attendance");
        let after_create = Utc::now();

        // capturedAt should be defined
        assert!(
            attendance.captured_at.timestamp() > 0,
            "capturedAt should be defined"
        );

        // capturedAt should be between before_create and after_create (with 1 second tolerance)
        let tolerance = chrono::Duration::seconds(1);
        assert!(
            attendance.captured_at >= before_create - tolerance,
            "capturedAt should be >= beforeCreate"
        );
        assert!(
            attendance.captured_at <= after_create + tolerance,
            "capturedAt should be <= afterCreate"
        );
    }
}

mod dev_bypass_audit_logging {
    use super::*;

    fn setup_test_data() -> MockSession {
        let admin = MockAdmin::new("testadmin", "admin@test.com", "password123");
        let location = MockLocation::new("Test Location", 12.9715987, 77.5945627, 100.0, admin.id);
        MockSession::new(location.id, admin.id, Utc::now() + Duration::minutes(30))
    }

    /// Test: should store bypass flags correctly
    ///
    /// Original Node.js test (lines 474-493):
    /// ```js
    /// test('should store bypass flags correctly', async () => {
    ///   const attendance = await Attendance.create({
    ///     flagged: true,
    ///     flagReason: 'DEV_BYPASS_ENABLED',
    ///     flagDetails: 'Camera:true, GPS:false, WebAuthn:true',
    ///     ...
    ///   });
    ///   
    ///   expect(attendance.flagged).toBe(true);
    ///   expect(attendance.flagReason).toBe('DEV_BYPASS_ENABLED');
    ///   expect(attendance.flagDetails).toBe('Camera:true, GPS:false, WebAuthn:true');
    /// });
    /// ```
    #[test]
    fn should_store_bypass_flags_correctly() {
        let session = setup_test_data();

        let attendance = MockAttendance::new_with_options(
            session.id,
            "Bypass Student",
            "21CS114",
            "https://example.com/photo.jpg",
            "photo136",
            12.971,
            77.594,
            0.0,
            true,
            true,
            true, // flagged
            Some("DEV_BYPASS_ENABLED".to_string()),
            Some("Camera:true, GPS:false, WebAuthn:true".to_string()),
            None,
            None,
        )
        .expect("Should create attendance");

        assert!(attendance.flagged, "flagged should be true");
        assert_eq!(
            attendance.flag_reason,
            Some("DEV_BYPASS_ENABLED".to_string())
        );
        assert_eq!(
            attendance.flag_details,
            Some("Camera:true, GPS:false, WebAuthn:true".to_string())
        );
    }

    /// Test: should default flagged to false
    ///
    /// Original Node.js test (lines 495-511):
    /// ```js
    /// test('should default flagged to false', async () => {
    ///   const attendance = await Attendance.create({ ... });
    ///   expect(attendance.flagged).toBe(false);
    ///   expect(attendance.flagReason).toBeNull();
    ///   expect(attendance.flagDetails).toBeNull();
    /// });
    /// ```
    #[test]
    fn should_default_flagged_to_false() {
        let session = setup_test_data();

        let attendance = MockAttendance::new(
            session.id,
            "Normal Student",
            "21CS115",
            "https://example.com/photo.jpg",
            "photo137",
            12.971,
            77.594,
            50.0,
            true,
        )
        .expect("Should create attendance");

        assert!(!attendance.flagged, "flagged should default to false");
        assert!(
            attendance.flag_reason.is_none(),
            "flagReason should be None"
        );
        assert!(
            attendance.flag_details.is_none(),
            "flagDetails should be None"
        );
    }
}

// ============================================================================
// Session Expiry Tests
// ============================================================================

mod session_expiry_tests {
    use super::*;

    fn setup_test_data() -> (MockAdmin, MockLocation) {
        let admin = MockAdmin::new("testadmin", "admin@test.com", "password123");
        let location = MockLocation::new("Test Location", 12.971, 77.594, 100.0, admin.id);
        (admin, location)
    }

    /// Test: should create session with future expiry
    ///
    /// Original Node.js test (lines 538-549):
    /// ```js
    /// test('should create session with future expiry', async () => {
    ///   const token = Session.generateToken();
    ///   const session = await Session.create({
    ///     expiresAt: new Date(Date.now() + 30 * 60 * 1000),
    ///     ...
    ///   });
    ///   expect(session.expiresAt.getTime()).toBeGreaterThan(Date.now());
    /// });
    /// ```
    #[test]
    fn should_create_session_with_future_expiry() {
        let (admin, location) = setup_test_data();

        let expires_at = Utc::now() + Duration::minutes(30);
        let session = MockSession::new(location.id, admin.id, expires_at);

        assert!(
            session.expires_at > Utc::now(),
            "expiresAt should be in the future"
        );
    }

    /// Test: should allow expired session to exist in DB
    ///
    /// Original Node.js test (lines 551-562):
    /// ```js
    /// test('should allow expired session to exist in DB', async () => {
    ///   const token = Session.generateToken();
    ///   const session = await Session.create({
    ///     expiresAt: new Date(Date.now() - 1000), // Already expired
    ///     ...
    ///   });
    ///   expect(session.expiresAt.getTime()).toBeLessThan(Date.now());
    /// });
    /// ```
    #[test]
    fn should_allow_expired_session_to_exist_in_db() {
        let (admin, location) = setup_test_data();

        let expires_at = Utc::now() - Duration::seconds(1); // Already expired
        let session = MockSession::new(location.id, admin.id, expires_at);

        assert!(
            session.expires_at < Utc::now(),
            "expiresAt should be in the past"
        );
    }

    /// Test: should set isActive to true by default
    ///
    /// Original Node.js test (lines 564-575):
    /// ```js
    /// test('should set isActive to true by default', async () => {
    ///   const token = Session.generateToken();
    ///   const session = await Session.create({ ... });
    ///   expect(session.isActive).toBe(true);
    /// });
    /// ```
    #[test]
    fn should_set_is_active_to_true_by_default() {
        let (admin, location) = setup_test_data();

        let session = MockSession::new(location.id, admin.id, Utc::now() + Duration::minutes(30));

        assert!(session.is_active, "isActive should default to true");
    }

    /// Test: should allow setting isActive to false
    ///
    /// Original Node.js test (lines 577-589):
    /// ```js
    /// test('should allow setting isActive to false', async () => {
    ///   const token = Session.generateToken();
    ///   const session = await Session.create({
    ///     isActive: false,
    ///     ...
    ///   });
    ///   expect(session.isActive).toBe(false);
    /// });
    /// ```
    #[test]
    fn should_allow_setting_is_active_to_false() {
        let (admin, location) = setup_test_data();

        let session = MockSession::new_with_active_status(
            location.id,
            admin.id,
            Utc::now() + Duration::minutes(30),
            false, // isActive: false
        );

        assert!(!session.is_active, "isActive should be false");
    }
}
