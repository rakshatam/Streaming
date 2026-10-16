use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::Response,
};
use serde::{Deserialize, Serialize};
use tracing::{warn, error};

use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct StreamProxyQuery {
    pub uri: String,
    pub token: String,
    pub exp: u64,
}

#[derive(Debug, Serialize)]
pub struct ManifestInitResponse {
    pub manifest_url: String,
    pub token: String,
    pub expires_at: u64,
}

/// Serves master M3U8 manifest with rewritten URLs and signed tokens
pub async fn get_manifest(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, (StatusCode, String)> {
    let video: Option<(String,)> = sqlx::query_as("SELECT stream_url FROM videos WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let upstream_url = match video {
        Some((url,)) => url,
        None => return Err((StatusCode::NOT_FOUND, "Video not found".to_string())),
    };

    let rewritten = state
        .stream_engine
        .rewrite_manifest(&upstream_url)
        .await
        .map_err(|e| {
            error!("Manifest rewrite failure for video {}: {}", id, e);
            (StatusCode::BAD_GATEWAY, format!("Stream acquisition failed: {}", e))
        })?;

    let resp = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")
        .header(header::CACHE_CONTROL, "no-cache, no-store, must-revalidate")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(header::ACCESS_CONTROL_ALLOW_HEADERS, "*")
        .header("X-Content-Type-Options", "nosniff")
        .body(axum::body::Body::from(rewritten))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(resp)
}

/// Serves sub-level playlist variants (resolution & bitrate ladders)
pub async fn get_playlist(
    State(state): State<AppState>,
    Query(params): Query<StreamProxyQuery>,
) -> Result<Response, (StatusCode, String)> {
    if !state.stream_engine.validate_token(&params.uri, &params.token, params.exp) {
        warn!("Unauthorized or expired playlist token access for URI: {}", params.uri);
        return Err((StatusCode::FORBIDDEN, "Invalid or expired stream token".to_string()));
    }

    let rewritten = state
        .stream_engine
        .rewrite_manifest(&params.uri)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Failed to fetch sub-playlist: {}", e)))?;

    let resp = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")
        .header(header::CACHE_CONTROL, "no-cache, no-store, must-revalidate")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(header::ACCESS_CONTROL_ALLOW_HEADERS, "*")
        .header("X-Content-Type-Options", "nosniff")
        .body(axum::body::Body::from(rewritten))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(resp)
}

/// Serves individual video segments (.ts, .m4s) with LRU memory caching and range support
pub async fn get_segment(
    State(state): State<AppState>,
    Query(params): Query<StreamProxyQuery>,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, String)> {
    if !state.stream_engine.validate_token(&params.uri, &params.token, params.exp) {
        warn!("Unauthorized segment access attempt detected. URI: {}", params.uri);
        return Err((StatusCode::FORBIDDEN, "Invalid or expired stream token".to_string()));
    }

    let client_range = headers
        .get(header::RANGE)
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());

    let seg = state
        .stream_engine
        .fetch_segment(&params.uri, client_range)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Segment fetch failed: {}", e)))?;

    let cache_status = if seg.is_cached { "HIT" } else { "MISS" };
    let status_code = StatusCode::from_u16(seg.status).unwrap_or(StatusCode::OK);

    let mut builder = Response::builder()
        .status(status_code)
        .header(header::CONTENT_TYPE, seg.content_type)
        .header(header::CONTENT_LENGTH, seg.bytes.len().to_string())
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CACHE_CONTROL, "public, max-age=86400, immutable")
        .header("X-Cache", cache_status)
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header("X-Content-Type-Options", "nosniff");

    if let Some(ref range) = seg.content_range {
        builder = builder.header(header::CONTENT_RANGE, range);
    }

    let resp = builder
        .body(axum::body::Body::from(seg.bytes))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(resp)
}
