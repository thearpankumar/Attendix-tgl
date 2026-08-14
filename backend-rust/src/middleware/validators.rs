use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use validator::Validate;

static ALPHANUMERIC_REGEX: once_cell::sync::Lazy<Regex> =
    once_cell::sync::Lazy::new(|| Regex::new(r"^[a-zA-Z0-9]+$").unwrap());

static ROLL_NUMBER_REGEX: once_cell::sync::Lazy<Regex> =
    once_cell::sync::Lazy::new(|| Regex::new(r"^[a-zA-Z0-9]+$").unwrap());

static EMAIL_REGEX: once_cell::sync::Lazy<Regex> = once_cell::sync::Lazy::new(|| {
    Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap()
});

static BASE64_IMAGE_REGEX: once_cell::sync::Lazy<Regex> = once_cell::sync::Lazy::new(|| {
    Regex::new(r"^data:image/[a-zA-Z]+;base64,[A-Za-z0-9+/=]+$").unwrap()
});

pub fn is_alphanumeric(s: &str) -> bool {
    ALPHANUMERIC_REGEX.is_match(s)
}

pub fn is_valid_email(s: &str) -> bool {
    EMAIL_REGEX.is_match(s)
}

/// Validates that a value is a valid UUID (entity IDs were migrated from
/// MongoDB ObjectIds to Postgres UUIDs; name kept to avoid churn at call sites).
pub fn is_valid_objectid(s: &str) -> bool {
    uuid::Uuid::parse_str(s).is_ok()
}

/// Validates that a string is a valid base64 image data URI
pub fn is_valid_base64_image(s: &str) -> bool {
    BASE64_IMAGE_REGEX.is_match(s)
}

#[derive(Debug, Serialize)]
pub struct ValidationError {
    pub message: String,
    pub errors: Vec<FieldError>,
}

#[derive(Debug, Serialize)]
pub struct FieldError {
    pub field: String,
    pub message: String,
}

impl IntoResponse for ValidationError {
    fn into_response(self) -> Response {
        (StatusCode::BAD_REQUEST, Json(self)).into_response()
    }
}

impl ValidationError {
    pub fn new(message: &str, errors: Vec<FieldError>) -> Self {
        Self {
            message: message.to_string(),
            errors,
        }
    }

    pub fn single(field: &str, message: &str) -> Self {
        Self {
            message: "Validation failed".to_string(),
            errors: vec![FieldError {
                field: field.to_string(),
                message: message.to_string(),
            }],
        }
    }
}

/// Admin Registration Request Validator
///
/// # Validations:
/// - username: length 3-30, alphanumeric
/// - email: valid email format, normalized to lowercase
/// - password: min 6 characters
/// - admin_secret: required (non-empty)
#[derive(Debug, Deserialize, Validate)]
pub struct AdminRegisterRequest {
    #[validate(length(min = 3, max = 30, message = "Username must be 3-30 characters"))]
    pub username: String,

    #[validate(email(message = "Valid email required"))]
    pub email: String,

    #[validate(length(min = 6, message = "Password must be at least 6 characters"))]
    pub password: String,

    #[validate(length(min = 1, message = "Admin secret required"))]
    pub admin_secret: String,
}

impl AdminRegisterRequest {
    /// Validate and normalize the request
    /// Returns Ok(()) if all validations pass
    /// Returns Err(ValidationError) with all field errors if any validation fails
    pub fn validate_and_normalize(&mut self) -> Result<(), ValidationError> {
        let mut errors = Vec::new();

        // Pre-normalize to ensure validation works correctly
        let username = self.username.trim();
        let email = self.email.trim();

        // Validate username length
        if username.len() < 3 || username.len() > 30 {
            errors.push(FieldError {
                field: "username".to_string(),
                message: "Username must be 3-30 characters".to_string(),
            });
        }

        // Validate username is alphanumeric
        if !is_alphanumeric(username) {
            errors.push(FieldError {
                field: "username".to_string(),
                message: "Username must be alphanumeric".to_string(),
            });
        }

        // Validate email format (after trimming)
        if !is_valid_email(email) {
            errors.push(FieldError {
                field: "email".to_string(),
                message: "Valid email required".to_string(),
            });
        }

        // Validate password length
        if self.password.len() < 6 {
            errors.push(FieldError {
                field: "password".to_string(),
                message: "Password must be at least 6 characters".to_string(),
            });
        }

        // Validate admin_secret is not empty
        if self.admin_secret.trim().is_empty() {
            errors.push(FieldError {
                field: "admin_secret".to_string(),
                message: "Admin secret required".to_string(),
            });
        }

        if errors.is_empty() {
            // Normalize after validation passes
            self.email = normalize_email(&self.email);
            self.username = username.to_string();
            Ok(())
        } else {
            Err(ValidationError::new("Validation failed", errors))
        }
    }
}

