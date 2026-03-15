use std::sync::Arc;

use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::AppState;

fn internal(e: impl std::fmt::Display) -> StatusCode {
    tracing::error!("ocr error: {e}");
    StatusCode::INTERNAL_SERVER_ERROR
}

#[derive(Deserialize)]
pub(crate) struct OcrRequest {
    /// Base64-encoded document content.
    data: String,
    /// MIME type of the document (e.g., "application/pdf", "image/png").
    mime_type: String,
}

/// POST /api/ocr — OCR a document and return extracted text.
pub(crate) async fn ocr_document(
    State(state): State<Arc<AppState>>,
    Json(body): Json<OcrRequest>,
) -> Result<Json<Value>, StatusCode> {
    if !state.ocr.is_available() {
        return Ok(Json(
            json!({"error": "OCR not configured — set MISTRAL_API_KEY"}),
        ));
    }

    let data = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        &body.data,
    )
    .map_err(|_| StatusCode::BAD_REQUEST)?;

    let result = state
        .ocr
        .ocr(&data, &body.mime_type)
        .await
        .map_err(internal)?;

    Ok(Json(json!({
        "text": result.text,
        "page_count": result.page_count,
        "pages": result.pages,
        "provider": state.ocr.provider_name(),
    })))
}

/// GET /api/ocr/status — Check if OCR is available.
pub(crate) async fn ocr_status(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    Json(json!({
        "available": state.ocr.is_available(),
        "provider": state.ocr.provider_name(),
    }))
}
