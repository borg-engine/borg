use std::collections::HashMap;

use anyhow::{Context, Result};
use async_trait::async_trait;
use tracing::{info, warn};

use crate::traits::{ScrapeProvider, ScrapeResult};

// ── Simple HTTP Scraper ──────────────────────────────────────────────────

/// Scrapes web pages using reqwest + html2md. Handles static HTML pages.
/// No JavaScript rendering — use FirecrawlScraper for JS-heavy sites.
pub struct SimpleHttpScraper {
    http: reqwest::Client,
}

impl SimpleHttpScraper {
    pub fn new() -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("Mozilla/5.0 (compatible; BorgBot/1.0)")
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()?;
        Ok(Self { http })
    }
}

#[async_trait]
impl ScrapeProvider for SimpleHttpScraper {
    async fn scrape(&self, url: &str) -> Result<ScrapeResult> {
        info!(url, "scraping page");

        let response = self
            .http
            .get(url)
            .send()
            .await
            .context("HTTP request failed")?;

        let final_url = response.url().to_string();
        let status = response.status();
        if !status.is_success() {
            anyhow::bail!("HTTP {status} from {url}");
        }

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        // Reject non-HTML content
        if !content_type.is_empty()
            && !content_type.contains("html")
            && !content_type.contains("text")
        {
            anyhow::bail!("non-HTML content type: {content_type}");
        }

        let html = response.text().await.context("failed to read response body")?;

        let title = extract_title(&html);
        let cleaned = clean_html(&html);
        let markdown = html2md::parse_html(&cleaned);

        // Post-process: collapse excessive whitespace
        let markdown = collapse_whitespace(&markdown);

        let mut metadata = HashMap::new();
        if let Some(desc) = extract_meta(&html, "description") {
            metadata.insert("description".into(), desc);
        }
        if !content_type.is_empty() {
            metadata.insert("content_type".into(), content_type);
        }

        info!(
            url,
            title = title.as_deref().unwrap_or(""),
            markdown_len = markdown.len(),
            "scrape complete"
        );

        Ok(ScrapeResult {
            markdown,
            title,
            url: final_url,
            metadata,
        })
    }

    fn name(&self) -> &str {
        "simple-http"
    }

    fn is_available(&self) -> bool {
        true
    }
}

// ── Firecrawl Scraper (optional paid provider) ───────────────────────────

/// Scrapes web pages using the Firecrawl API. Handles JS rendering,
/// anti-bot bypassing, and returns optimized markdown.
pub struct FirecrawlScraper {
    api_key: String,
    api_url: String,
    http: reqwest::Client,
}

impl FirecrawlScraper {
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()?;
        Ok(Self {
            api_key: api_key.into(),
            api_url: "https://api.firecrawl.dev/v1".into(),
            http,
        })
    }

    pub fn with_api_url(mut self, url: impl Into<String>) -> Self {
        self.api_url = url.into();
        self
    }
}

