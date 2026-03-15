use anyhow::{Context, Result};
use async_trait::async_trait;
use bytes::Bytes;
use chrono::Utc;

use crate::traits::{BackupExportProvider, BackupExportResult};

// ── Local Export Provider ─────────────────────────────────────────────────

/// Writes export archives to a local directory for direct download.
pub struct LocalExportProvider {
    pub export_dir: String,
}

impl LocalExportProvider {
    pub fn new(data_dir: &str) -> Self {
        Self {
            export_dir: format!("{data_dir}/exports"),
        }
    }
}

#[async_trait]
impl BackupExportProvider for LocalExportProvider {
    async fn export(&self, data: Bytes, filename: &str) -> Result<BackupExportResult> {
        tokio::fs::create_dir_all(&self.export_dir)
            .await
            .with_context(|| format!("create export dir {}", self.export_dir))?;

        let path = format!("{}/{filename}", self.export_dir);
        tokio::fs::write(&path, &data)
            .await
            .with_context(|| format!("write export file {path}"))?;

        Ok(BackupExportResult {
            provider: "local".into(),
            location: path,
            size_bytes: data.len() as u64,
            timestamp: Utc::now(),
        })
    }

    fn name(&self) -> &str {
        "local"
    }

    fn is_available(&self) -> bool {
        true
    }
}

// ── S3 Export Provider ────────────────────────────────────────────────────

/// Uploads export archives to S3 using existing backup config credentials.
pub struct S3ExportProvider {
    pub bucket: String,
    pub prefix: String,
    pub client: aws_sdk_s3::Client,
}

impl S3ExportProvider {
    pub async fn from_backup_config(config: &crate::config::BackupConfig) -> Result<Option<Self>> {
        if !config.backend.trim().eq_ignore_ascii_case("s3") {
            return Ok(None);
        }
        if config.bucket.trim().is_empty() {
            return Ok(None);
        }

        let mut loader = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(aws_config::Region::new(config.region.clone()));
        if !config.access_key.is_empty() && !config.secret_key.is_empty() {
            loader = loader.credentials_provider(aws_credential_types::Credentials::new(
                config.access_key.clone(),
                config.secret_key.clone(),
                None,
                None,
                "borg-export-config",
            ));
        }
        let shared = loader.load().await;
        let mut s3_builder = aws_sdk_s3::config::Builder::from(&shared);
        if !config.endpoint.trim().is_empty() {
            s3_builder = s3_builder
                .endpoint_url(config.endpoint.clone())
                .force_path_style(true);
        }
        let client = aws_sdk_s3::Client::from_conf(s3_builder.build());

        let mut prefix = config.prefix.trim().to_string();
        if !prefix.is_empty() && !prefix.ends_with('/') {
            prefix.push('/');
        }

        Ok(Some(Self {
            bucket: config.bucket.clone(),
            prefix,
            client,
        }))
    }
}

#[async_trait]
impl BackupExportProvider for S3ExportProvider {
    async fn export(&self, data: Bytes, filename: &str) -> Result<BackupExportResult> {
        let object_key = format!("{}exports/{filename}", self.prefix);
        let size = data.len() as u64;

        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&object_key)
            .body(aws_sdk_s3::primitives::ByteStream::from(data.to_vec()))
            .send()
            .await
            .with_context(|| format!("s3 put export object {object_key}"))?;

        Ok(BackupExportResult {
            provider: "s3".into(),
            location: format!("s3://{}/{object_key}", self.bucket),
            size_bytes: size,
            timestamp: Utc::now(),
        })
    }

    fn name(&self) -> &str {
        "s3"
    }

    fn is_available(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_provider_is_available() {
        let provider = LocalExportProvider::new("/tmp/test-data");
        assert!(provider.is_available());
        assert_eq!(provider.name(), "local");
        assert_eq!(provider.export_dir, "/tmp/test-data/exports");
    }

    #[tokio::test]
    async fn local_provider_export_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let provider = LocalExportProvider::new(tmp.path().to_str().unwrap());
        let data = Bytes::from_static(b"test archive content");
        let result = provider.export(data.clone(), "test-backup.zip").await.unwrap();
        assert_eq!(result.provider, "local");
        assert_eq!(result.size_bytes, 20);
        let read_back = tokio::fs::read(&result.location).await.unwrap();
        assert_eq!(read_back, &data[..]);
    }
}
