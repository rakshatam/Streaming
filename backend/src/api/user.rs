use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::{
    api::videos::Video,
    security::verify_jwt,
    AppState,
};

#[derive(Debug, Deserialize)]
pub struct UpdateProgressRequest {
    pub video_id: String,
    pub progress_secs: i64,
}

#[derive(Debug, Deserialize)]
pub struct BookmarkToggleRequest {
    pub video_id: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct HistoryItem {
    pub video_id: String,
    pub progress_secs: i64,
    pub updated_at: i64,
    pub title: String,
    pub thumbnail_url: String,
    pub duration_secs: i64,
    pub category: String,
    pub is_live: i64,
}

fn extract_user_id(headers: &HeaderMap, jwt_secret: &str) -> Result<String, (StatusCode, String)> {
    let auth_header = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or_else(|| (StatusCode::UNAUTHORIZED, "Missing authorization token".to_string()))?;

    let claims = verify_jwt(auth_header, jwt_secret)
        .map_err(|e| (StatusCode::UNAUTHORIZED, e))?;

    Ok(claims.sub)
}

pub async fn get_history(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let user_id = extract_user_id(&headers, &state.config.jwt_secret)?;

    let history = sqlx::query_as::<_, HistoryItem>(
        r#"
        SELECT h.video_id, h.progress_secs, h.updated_at,
               v.title, v.thumbnail_url, v.duration_secs, v.category, v.is_live
        FROM watch_history h
        JOIN videos v ON h.video_id = v.id
        WHERE h.user_id = ?
        ORDER BY h.updated_at DESC
        LIMIT 20
        "#
    )
    .bind(&user_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(history))
}

pub async fn update_history(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpdateProgressRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let user_id = extract_user_id(&headers, &state.config.jwt_secret)?;
    let now = chrono::Utc::now().timestamp();

    sqlx::query(
        r#"
        INSERT INTO watch_history (user_id, video_id, progress_secs, updated_at)
        VALUES (?, ?, ?, ?)
        ON CONFLICT(user_id, video_id) DO UPDATE SET
            progress_secs = excluded.progress_secs,
            updated_at = excluded.updated_at
        "#
    )
    .bind(&user_id)
    .bind(&payload.video_id)
    .bind(payload.progress_secs)
    .bind(now)
    .execute(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((StatusCode::OK, Json(serde_json::json!({ "status": "updated" }))))
}

pub async fn get_bookmarks(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let user_id = extract_user_id(&headers, &state.config.jwt_secret)?;

    let bookmarks = sqlx::query_as::<_, Video>(
        r#"
        SELECT v.*
        FROM bookmarks b
        JOIN videos v ON b.video_id = v.id
        WHERE b.user_id = ?
        ORDER BY b.created_at DESC
        "#
    )
    .bind(&user_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(bookmarks))
}

pub async fn toggle_bookmark(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<BookmarkToggleRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let user_id = extract_user_id(&headers, &state.config.jwt_secret)?;

    let deleted = sqlx::query("DELETE FROM bookmarks WHERE user_id = ? AND video_id = ?")
        .bind(&user_id)
        .bind(&payload.video_id)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .rows_affected();

    let is_bookmarked = if deleted > 0 {
        false
    } else {
        let now = chrono::Utc::now().timestamp();
        let _ = sqlx::query("INSERT OR IGNORE INTO bookmarks (user_id, video_id, created_at) VALUES (?, ?, ?)")
            .bind(&user_id)
            .bind(&payload.video_id)
            .bind(now)
            .execute(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        true
    };

    Ok(Json(serde_json::json!({ "is_bookmarked": is_bookmarked })))
}
