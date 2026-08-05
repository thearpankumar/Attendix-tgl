use crate::config::S3Config;
use crate::error::AppError;
use crate::storage::{PresignedUrlResult, StorageProvider, UploadResult};
use async_trait::async_trait;
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client;
use std::time::Duration;

pub struct S3Storage {
    client: Client,
    bucket: String,
    region: String,
}

impl S3Storage {
    pub fn new(aws_config: &aws_config::SdkConfig, config: S3Config) -> Self {
        let client = Client::new(aws_config);

        Self {
            client,
            bucket: config.bucket,
            region: config.region,
        }
    }

    pub fn generate_key(folder: &str, key: &str) -> String {
        let sanitized_key = key.replace(
            |c: char| !c.is_alphanumeric() && c != '-' && c != '_' && c != '.',
            "_",
        );
        format!("{}/{}.jpg", folder, sanitized_key)
    }
}

/// Object-key prefix every attendance photo lives under.
pub const ATTENDANCE_PHOTO_PREFIX: &str = "attendance-photos/";

/// Validates a client-supplied object key before it reaches S3.
///
/// `photo_public_id` arrives in the attendance request body and used to be
/// passed straight to `download()` (arbitrary read of any object in the
/// bucket) and later to `delete()` when a session was removed — so a submitter
/// could name another session's photo, or a database backup, and have it
/// destroyed. Keys are now confined to the attendance-photo prefix with no
/// traversal segments.
pub fn validate_attendance_photo_key(key: &str) -> Result<&str, AppError> {
    if !key.starts_with(ATTENDANCE_PHOTO_PREFIX) {
        return Err(AppError::BadRequest("Invalid photo reference".to_string()));
    }

    // `..` and absolute-looking segments would escape the prefix once S3
    // normalises the key; backslashes and control characters are never valid
    // in a key this service generates.
    if key.contains("..")
        || key.contains("//")
        || key.contains('\\')
        || key.chars().any(|c| c.is_control())
    {
        return Err(AppError::BadRequest("Invalid photo reference".to_string()));
    }

    Ok(key)
}

#[async_trait]
impl StorageProvider for S3Storage {
    async fn upload(
        &self,
        file: &[u8],
        key: &str,
        content_type: &str,
    ) -> Result<UploadResult, AppError> {
        let object_key = if key.starts_with("attendance-photos/") {
            key.to_string()
        } else {
            Self::generate_key("attendance-photos", key)
        };

        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&object_key)
            .body(ByteStream::from(file.to_vec()))
            .content_type(content_type)
            .cache_control("max-age=31536000")
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("S3 upload failed: {}", e)))?;

        let url = format!(
            "https://{}.s3.{}.amazonaws.com/{}",
            self.bucket, self.region, object_key
        );

        Ok(UploadResult {
            url,
            public_id: object_key,
            provider: "s3".to_string(),
        })
    }

    async fn delete(&self, key: &str) -> Result<(), AppError> {
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("S3 delete failed: {}", e)))?;
        Ok(())
    }

    fn get_file_url(&self, key: &str) -> String {
        format!(
            "https://{}.s3.{}.amazonaws.com/{}",
            self.bucket, self.region, key
        )
    }

    async fn get_upload_url(
        &self,
        key: &str,
        content_type: &str,
    ) -> Result<PresignedUrlResult, AppError> {
        let object_key = if key.starts_with("attendance-photos/") {
            key.to_string()
        } else {
            format!("attendance-photos/{}", key)
        };

        let expires_in = Duration::from_secs(300);

        let presigned_request = self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(&object_key)
            .content_type(content_type)
            .presigned(
                PresigningConfig::expires_in(expires_in)
                    .map_err(|e| AppError::Storage(e.to_string()))?,
            )
            .await
            .map_err(|e| AppError::Internal(format!("Failed to generate upload URL: {}", e)))?;

        Ok(PresignedUrlResult {
            upload_url: presigned_request.uri().to_string(),
            public_id: object_key,
            method: "PUT".to_string(),
            content_type: content_type.to_string(),
            headers: vec![("Content-Type".to_string(), content_type.to_string())],
        })
    }

    async fn get_download_url(&self, key: &str, expires_in: u32) -> Result<String, AppError> {
        let expires_in = Duration::from_secs(expires_in as u64);

        let presigned_request = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .presigned(
                PresigningConfig::expires_in(expires_in)
                    .map_err(|e| AppError::Storage(e.to_string()))?,
            )
            .await
            .map_err(|e| AppError::Internal(format!("Failed to generate download URL: {}", e)))?;

        Ok(presigned_request.uri().to_string())
    }

    async fn download(&self, key: &str) -> Result<Vec<u8>, AppError> {
        let response = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("S3 download failed: {}", e)))?;

        let body =
            response.body.collect().await.map_err(|e| {
                AppError::Internal(format!("Failed to read S3 response body: {}", e))
            })?;

        Ok(body.to_vec())
    }

    async fn list_objects(&self, limit: u32) -> Result<Vec<String>, AppError> {
        let response = self
            .client
            .list_objects_v2()
            .bucket(&self.bucket)
            .max_keys(limit as i32)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("S3 list objects failed: {}", e)))?;

        let keys: Vec<String> = response
            .contents()
            .iter()
            .filter_map(|obj| obj.key().map(|k| k.to_string()))
            .collect();

        Ok(keys)
    }

    fn get_name(&self) -> &'static str {
        "s3"
    }
}

#[cfg(test)]
mod key_validation_tests {
    use super::*;

    #[test]
    fn accepts_a_generated_attendance_key() {
        let key = "attendance-photos/8f14e45f-ea8d-4c2b-9f1a-0d3c5b7e9a11_1721654400.jpg";
        assert!(validate_attendance_photo_key(key).is_ok());
    }

    #[test]
    fn rejects_a_key_outside_the_prefix() {
        assert!(validate_attendance_photo_key("db-backups/dump.sql.gz").is_err());
    }

    #[test]
    fn rejects_traversal_out_of_the_prefix() {
        assert!(
            validate_attendance_photo_key("attendance-photos/../db-backups/dump.sql.gz").is_err()
        );
    }

    #[test]
    fn rejects_a_prefix_lookalike() {
        assert!(validate_attendance_photo_key("attendance-photos-evil/x.jpg").is_err());
    }

    #[test]
    fn rejects_control_characters() {
        assert!(validate_attendance_photo_key("attendance-photos/a\nb.jpg").is_err());
    }
}
