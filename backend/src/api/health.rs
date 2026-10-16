use axum::{extract::State, response::IntoResponse, Json};
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

pub async fn clear_cache(State(state): State<AppState>) -> impl IntoResponse {
    state.stream_engine.clear_cache().await;
    state.stream_engine.reset_cache_counters();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    Json(ClearCacheResponse {
        success: true,
        message: "In-memory segment and manifest cache cleared successfully",
        cleared_at_unix: now,
    })
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
    } else if reqs > 0 {
        (hits as f64 / reqs as f64) * 100.0
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
