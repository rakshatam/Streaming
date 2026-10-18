use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Serialize;
use std::sync::atomic::Ordering;

use crate::AppState;

#[derive(Serialize)]
pub struct HealthStatus {
    pub status: &'static str,
    pub service: &'static str,
    pub version: &'static str,
}

#[derive(Serialize)]
pub struct StreamMetrics {
    pub total_requests: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub cache_hit_ratio_percent: f64,
    pub bytes_streamed: u64,
    pub megabytes_streamed: f64,
    pub segment_cache_entries: u64,
    pub manifest_cache_entries: u64,
}

#[derive(Serialize)]
pub struct ClearCacheResponse {
    pub success: bool,
    pub message: &'static str,
    pub cleared_at_unix: u64,
}

pub async fn health_check() -> impl IntoResponse {
    Json(HealthStatus {
        status: "healthy",
        service: "streamflow-rust-core",
        version: "1.0.0",
    })
}

pub async fn clear_cache(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, StatusCode> {
    let configured_token = state
        .config
        .cache_admin_token
        .as_deref()
        .ok_or(StatusCode::NOT_FOUND)?;
    let supplied_token = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    if !constant_time_eq(configured_token.as_bytes(), supplied_token.as_bytes()) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    state.stream_engine.clear_cache().await;
    state.stream_engine.reset_cache_counters();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    Ok(Json(ClearCacheResponse {
        success: true,
        message: "In-memory segment and manifest cache cleared successfully",
        cleared_at_unix: now,
    }))
}

fn constant_time_eq(expected: &[u8], supplied: &[u8]) -> bool {
    if expected.len() != supplied.len() {
        return false;
    }

    expected
        .iter()
        .zip(supplied)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

pub async fn get_metrics(State(state): State<AppState>) -> impl IntoResponse {
    let reqs = state.stream_engine.total_requests.load(Ordering::Relaxed);
    let hits = state.stream_engine.cache_hits.load(Ordering::Relaxed);
    let misses = state.stream_engine.cache_misses.load(Ordering::Relaxed);
    let bytes = state.stream_engine.bytes_streamed.load(Ordering::Relaxed);
    let (manifest_entries, segment_entries) = state.stream_engine.cache_entry_counts();

    let total_cache_lookups = hits + misses;
    let ratio = if total_cache_lookups > 0 {
        (hits as f64 / total_cache_lookups as f64) * 100.0
    } else {
        0.0
    };

    Json(StreamMetrics {
        total_requests: reqs,
        cache_hits: hits,
        cache_misses: misses,
        cache_hit_ratio_percent: (ratio * 100.0).round() / 100.0,
        bytes_streamed: bytes,
        megabytes_streamed: ((bytes as f64 / (1024.0 * 1024.0)) * 100.0).round() / 100.0,
        segment_cache_entries: segment_entries,
        manifest_cache_entries: manifest_entries,
    })
}
