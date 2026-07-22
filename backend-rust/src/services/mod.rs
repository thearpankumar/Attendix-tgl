pub mod face_detection;
pub mod gps_history_service;
pub mod ip_lookup;
pub mod system_health;

pub use face_detection::{
    check_photo_reuse, compare_hashes, compute_image_hash, detect_faces, init_face_detector,
    FaceDetectionResult,
};
pub use gps_history_service::{GpsAnomaly, GpsHistoryService, GpsPositionEntry};
pub use ip_lookup::{lookup_ip, IpInfo, IpLookupService};
pub use system_health::{
    check_database, check_redis, check_storage, get_system_health, ComponentHealth, HealthStatus,
    SystemHealth,
};