/// Admin Login Request Validator
///
/// # Validations:
/// - username: required, non-empty
/// - password: required, non-empty
#[derive(Debug, Deserialize, Validate)]
pub struct AdminLoginRequest {
    #[validate(length(min = 1, message = "Username required"))]
    pub username: String,

    #[validate(length(min = 1, message = "Password required"))]
    pub password: String,
}

impl AdminLoginRequest {
    /// Validate the login request
    pub fn validate_request(&self) -> Result<(), ValidationError> {
        let mut errors = Vec::new();

        if self.username.trim().is_empty() {
            errors.push(FieldError {
                field: "username".to_string(),
                message: "Username required".to_string(),
            });
        }

        if self.password.is_empty() {
            errors.push(FieldError {
                field: "password".to_string(),
                message: "Password required".to_string(),
            });
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(ValidationError::new("Validation failed", errors))
        }
    }
}

/// Location Create/Update Request Validator
///
/// # Validations:
/// - name: length 1-100, XSS escape (no HTML)
/// - latitude: range -90 to 90
/// - longitude: range -180 to 180
/// - radius_meters: range 10-10000
#[derive(Debug, Deserialize, Validate)]
pub struct LocationCreateRequest {
    #[validate(length(min = 1, max = 100, message = "Location name must be 1-100 characters"))]
    pub name: String,

    #[validate(range(min = -90.0, max = 90.0, message = "Valid latitude required"))]
    pub latitude: f64,

    #[validate(range(min = -180.0, max = 180.0, message = "Valid longitude required"))]
    pub longitude: f64,

    #[validate(range(min = 10, max = 10000, message = "Radius must be 10-10000 meters"))]
    pub radius_meters: Option<i32>,
}

impl LocationCreateRequest {
    /// Sanitize the name field to prevent XSS attacks
    pub fn sanitize(&mut self) {
        self.name = sanitize_string(&self.name);
    }

    /// Validate and sanitize the request
    pub fn validate_and_sanitize(&mut self) -> Result<(), ValidationError> {
        // First apply standard validation
        validate_request(self)?;

        // Then sanitize to prevent XSS
        self.sanitize();

        Ok(())
    }
}

/// Session Create Request Validator
///
/// Two shapes share this one endpoint:
/// - **Normal session** (the original QR/link attendance flow): `batch_id`
///   stays optional, exactly as before this feature existed.
/// - **Exam session** (mentor-assignment flow): signalled by a present,
///   non-empty `assigned_admin_id`. When that's set, `batch_id`,
///   `college_name`, and `starts_at` all become required together — a
///   session assigned to a mentor must have a roster, a college, and a
///   scheduled time.
///
/// # Validations:
/// - location_id: valid UUID if provided; required unless assigned_admin_ids is set
/// - duration_minutes: range 5-480
/// - batch_id: valid UUID if provided; required if assigned_admin_ids is set
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct SessionCreateRequest {
    /// Required for a normal self-service session (geofenced); irrelevant
    /// for an exam session, which is marked manually and has no location.
    pub location_id: Option<String>,

    #[validate(range(min = 5, max = 480, message = "Duration must be 5-480 minutes"))]
    pub duration_minutes: Option<i32>,

    pub batch_id: Option<String>,
    /// One or more mentor IDs. Present (non-empty) + valid signals an exam
    /// session — see `SessionCreateRequest::is_exam_session`.
    #[serde(default)]
    pub assigned_admin_ids: Vec<String>,
    pub college_name: Option<String>,
    pub starts_at: Option<String>,

    pub description: Option<String>,
}

