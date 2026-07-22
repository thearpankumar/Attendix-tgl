mod s3;

pub use s3::S3Storage;

use crate::config::S3Config;
use crate::error::AppError;
use async_trait::async_trait;

#[async_trait]
pub trait StorageProvider: Send + Sync {
    async fn upload(
        &self,
        file: &[u8],
        key: &str,
        content_type: &str,
    ) -> Result<UploadResult, AppError>;
    async fn delete(&self, key: &str) -> Result<(), AppError>;
    fn get_file_url(&self, key: &str) -> String;
    async fn get_upload_url(
        &self,
        key: &str,
        content_type: &str,
    ) -> Result<PresignedUrlResult, AppError>;
    async fn get_download_url(&self, key: &str, expires_in: u32) -> Result<String, AppError>;
    async fn download(&self, key: &str) -> Result<Vec<u8>, AppError>;
    async fn list_objects(&self, limit: u32) -> Result<Vec<String>, AppError>;
    fn get_name(&self) -> &'static str;
}

#[derive(Debug, Clone)]
pub struct UploadResult {
    pub url: String,
    pub public_id: String,
    pub provider: String,
}

#[derive(Debug, Clone)]
pub struct PresignedUrlResult {
    pub upload_url: String,
    pub public_id: String,
    pub method: String,
    pub content_type: String,
    pub headers: Vec<(String, String)>,
}

pub fn create_storage_provider(
    aws_config: &aws_config::SdkConfig,
    config: &S3Config,
) -> Result<Arc<dyn StorageProvider>, AppError> {
    if config.bucket.is_empty()
        || config.access_key_id.is_empty()
        || config.secret_access_key.is_empty()
    {
        return Err(AppError::Internal(
            "S3 configuration incomplete. Required: bucket, access_key_id, secret_access_key"
                .to_string(),
        ));
    }
    Ok(Arc::new(S3Storage::new(aws_config, config.clone())))
}

use std::sync::Arc;

pub struct Storage {
    provider: Arc<dyn StorageProvider>,
}

impl Clone for Storage {
    fn clone(&self) -> Self {
        Self {
            provider: Arc::clone(&self.provider),
        }
    }
}

impl Storage {
    pub fn new(
        aws_config: &aws_config::SdkConfig,
        config: &crate::config::StorageConfig,
    ) -> Result<Self, AppError> {
        let provider = create_storage_provider(aws_config, &config.s3)?;
        Ok(Self { provider })
    }

    pub fn provider(&self) -> &dyn StorageProvider {
        self.provider.as_ref()
    }
}
