pub mod auth;
pub mod health;
pub mod stream_routes;
pub mod user;
pub mod videos;

use axum::{
    routing::{get, post},
    Router,
};

use crate::AppState;

pub fn create_router() -> Router<AppState> {
    Router::new()
        // Health & Metrics
        .route("/api/health", get(health::health_check))
        .route("/api/metrics", get(health::get_metrics))
        // Authentication (Pure Username + Argon2id Password)
        .route("/api/auth/register", post(auth::register))
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/me", get(auth::me))
        // Video Catalog & Search
        .route("/api/videos", get(videos::list_videos))
        .route("/api/videos/search", get(videos::search_videos))
        .route("/api/videos/categories", get(videos::get_categories))
        .route("/api/videos/:id", get(videos::get_video))
        // Interactive Likes & Ratings
        .route("/api/videos/:id/interactions", get(videos::get_interactions))
        .route("/api/videos/:id/like", post(videos::toggle_like))
        .route("/api/videos/:id/rate", post(videos::rate_video))
        // Stream Proxy
        .route("/api/stream/manifest/:id", get(stream_routes::get_manifest))
        .route("/api/stream/playlist", get(stream_routes::get_playlist))
        .route("/api/stream/segment", get(stream_routes::get_segment))
        // User Watch History & Bookmarks
        .route("/api/user/history", get(user::get_history).post(user::update_history))
        .route("/api/user/bookmarks", get(user::get_bookmarks).post(user::toggle_bookmark))
}