impl SessionCreateRequest {
    /// True when this request is creating an exam session (mentor-assigned,
    /// mandatory batch/college/time, no location) rather than a normal
    /// self-service attendance session (optional batch, geofenced, no mentor).
    pub fn is_exam_session(&self) -> bool {
        self.assigned_admin_ids.iter().any(|s| !s.trim().is_empty())
    }

    /// Validate the session request including ObjectId format checks
    pub fn validate_with_objectids(&self) -> Result<(), ValidationError> {
        let mut errors = Vec::new();

        // Validate duration_minutes range
        if let Some(duration) = self.duration_minutes {
            if !(5..=480).contains(&duration) {
                errors.push(FieldError {
                    field: "duration_minutes".to_string(),
                    message: "Duration must be 5-480 minutes".to_string(),
                });
            }
        }

        if self.is_exam_session() {
            // Exam session: batch/mentors/college/start-time are required
            // together; location does not apply (manual attendance, no
            // geofencing) so it is never validated here.
            match self.batch_id.as_deref() {
                Some(id) if is_valid_objectid(id) => {}
                _ => errors.push(FieldError {
                    field: "batch_id".to_string(),
                    message: "Valid batch ID required for an exam session".to_string(),
                }),
            }

            let mentor_ids: Vec<&str> = self
                .assigned_admin_ids
                .iter()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();
            if mentor_ids.is_empty() {
                errors.push(FieldError {
                    field: "assigned_admin_ids".to_string(),
                    message: "At least one mentor is required for an exam session".to_string(),
                });
            } else {
                let mut seen = std::collections::HashSet::new();
                for id in &mentor_ids {
                    if !is_valid_objectid(id) {
                        errors.push(FieldError {
                            field: "assigned_admin_ids".to_string(),
                            message: format!("Invalid mentor ID: {}", id),
                        });
                    } else if !seen.insert(*id) {
                        errors.push(FieldError {
                            field: "assigned_admin_ids".to_string(),
                            message: "Duplicate mentor in mentor list".to_string(),
                        });
                    }
                }
            }

            if self
                .college_name
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
            {
                errors.push(FieldError {
                    field: "college_name".to_string(),
                    message: "College name is required for an exam session".to_string(),
                });
            }
            if self
                .starts_at
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
            {
                errors.push(FieldError {
                    field: "starts_at".to_string(),
                    message: "Start time is required for an exam session".to_string(),
                });
            }
        } else {
            // Normal session: location is mandatory (geofenced check-in).
            match self.location_id.as_deref() {
                Some(id) if is_valid_objectid(id) => {}
                _ => errors.push(FieldError {
                    field: "location_id".to_string(),
                    message: "Valid location ID required".to_string(),
                }),
            }
            // Batch stays optional, exactly as before.
            if let Some(ref batch_id) = self.batch_id {
                if !batch_id.is_empty() && !is_valid_objectid(batch_id) {
                    errors.push(FieldError {
                        field: "batch_id".to_string(),
                        message: "Valid batch ID required".to_string(),
                    });
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(ValidationError::new("Validation failed", errors))
        }
    }
}

/// Attendance Submit Request Validator
///
/// # Validations:
/// - student_name: length 2-100, XSS escape
/// - roll_number: alphanumeric, max 20 chars
/// - latitude: range -90 to 90
/// - longitude: range -180 to 180
/// - photo: base64 data URI format (data:image/...)
#[derive(Debug, Deserialize)]
pub struct AttendanceSubmitRequest {
    pub student_name: String,
    pub roll_number: String,
    pub latitude: f64,
    pub longitude: f64,
    pub photo: Option<String>,
    pub direct_upload: Option<bool>,
    pub public_id: Option<String>,
    pub face_detected: Option<bool>,
    pub captcha_answer: Option<String>,
    pub captcha_id: Option<String>,
    pub device_fingerprint: Option<String>,
    pub user_agent: Option<String>,
    pub gps_data: Option<GpsData>,
}

impl AttendanceSubmitRequest {
    /// Sanitize fields to prevent XSS attacks
    pub fn sanitize(&mut self) {
        self.student_name = sanitize_string(&self.student_name);
        self.roll_number = sanitize_string(&self.roll_number);
    }

    /// Validate all fields
    pub fn validate(&self) -> Result<(), ValidationError> {
        let mut errors = Vec::new();

        // Validate student_name: length 2-100
        if self.student_name.len() < 2 || self.student_name.len() > 100 {
            errors.push(FieldError {
                field: "student_name".to_string(),
                message: "Name must be 2-100 characters".to_string(),
            });
        }

        // Validate roll_number: alphanumeric, max 20 chars
        if self.roll_number.is_empty() || self.roll_number.len() > 20 {
            errors.push(FieldError {
                field: "roll_number".to_string(),
                message: "Roll number required (max 20 characters)".to_string(),
            });
        } else if !ROLL_NUMBER_REGEX.is_match(&self.roll_number) {
            errors.push(FieldError {
                field: "roll_number".to_string(),
                message: "Roll number must be alphanumeric".to_string(),
            });
        }

        // Validate latitude: range -90 to 90
        if self.latitude < -90.0 || self.latitude > 90.0 {
            errors.push(FieldError {
                field: "latitude".to_string(),
                message: "Valid latitude required".to_string(),
            });
        }

        // Validate longitude: range -180 to 180
        if self.longitude < -180.0 || self.longitude > 180.0 {
            errors.push(FieldError {
                field: "longitude".to_string(),
                message: "Valid longitude required".to_string(),
            });
        }

        // Validate photo based on upload mode
        match self.direct_upload {
            Some(true) => {
                // Direct upload mode: public_id is required
                if self.public_id.is_none()
                    || self.public_id.as_ref().is_none_or(|id| id.is_empty())
                {
                    errors.push(FieldError {
                        field: "public_id".to_string(),
                        message: "publicId required when using direct upload".to_string(),
                    });
                }
            }
            _ => {
                // Regular upload mode: photo is required and must be base64 data URI
                match &self.photo {
                    None => {
                        errors.push(FieldError {
                            field: "photo".to_string(),
                            message: "Photo required".to_string(),
                        });
                    }
                    Some(photo) => {
                        if !photo.starts_with("data:image/") {
                            errors.push(FieldError {
                                field: "photo".to_string(),
                                message: "Invalid photo format - must be base64 data URI"
                                    .to_string(),
                            });
                        }
                    }
                }
            }
        }

        // Validate face_detected if provided (already validated as boolean by serde)
        if let Some(face_detected) = self.face_detected {
            let _ = face_detected; // Already a boolean
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(ValidationError::new("Validation failed", errors))
        }
    }

    /// Validate and sanitize the request
    pub fn validate_and_sanitize(&mut self) -> Result<(), ValidationError> {
        self.validate()?;
        self.sanitize();
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct GpsData {
    pub accuracy: Option<f64>,
    pub altitude: Option<f64>,
    pub speed: Option<f64>,
    pub timestamp: Option<i64>,
    pub mock_location: Option<bool>,
    pub provider: Option<String>,
}

/// Generic validation function for types implementing Validate trait
pub fn validate_request<T: Validate>(payload: &T) -> Result<(), ValidationError> {
    match payload.validate() {
        Ok(_) => Ok(()),
        Err(errors) => {
            let field_errors: Vec<FieldError> = errors
                .field_errors()
                .iter()
                .flat_map(|(field, errs)| {
                    errs.iter().map(|e| FieldError {
                        field: field.to_string(),
                        message: e.message.clone().unwrap_or_default().to_string(),
                    })
                })
                .collect();

            Err(ValidationError::new("Validation failed", field_errors))
        }
    }
}

/// XSS Prevention: Escape HTML entities
///
/// Escapes the following characters:
/// - `&` → `&amp;`
/// - `<` → `&lt;`
/// - `>` → `&gt;`
/// - `"` → `&quot;`
/// - `'` → `&#x27;`
/// - `/` → `&#x2F;`
pub fn sanitize_string(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
        .replace('/', "&#x2F;")
}

/// Normalize email to lowercase and trim whitespace
pub fn normalize_email(email: &str) -> String {
    email.to_lowercase().trim().to_string()
}

/// Validate and normalize email format
pub fn validate_email(email: &str) -> Result<String, ValidationError> {
    let normalized = normalize_email(email);
    if !is_valid_email(&normalized) {
        return Err(ValidationError::single("email", "Valid email required"));
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_admin_register() {
        let mut valid = AdminRegisterRequest {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password: "password123".to_string(),
            admin_secret: "secret".to_string(),
        };
        assert!(valid.validate_and_normalize().is_ok());
        assert_eq!(valid.email, "test@example.com");

        // Invalid: username too short
        let mut invalid_short = AdminRegisterRequest {
            username: "ab".to_string(),
            email: "test@example.com".to_string(),
            password: "password123".to_string(),
            admin_secret: "secret".to_string(),
        };
        assert!(invalid_short.validate_and_normalize().is_err());

        // Invalid: username contains special characters
        let mut invalid_chars = AdminRegisterRequest {
            username: "test-user".to_string(),
            email: "test@example.com".to_string(),
            password: "password123".to_string(),
            admin_secret: "secret".to_string(),
        };
        assert!(invalid_chars.validate_and_normalize().is_err());

        // Invalid: empty admin_secret
        let mut invalid_secret = AdminRegisterRequest {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password: "password123".to_string(),
            admin_secret: "".to_string(),
        };
        assert!(invalid_secret.validate_and_normalize().is_err());
    }

    #[test]
    fn test_admin_register_email_normalization() {
        let mut req = AdminRegisterRequest {
            username: "testuser".to_string(),
            email: "  TEST@EXAMPLE.COM  ".to_string(),
            password: "password123".to_string(),
            admin_secret: "secret".to_string(),
        };
        req.validate_and_normalize().unwrap();
        assert_eq!(req.email, "test@example.com");
    }

    #[test]
    fn test_validate_admin_login() {
        let valid = AdminLoginRequest {
            username: "testuser".to_string(),
            password: "password123".to_string(),
        };
        assert!(valid.validate_request().is_ok());

        // Invalid: empty username
        let invalid = AdminLoginRequest {
            username: "".to_string(),
            password: "password123".to_string(),
        };
        assert!(invalid.validate_request().is_err());

        // Invalid: whitespace-only username
        let invalid_ws = AdminLoginRequest {
            username: "   ".to_string(),
            password: "password123".to_string(),
        };
        assert!(invalid_ws.validate_request().is_err());
    }

    #[test]
    fn test_validate_session_create() {
        // ── Normal session (no assigned_admin_ids): location mandatory, batch optional ──
        let normal_no_batch = SessionCreateRequest {
            location_id: Some("550e8400-e29b-41d4-a716-446655440000".to_string()),
            duration_minutes: Some(30),
            batch_id: None,
            assigned_admin_ids: vec![],
            college_name: None,
            starts_at: None,
            description: None,
        };
        assert!(!normal_no_batch.is_exam_session());
        assert!(
            normal_no_batch.validate_with_objectids().is_ok(),
            "normal session with no batch should be allowed"
        );

        let mut normal_with_batch = normal_no_batch.clone();
        normal_with_batch.batch_id = Some("550e8400-e29b-41d4-a716-446655440001".to_string());
        assert!(
            normal_with_batch.validate_with_objectids().is_ok(),
            "normal session with a valid batch should be allowed"
        );

        let mut normal_invalid_batch = normal_no_batch.clone();
        normal_invalid_batch.batch_id = Some("invalid".to_string());
        assert!(
            normal_invalid_batch.validate_with_objectids().is_err(),
            "normal session with a malformed batch id should still be rejected"
        );

        // Invalid location UUID (normal session only — location isn't checked for exam sessions)
        let invalid_location = SessionCreateRequest {
            location_id: Some("invalid-id".to_string()),
            duration_minutes: Some(30),
            batch_id: None,
            assigned_admin_ids: vec![],
            college_name: None,
            starts_at: None,
            description: None,
        };
        assert!(invalid_location.validate_with_objectids().is_err());

        // Missing location entirely on a normal session
        let missing_location = SessionCreateRequest {
            location_id: None,
            ..invalid_location.clone()
        };
        assert!(
            missing_location.validate_with_objectids().is_err(),
            "normal session requires a location"
        );

        // Invalid duration (out of range)
        let invalid_duration = SessionCreateRequest {
            location_id: Some("550e8400-e29b-41d4-a716-446655440000".to_string()),
            duration_minutes: Some(500), // Max is 480
            batch_id: None,
            assigned_admin_ids: vec![],
            college_name: None,
            starts_at: None,
            description: None,
        };
        assert!(invalid_duration.validate_with_objectids().is_err());

        // ── Exam session (assigned_admin_ids non-empty): batch/mentors/college/time
        // required, location does not apply ──
        let valid_exam = SessionCreateRequest {
            location_id: None,
            duration_minutes: Some(30),
            batch_id: Some("550e8400-e29b-41d4-a716-446655440001".to_string()),
            assigned_admin_ids: vec!["550e8400-e29b-41d4-a716-446655440002".to_string()],
            college_name: Some("Example College".to_string()),
            starts_at: Some("2026-08-14T09:00:00Z".to_string()),
            description: None,
        };
        assert!(valid_exam.is_exam_session());
        assert!(
            valid_exam.validate_with_objectids().is_ok(),
            "exam session needs no location"
        );

        // Multiple mentors is valid
        let multi_mentor = SessionCreateRequest {
            assigned_admin_ids: vec![
                "550e8400-e29b-41d4-a716-446655440002".to_string(),
                "550e8400-e29b-41d4-a716-446655440003".to_string(),
            ],
            ..valid_exam.clone()
        };
        assert!(
            multi_mentor.validate_with_objectids().is_ok(),
            "exam session may have more than one mentor"
        );

        // Duplicate mentor in the list is rejected
        let duplicate_mentor = SessionCreateRequest {
            assigned_admin_ids: vec![
                "550e8400-e29b-41d4-a716-446655440002".to_string(),
                "550e8400-e29b-41d4-a716-446655440002".to_string(),
            ],
            ..valid_exam.clone()
        };
        assert!(
            duplicate_mentor.validate_with_objectids().is_err(),
            "duplicate mentor id should be rejected"
        );

        // Exam session missing batch_id
        let exam_missing_batch = SessionCreateRequest {
            batch_id: None,
            ..valid_exam.clone()
        };
        assert!(
            exam_missing_batch.validate_with_objectids().is_err(),
            "exam session requires a batch"
        );

        // Exam session with no mentors at all
        let exam_no_mentors = SessionCreateRequest {
            assigned_admin_ids: vec![],
            ..valid_exam.clone()
        };
        assert!(
            !exam_no_mentors.is_exam_session(),
            "empty mentor list is not an exam session"
        );

        // Exam session with an invalid mentor id mixed into an otherwise valid list
        let exam_invalid_mentor = SessionCreateRequest {
            assigned_admin_ids: vec!["invalid".to_string()],
            ..valid_exam.clone()
        };
        assert!(exam_invalid_mentor.validate_with_objectids().is_err());

        // Exam session missing college_name
        let exam_missing_college = SessionCreateRequest {
            college_name: None,
            ..valid_exam.clone()
        };
        assert!(exam_missing_college.validate_with_objectids().is_err());

        // Exam session missing starts_at
        let exam_missing_starts_at = SessionCreateRequest {
            starts_at: None,
            ..valid_exam.clone()
        };
        assert!(exam_missing_starts_at.validate_with_objectids().is_err());
    }

    #[test]
    fn test_validate_location() {
        let mut valid = LocationCreateRequest {
            name: "Test Location".to_string(),
            latitude: 40.7128,
            longitude: -74.0060,
            radius_meters: Some(100),
        };
        assert!(valid.validate_and_sanitize().is_ok());

        // Invalid latitude
        let mut invalid_lat = LocationCreateRequest {
            name: "Test Location".to_string(),
            latitude: 100.0,
            longitude: -74.0060,
            radius_meters: Some(100),
        };
        assert!(invalid_lat.validate_and_sanitize().is_err());

        // Invalid radius
        let mut invalid_radius = LocationCreateRequest {
            name: "Test Location".to_string(),
            latitude: 40.7128,
            longitude: -74.0060,
            radius_meters: Some(5), // Min is 10
        };
        assert!(invalid_radius.validate_and_sanitize().is_err());
    }

    #[test]
    fn test_location_xss_sanitization() {
        let mut req = LocationCreateRequest {
            name: "<script>alert('xss')</script>Location".to_string(),
            latitude: 40.7128,
            longitude: -74.0060,
            radius_meters: Some(100),
        };
        req.validate_and_sanitize().unwrap();
        assert!(req.name.contains("&lt;"));
        assert!(req.name.contains("&gt;"));
        assert!(!req.name.contains('<'));
        assert!(!req.name.contains('>'));
    }

    #[test]
    fn test_sanitize_string() {
        assert_eq!(
            sanitize_string("<script>alert('xss')</script>"),
            "&lt;script&gt;alert(&#x27;xss&#x27;)&lt;&#x2F;script&gt;"
        );

        // Test all HTML entities
        assert_eq!(sanitize_string("&"), "&amp;");
        assert_eq!(sanitize_string("<"), "&lt;");
        assert_eq!(sanitize_string(">"), "&gt;");
        assert_eq!(sanitize_string("\""), "&quot;");
        assert_eq!(sanitize_string("'"), "&#x27;");
        assert_eq!(sanitize_string("/"), "&#x2F;");
    }

    #[test]
    fn test_is_valid_objectid() {
        assert!(is_valid_objectid("550e8400-e29b-41d4-a716-446655440000"));
        assert!(!is_valid_objectid("invalid"));
        assert!(!is_valid_objectid("507f1f77bcf86cd799439011")); // old Mongo ObjectId format
        assert!(!is_valid_objectid("550e8400-e29b-41d4-a716-44665544000")); // truncated UUID
    }

    #[test]
    fn test_normalize_email() {
        assert_eq!(normalize_email("  TEST@EXAMPLE.COM  "), "test@example.com");
        assert_eq!(
            normalize_email("Test.User@Example.Com"),
            "test.user@example.com"
        );
    }

    #[test]
    fn test_is_valid_email() {
        assert!(is_valid_email("test@example.com"));
        assert!(is_valid_email("test.user@example.com"));
        assert!(is_valid_email("test+label@example.org"));
        assert!(!is_valid_email("invalid"));
        assert!(!is_valid_email("test@"));
        assert!(!is_valid_email("@example.com"));
    }

    #[test]
    fn test_is_valid_base64_image() {
        assert!(is_valid_base64_image("data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg=="));
        assert!(!is_valid_base64_image("hello world"));
        assert!(!is_valid_base64_image(
            "data:text/plain;base64,SGVsbG8gV29ybGQ="
        ));
    }

    #[test]
    fn test_attendance_validation_with_photo() {
        // Valid with base64 image
        let valid = AttendanceSubmitRequest {
            student_name: "John Doe".to_string(),
            roll_number: "ABC123".to_string(),
            latitude: 40.7128,
            longitude: -74.0060,
            photo: Some("data:image/png;base64,iVBORw0KGgo=".to_string()),
            direct_upload: None,
            public_id: None,
            face_detected: None,
            captcha_answer: None,
            captcha_id: None,
            device_fingerprint: None,
            user_agent: None,
            gps_data: None,
        };
        assert!(valid.validate().is_ok());

        // Invalid photo format
        let invalid_photo = AttendanceSubmitRequest {
            student_name: "John Doe".to_string(),
            roll_number: "ABC123".to_string(),
            latitude: 40.7128,
            longitude: -74.0060,
            photo: Some("invalid-format".to_string()),
            direct_upload: None,
            public_id: None,
            face_detected: None,
            captcha_answer: None,
            captcha_id: None,
            device_fingerprint: None,
            user_agent: None,
            gps_data: None,
        };
        assert!(invalid_photo.validate().is_err());

        // Direct upload mode without photo
        let direct_upload = AttendanceSubmitRequest {
            student_name: "John Doe".to_string(),
            roll_number: "ABC123".to_string(),
            latitude: 40.7128,
            longitude: -74.0060,
            photo: None,
            direct_upload: Some(true),
            public_id: Some("public-id-123".to_string()),
            face_detected: None,
            captcha_answer: None,
            captcha_id: None,
            device_fingerprint: None,
            user_agent: None,
            gps_data: None,
        };
        assert!(direct_upload.validate().is_ok());

        // Direct upload mode without public_id should fail
        let invalid_direct = AttendanceSubmitRequest {
            student_name: "John Doe".to_string(),
            roll_number: "ABC123".to_string(),
            latitude: 40.7128,
            longitude: -74.0060,
            photo: None,
            direct_upload: Some(true),
            public_id: None,
            face_detected: None,
            captcha_answer: None,
            captcha_id: None,
            device_fingerprint: None,
            user_agent: None,
            gps_data: None,
        };
        assert!(invalid_direct.validate().is_err());
    }

    #[test]
    fn test_attendance_xss_sanitization() {
        let mut req = AttendanceSubmitRequest {
            student_name: "<script>alert('xss')</script>".to_string(),
            roll_number: "ABC123".to_string(),
            latitude: 40.7128,
            longitude: -74.0060,
            photo: Some("data:image/png;base64,abc".to_string()),
            direct_upload: None,
            public_id: None,
            face_detected: None,
            captcha_answer: None,
            captcha_id: None,
            device_fingerprint: None,
            user_agent: None,
            gps_data: None,
        };
        req.validate_and_sanitize().unwrap();
        assert!(req.student_name.contains("&lt;"));
        assert!(!req.student_name.contains('<'));
    }

    #[test]
    fn test_attendance_validation_bounds() {
        // Invalid student_name (too short)
        let invalid_name = AttendanceSubmitRequest {
            student_name: "J".to_string(),
            roll_number: "ABC123".to_string(),
            latitude: 40.7128,
            longitude: -74.0060,
            photo: Some("data:image/png;base64,abc".to_string()),
            direct_upload: None,
            public_id: None,
            face_detected: None,
            captcha_answer: None,
            captcha_id: None,
            device_fingerprint: None,
            user_agent: None,
            gps_data: None,
        };
        assert!(invalid_name.validate().is_err());

        // Invalid roll_number (special characters)
        let invalid_roll = AttendanceSubmitRequest {
            student_name: "John Doe".to_string(),
            roll_number: "ABC-123".to_string(), // Contains dash
            latitude: 40.7128,
            longitude: -74.0060,
            photo: Some("data:image/png;base64,abc".to_string()),
            direct_upload: None,
            public_id: None,
            face_detected: None,
            captcha_answer: None,
            captcha_id: None,
            device_fingerprint: None,
            user_agent: None,
            gps_data: None,
        };
        assert!(invalid_roll.validate().is_err());

        // Invalid latitude
        let invalid_lat = AttendanceSubmitRequest {
            student_name: "John Doe".to_string(),
            roll_number: "ABC123".to_string(),
            latitude: 100.0, // Out of range
            longitude: -74.0060,
            photo: Some("data:image/png;base64,abc".to_string()),
            direct_upload: None,
            public_id: None,
            face_detected: None,
            captcha_answer: None,
            captcha_id: None,
            device_fingerprint: None,
            user_agent: None,
            gps_data: None,
        };
        assert!(invalid_lat.validate().is_err());

        // Invalid longitude
        let invalid_lng = AttendanceSubmitRequest {
            student_name: "John Doe".to_string(),
            roll_number: "ABC123".to_string(),
            latitude: 40.7128,
            longitude: 200.0, // Out of range
            photo: Some("data:image/png;base64,abc".to_string()),
            direct_upload: None,
            public_id: None,
            face_detected: None,
            captcha_answer: None,
            captcha_id: None,
            device_fingerprint: None,
            user_agent: None,
            gps_data: None,
        };
        assert!(invalid_lng.validate().is_err());
    }

    #[test]
    fn test_alphanumeric_validation() {
        assert!(is_alphanumeric("testuser123"));
        assert!(is_alphanumeric("TESTUSER"));
        assert!(is_alphanumeric("123456"));
        assert!(!is_alphanumeric("test-user"));
        assert!(!is_alphanumeric("test@user"));
        assert!(!is_alphanumeric(""));
    }
}
