use anyhow::{Context, Result};
use async_trait::async_trait;
use base64::Engine;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::traits::{OcrPage, OcrProvider, OcrResult};

// ── Mistral OCR Provider ─────────────────────────────────────────────────

/// OCR provider using the Mistral OCR API (https://api.mistral.ai/v1/ocr).
pub struct MistralOcrProvider {
    api_key: String,
    model: String,
    http: reqwest::Client,
}

impl MistralOcrProvider {
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()?;
        Ok(Self {
            api_key: api_key.into(),
            model: "mistral-ocr-latest".into(),
            http,
        })
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    fn mime_to_data_url_prefix(mime_type: &str) -> &'static str {
        match mime_type {
            "application/pdf" => "data:application/pdf;base64,",
            "image/png" => "data:image/png;base64,",
            "image/jpeg" | "image/jpg" => "data:image/jpeg;base64,",
            "image/webp" => "data:image/webp;base64,",
            "image/tiff" => "data:image/tiff;base64,",
            _ => "data:application/pdf;base64,",
        }
    }
}

#[derive(Serialize)]
struct MistralOcrRequest {
    model: String,
    document: MistralDocument,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    include_image_base64: bool,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum MistralDocument {
    #[serde(rename = "document_url")]
    DocumentUrl { document_url: String },
    #[serde(rename = "image_url")]
    ImageUrl { image_url: String },
}

#[derive(Deserialize)]
struct MistralOcrResponse {
    pages: Vec<MistralOcrPageResponse>,
    #[allow(dead_code)]
    model: String,
    #[allow(dead_code)]
    usage: Option<MistralOcrUsage>,
}

#[derive(Deserialize)]
struct MistralOcrPageResponse {
    index: usize,
    markdown: String,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct MistralOcrUsage {
    pages_processed: Option<u64>,
    doc_size_bytes: Option<u64>,
}

#[async_trait]
impl OcrProvider for MistralOcrProvider {
    async fn ocr_document(&self, data: &[u8], mime_type: &str) -> Result<OcrResult> {
        let b64 = base64::engine::general_purpose::STANDARD.encode(data);
        let prefix = Self::mime_to_data_url_prefix(mime_type);
        let data_url = format!("{prefix}{b64}");

        let is_image = mime_type.starts_with("image/");
        let document = if is_image {
            MistralDocument::ImageUrl {
                image_url: data_url,
            }
        } else {
            MistralDocument::DocumentUrl {
                document_url: data_url,
            }
        };

        let request = MistralOcrRequest {
            model: self.model.clone(),
            document,
            include_image_base64: false,
        };

        info!(model = %self.model, size_bytes = data.len(), mime = mime_type, "calling Mistral OCR");

        let response = self
            .http
            .post("https://api.mistral.ai/v1/ocr")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await
            .context("Mistral OCR request failed")?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            let truncated = if body.len() > 500 { &body[..500] } else { &body };
            anyhow::bail!("Mistral OCR error {status}: {truncated}");
        }

        let parsed: MistralOcrResponse = response
            .json()
            .await
            .context("failed to parse Mistral OCR response")?;

        let pages: Vec<OcrPage> = parsed
            .pages
            .iter()
            .map(|p| OcrPage {
                page_index: p.index,
                text: p.markdown.clone(),
            })
            .collect();

        let text = parsed
            .pages
            .iter()
            .map(|p| p.markdown.as_str())
            .collect::<Vec<_>>()
            .join("\n\n---\n\n");

        let page_count = pages.len();

        info!(pages = page_count, text_len = text.len(), "Mistral OCR complete");

        Ok(OcrResult {
            text,
            pages,
            page_count,
        })
    }

    fn name(&self) -> &str {
        "mistral"
    }

    fn is_available(&self) -> bool {
        !self.api_key.is_empty()
    }
}

// ── OCR Router ───────────────────────────────────────────────────────────

/// Routes OCR requests to the best available provider.
pub struct OcrRouter {
    providers: Vec<Box<dyn OcrProvider>>,
}

impl OcrRouter {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    pub fn with_provider(mut self, provider: Box<dyn OcrProvider>) -> Self {
        self.providers.push(provider);
        self
    }

    /// Build from environment variables. Registers all providers that have
    /// their API keys configured.
    pub fn from_env() -> Result<Self> {
        let mut router = Self::new();

        if let Ok(key) = std::env::var("MISTRAL_API_KEY") {
            if !key.is_empty() {
                let provider = MistralOcrProvider::new(key)?;
                router.providers.push(Box::new(provider));
                info!("OCR provider registered: mistral");
            }
        }

        if router.providers.is_empty() {
            warn!("no OCR providers configured — set MISTRAL_API_KEY to enable OCR");
        }

        Ok(router)
    }

    /// OCR a document using the first available provider.
    pub async fn ocr(&self, data: &[u8], mime_type: &str) -> Result<OcrResult> {
        let provider = self
            .providers
            .iter()
            .find(|p| p.is_available())
            .ok_or_else(|| anyhow::anyhow!("no OCR provider available — set MISTRAL_API_KEY"))?;

        provider.ocr_document(data, mime_type).await
    }

    /// Whether any OCR provider is available.
    pub fn is_available(&self) -> bool {
        self.providers.iter().any(|p| p.is_available())
    }

    /// Name of the primary provider (first available).
    pub fn provider_name(&self) -> Option<&str> {
        self.providers
            .iter()
            .find(|p| p.is_available())
            .map(|p| p.name())
    }
}

impl Default for OcrRouter {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a mime type is likely a scanned/image-based document that needs OCR.
pub fn needs_ocr(mime_type: &str, text: &str) -> bool {
    let is_image = mime_type.starts_with("image/");
    let is_pdf = mime_type == "application/pdf";

    if is_image {
        return true;
    }

    // PDF with very little extracted text relative to file size is likely scanned
    if is_pdf && text.trim().len() < 50 {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needs_ocr_image_always_true() {
        assert!(needs_ocr("image/png", ""));
        assert!(needs_ocr("image/jpeg", "some text"));
    }

    #[test]
    fn needs_ocr_pdf_with_no_text() {
        assert!(needs_ocr("application/pdf", ""));
        assert!(needs_ocr("application/pdf", "   "));
    }

    #[test]
    fn needs_ocr_pdf_with_text_false() {
        let long_text = "a".repeat(100);
        assert!(!needs_ocr("application/pdf", &long_text));
    }

    #[test]
    fn needs_ocr_text_file_false() {
        assert!(!needs_ocr("text/plain", "hello world"));
    }

    #[test]
    fn ocr_router_empty_not_available() {
        let router = OcrRouter::new();
        assert!(!router.is_available());
        assert!(router.provider_name().is_none());
    }

    #[test]
    fn mime_to_data_url_prefix_works() {
        assert!(MistralOcrProvider::mime_to_data_url_prefix("application/pdf").contains("pdf"));
        assert!(MistralOcrProvider::mime_to_data_url_prefix("image/png").contains("png"));
        assert!(MistralOcrProvider::mime_to_data_url_prefix("image/jpeg").contains("jpeg"));
    }
}
