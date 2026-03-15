use std::sync::Arc;

use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::AppState;

#[derive(Deserialize)]
pub(crate) struct ScrapeRequest {
    url: String,
    #[serde(default = "default_true")]
    include_links: bool,
}

fn default_true() -> bool {
    true
}

/// POST /api/scrape — Scrape a URL and return clean markdown.
pub(crate) async fn scrape_url(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ScrapeRequest>,
) -> Result<Json<Value>, StatusCode> {
    if body.url.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Basic URL validation
    if !body.url.starts_with("http://") && !body.url.starts_with("https://") {
        return Ok(Json(json!({"error": "URL must start with http:// or https://"})));
    }

    let result = state
        .scraper
        .scrape(&body.url)
        .await
        .map_err(|e| {
            tracing::warn!(url = %body.url, "scrape failed: {e}");
            StatusCode::BAD_GATEWAY
        })?;

    let mut markdown = result.markdown;
    if !body.include_links {
        markdown = strip_markdown_links(&markdown);
    }

    Ok(Json(json!({
        "markdown": markdown,
        "title": result.title,
        "url": result.url,
        "metadata": result.metadata,
        "provider": state.scraper.provider_name(),
    })))
}

/// GET /api/scrape/status — Check scraper availability.
pub(crate) async fn scrape_status(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    Json(json!({
        "available": true,
        "provider": state.scraper.provider_name(),
    }))
}

/// Remove markdown links, keeping just the text: [text](url) → text
fn strip_markdown_links(md: &str) -> String {
    let mut result = String::with_capacity(md.len());
    let chars: Vec<char> = md.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '[' {
            // Look for ](url)
            if let Some(close_bracket) = chars[i + 1..].iter().position(|c| *c == ']') {
                let text_end = i + 1 + close_bracket;
                if text_end + 1 < chars.len() && chars[text_end + 1] == '(' {
                    if let Some(close_paren) = chars[text_end + 2..].iter().position(|c| *c == ')') {
                        let link_text: String = chars[i + 1..text_end].iter().collect();
                        result.push_str(&link_text);
                        i = text_end + 2 + close_paren + 1;
                        continue;
                    }
                }
            }
        }
        result.push(chars[i]);
        i += 1;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_links_basic() {
        assert_eq!(strip_markdown_links("[Click here](https://example.com)"), "Click here");
    }

    #[test]
    fn strip_links_preserves_plain_text() {
        assert_eq!(strip_markdown_links("No links here"), "No links here");
    }

    #[test]
    fn strip_links_mixed() {
        let input = "See [docs](https://docs.rs) and [repo](https://github.com) for info.";
        assert_eq!(strip_markdown_links(input), "See docs and repo for info.");
    }
}
