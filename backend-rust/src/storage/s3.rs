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