#[async_trait]
impl ScrapeProvider for FirecrawlScraper {
    async fn scrape(&self, url: &str) -> Result<ScrapeResult> {
        info!(url, "scraping via Firecrawl");

        let response = self
            .http
            .post(format!("{}/scrape", self.api_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&serde_json::json!({
                "url": url,
                "formats": ["markdown"],
            }))
            .send()
            .await
            .context("Firecrawl request failed")?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            let truncated = if body.len() > 500 { &body[..500] } else { &body };
            anyhow::bail!("Firecrawl error {status}: {truncated}");
        }

        let parsed: serde_json::Value = response.json().await?;
        let data = parsed.get("data").unwrap_or(&parsed);

        let markdown = data
            .get("markdown")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let title = data
            .get("metadata")
            .and_then(|m| m.get("title"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let final_url = data
            .get("metadata")
            .and_then(|m| m.get("sourceURL"))
            .and_then(|v| v.as_str())
            .unwrap_or(url)
            .to_string();

        let mut metadata = HashMap::new();
        if let Some(desc) = data
            .get("metadata")
            .and_then(|m| m.get("description"))
            .and_then(|v| v.as_str())
        {
            metadata.insert("description".into(), desc.to_string());
        }

        Ok(ScrapeResult {
            markdown,
            title,
            url: final_url,
            metadata,
        })
    }

    fn name(&self) -> &str {
        "firecrawl"
    }

    fn is_available(&self) -> bool {
        !self.api_key.is_empty()
    }
}

// ── Scrape Router ────────────────────────────────────────────────────────

/// Routes scrape requests to the best available provider.
/// Tries Firecrawl first (if configured), falls back to simple HTTP.
pub struct ScrapeRouter {
    providers: Vec<Box<dyn ScrapeProvider>>,
}

impl ScrapeRouter {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    pub fn with_provider(mut self, provider: Box<dyn ScrapeProvider>) -> Self {
        self.providers.push(provider);
        self
    }

    /// Build from environment variables.
    pub fn from_env() -> Result<Self> {
        let mut router = Self::new();

        // Firecrawl (paid, higher quality) — registered first so it's preferred
        if let Ok(key) = std::env::var("FIRECRAWL_API_KEY") {
            if !key.is_empty() {
                let mut provider = FirecrawlScraper::new(key)?;
                if let Ok(url) = std::env::var("FIRECRAWL_API_URL") {
                    if !url.is_empty() {
                        provider = provider.with_api_url(url);
                    }
                }
                router.providers.push(Box::new(provider));
                info!("scrape provider registered: firecrawl");
            }
        }

        // Simple HTTP (always available, free)
        router
            .providers
            .push(Box::new(SimpleHttpScraper::new()?));
        info!("scrape provider registered: simple-http");

        Ok(router)
    }

    /// Scrape a URL using the best available provider.
    pub async fn scrape(&self, url: &str) -> Result<ScrapeResult> {
        for provider in &self.providers {
            if provider.is_available() {
                match provider.scrape(url).await {
                    Ok(result) => return Ok(result),
                    Err(e) => {
                        warn!(
                            provider = provider.name(),
                            url,
                            "scrape failed, trying next provider: {e}"
                        );
                    }
                }
            }
        }
        anyhow::bail!("all scrape providers failed for {url}")
    }

    pub fn provider_name(&self) -> &str {
        self.providers
            .iter()
            .find(|p| p.is_available())
            .map(|p| p.name())
            .unwrap_or("none")
    }
}

impl Default for ScrapeRouter {
    fn default() -> Self {
        Self::new()
    }
}

// ── HTML helpers ─────────────────────────────────────────────────────────

fn extract_title(html: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let start = lower.find("<title")?;
    let after_open = lower[start..].find('>')?;
    let content_start = start + after_open + 1;
    let end = lower[content_start..].find("</title>")?;
    let title = html[content_start..content_start + end].trim();
    if title.is_empty() {
        None
    } else {
        Some(decode_entities(title))
    }
}

fn extract_meta(html: &str, name: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let pattern = format!("name=\"{name}\"");
    let idx = lower.find(&pattern)?;
    let region = &lower[idx.saturating_sub(200)..lower.len().min(idx + 500)];
    let content_start = region.find("content=\"")?;
    let after = &region[content_start + 9..];
    let end = after.find('"')?;
    let html_region = &html[idx.saturating_sub(200)..html.len().min(idx + 500)];
    let cs = html_region.to_lowercase().find("content=\"")?;
    let value = &html_region[cs + 9..];
    let e = value.find('"')?;
    let _ = (region, after, end); // suppress unused
    Some(value[..e].to_string())
}

/// Remove script, style, nav, footer, header tags and their content.
fn clean_html(html: &str) -> String {
    let mut result = html.to_string();
    for tag in &["script", "style", "nav", "footer", "header", "aside", "noscript", "svg"] {
        loop {
            let lower = result.to_lowercase();
            let open = format!("<{}", tag);
            let close = format!("</{}>", tag);
            if let Some(start) = lower.find(&open) {
                if let Some(end_offset) = lower[start..].find(&close) {
                    let end = start + end_offset + close.len();
                    result = format!("{}{}", &result[..start], &result[end..]);
                    continue;
                }
            }
            break;
        }
    }
    result
}

fn collapse_whitespace(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut blank_count = 0u32;
    for line in text.lines() {
        if line.trim().is_empty() {
            blank_count += 1;
            if blank_count <= 2 {
                result.push('\n');
            }
        } else {
            blank_count = 0;
            result.push_str(line);
            result.push('\n');
        }
    }
    result.trim().to_string()
}

fn decode_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_title_basic() {
        let html = "<html><head><title>Hello World</title></head></html>";
        assert_eq!(extract_title(html).unwrap(), "Hello World");
    }

    #[test]
    fn extract_title_with_entities() {
        let html = "<title>Foo &amp; Bar</title>";
        assert_eq!(extract_title(html).unwrap(), "Foo & Bar");
    }

    #[test]
    fn extract_title_missing() {
        assert!(extract_title("<html><body>no title</body></html>").is_none());
    }

    #[test]
    fn clean_html_removes_scripts() {
        let html = "<p>Before</p><script>alert('xss')</script><p>After</p>";
        let cleaned = clean_html(html);
        assert!(cleaned.contains("Before"));
        assert!(cleaned.contains("After"));
        assert!(!cleaned.contains("alert"));
    }

    #[test]
    fn clean_html_removes_nav_footer() {
        let html = "<nav>Menu</nav><main>Content</main><footer>Copyright</footer>";
        let cleaned = clean_html(html);
        assert!(!cleaned.contains("Menu"));
        assert!(cleaned.contains("Content"));
        assert!(!cleaned.contains("Copyright"));
    }

    #[test]
    fn collapse_whitespace_limits_blanks() {
        let text = "Line 1\n\n\n\n\n\nLine 2";
        let collapsed = collapse_whitespace(text);
        assert_eq!(collapsed.matches('\n').count(), 3); // line1 + 2 blanks + line2
    }

    #[test]
    fn scrape_router_always_has_simple_http() {
        let router = ScrapeRouter::from_env().unwrap();
        assert_eq!(router.provider_name(), "simple-http");
    }

    #[test]
    fn decode_entities_works() {
        assert_eq!(decode_entities("A &amp; B"), "A & B");
        assert_eq!(decode_entities("&lt;tag&gt;"), "<tag>");
    }
}
