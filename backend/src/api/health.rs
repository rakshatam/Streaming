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
}

pub async fn health_check() -> impl IntoResponse {
    Json(HealthStatus {
        status: "healthy",
        service: "streamflow-rust-core",
        version: "1.0.0",
    })
}

pub async fn get_metrics(State(state): State<AppState>) -> impl IntoResponse {
    let reqs = state.stream_engine.total_requests.load(Ordering::Relaxed);
    let hits = state.stream_engine.cache_hits.load(Ordering::Relaxed);
    let misses = state.stream_engine.cache_misses.load(Ordering::Relaxed);
    let bytes = state.stream_engine.bytes_streamed.load(Ordering::Relaxed);

    let ratio = if reqs > 0 {
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
    })
}
