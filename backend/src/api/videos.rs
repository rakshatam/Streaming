use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::{security::verify_jwt, AppState};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct Video {
    pub id: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub quality: String,
    pub stream_url: String,
    pub thumbnail_url: String,
    pub backdrop_url: String,
    pub duration_secs: i64,
    pub is_live: i64,
    pub is_featured: i64,
    pub views_count: i64,
    pub rating: f64,
    pub tags: String,
    pub created_at: i64,
}

#[derive(Debug, Deserialize)]
pub struct ListVideosQuery {
    pub category: Option<String>,
    pub is_live: Option<i64>,
    pub is_featured: Option<i64>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

#[derive(Debug, Deserialize)]
pub struct RateVideoRequest {
    pub rating: i64, // 1 to 5
}

#[derive(Debug, Serialize)]
pub struct VideoInteractionsResponse {
    pub video_id: String,
    pub likes_count: i64,
    pub has_liked: bool,
    pub user_rating: Option<i64>,
    pub average_rating: f64,
    pub total_ratings: i64,
}

#[derive(Debug, Serialize)]
pub struct VideoActionError {
    pub message: String,
}

fn database_error(error_value: sqlx::Error) -> (StatusCode, String) {
    error!("Video API database operation failed: {}", error_value);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        "Internal server error".into(),
    )
}

fn action_database_error(error_value: sqlx::Error) -> (StatusCode, Json<VideoActionError>) {
    error!(
        "Video interaction database operation failed: {}",
        error_value
    );
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(VideoActionError {
            message: "Internal server error".into(),
        }),
    )
}

fn try_extract_user_id(headers: &HeaderMap, jwt_secret: &str) -> Option<String> {
    headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .and_then(|t| verify_jwt(t, jwt_secret).ok())
        .map(|c| c.sub)
}

pub async fn list_videos(
    State(state): State<AppState>,
    Query(query): Query<ListVideosQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    let offset = query.offset.unwrap_or(0).max(0);

    let mut sql = "SELECT * FROM videos WHERE 1=1".to_string();

    if query.category.is_some() {
        sql.push_str(" AND category = ?");
    }
    if query.is_live.is_some() {
        sql.push_str(" AND is_live = ?");
    }
    if query.is_featured.is_some() {
        sql.push_str(" AND is_featured = ?");
    }

    sql.push_str(" ORDER BY is_featured DESC, rating DESC LIMIT ? OFFSET ?");

    let mut q = sqlx::query_as::<_, Video>(&sql);

    if let Some(ref cat) = query.category {
        q = q.bind(cat);
    }
    if let Some(live) = query.is_live {
        q = q.bind(live);
    }
    if let Some(feat) = query.is_featured {
        q = q.bind(feat);
    }

    q = q.bind(limit).bind(offset);

    let videos = q.fetch_all(&state.db).await.map_err(database_error)?;

    Ok(Json(videos))
}

pub async fn get_video(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let video = sqlx::query_as::<_, Video>("SELECT * FROM videos WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.db)
        .await
        .map_err(database_error)?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Video not found".to_string()))?;

    Ok(Json(video))
}

pub async fn search_videos(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let search_term = query.q.trim();
    if search_term.is_empty() {
        return Ok(Json(Vec::<Video>::new()));
    }
    if search_term.len() > 100 {
        return Err((StatusCode::BAD_REQUEST, "Search query is too long".into()));
    }

    // Format valid FTS5 query (e.g. "space*" for prefix matching)
    let sanitized = search_term.replace('"', "").replace('*', "");
    let fts_query = format!("{}*", sanitized);
    let fts_results = sqlx::query_as::<_, Video>(
        r#"
        SELECT v.* FROM videos v
        JOIN fts_videos f ON v.id = f.video_id
        WHERE fts_videos MATCH ?
        ORDER BY rank
        LIMIT 30
        "#,
    )
    .bind(&fts_query)
    .fetch_all(&state.db)
    .await;

    match fts_results {
        Ok(videos) if !videos.is_empty() => Ok(Json(videos)),
        _ => {
            // Fallback to LIKE query for partial matching
            let like_term = format!("%{}%", search_term);
            let fallback_results = sqlx::query_as::<_, Video>(
                r#"
                SELECT * FROM videos
                WHERE title LIKE ? OR description LIKE ? OR category LIKE ? OR tags LIKE ?
                ORDER BY rating DESC
                LIMIT 30
                "#,
            )
            .bind(&like_term)
            .bind(&like_term)
            .bind(&like_term)
            .bind(&like_term)
            .fetch_all(&state.db)
            .await
            .map_err(database_error)?;

            Ok(Json(fallback_results))
        }
    }
}

pub async fn get_categories(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    #[derive(Serialize, sqlx::FromRow)]
    struct CategoryCount {
        category: String,
        count: i64,
    }

    let categories = sqlx::query_as::<_, CategoryCount>(
        "SELECT category, COUNT(*) as count FROM videos GROUP BY category ORDER BY count DESC",
    )
    .fetch_all(&state.db)
    .await
    .map_err(database_error)?;

    Ok(Json(categories))
}

/// Fetches likes count, user like status, and ratings statistics
pub async fn get_interactions(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let user_id = try_extract_user_id(&headers, &state.config.jwt_secret);

    let video_rating: Option<(f64,)> = sqlx::query_as("SELECT rating FROM videos WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.db)
        .await
        .map_err(database_error)?;
    let Some((catalog_rating,)) = video_rating else {
        return Err((StatusCode::NOT_FOUND, "Video not found".into()));
    };

    let (likes_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM video_likes WHERE video_id = ?")
            .bind(&id)
            .fetch_one(&state.db)
            .await
            .map_err(database_error)?;

    let has_liked = if let Some(ref uid) = user_id {
        let exists: Option<(String,)> =
            sqlx::query_as("SELECT video_id FROM video_likes WHERE user_id = ? AND video_id = ?")
                .bind(uid)
                .bind(&id)
                .fetch_optional(&state.db)
                .await
                .map_err(database_error)?;
        exists.is_some()
    } else {
        false
    };

    let user_rating: Option<i64> = if let Some(ref uid) = user_id {
        let r: Option<(i64,)> =
            sqlx::query_as("SELECT rating FROM video_ratings WHERE user_id = ? AND video_id = ?")
                .bind(uid)
                .bind(&id)
                .fetch_optional(&state.db)
                .await
                .map_err(database_error)?;
        r.map(|(val,)| val)
    } else {
        None
    };

    let rating_stats: (i64, Option<f64>) =
        sqlx::query_as("SELECT COUNT(*), AVG(rating) FROM video_ratings WHERE video_id = ?")
            .bind(&id)
            .fetch_one(&state.db)
            .await
            .map_err(database_error)?;

    let total_ratings = rating_stats.0;
    let avg_rating = rating_stats.1.unwrap_or(catalog_rating);

    Ok(Json(VideoInteractionsResponse {
        video_id: id,
        likes_count,
        has_liked,
        user_rating,
        average_rating: (avg_rating * 10.0).round() / 10.0,
        total_ratings,
    }))
}

/// Toggles user like status (requires JWT authentication)
pub async fn toggle_like(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<VideoActionError>)> {
    let user_id = try_extract_user_id(&headers, &state.config.jwt_secret).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(VideoActionError {
                message: "Please sign in to like this video".into(),
            }),
        )
    })?;

    let video_exists: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM videos WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.db)
        .await
        .map_err(action_database_error)?;
    if video_exists.is_none() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(VideoActionError {
                message: "Video not found".into(),
            }),
        ));
    }

    let now = chrono::Utc::now().timestamp();
    let inserted = sqlx::query(
        "INSERT OR IGNORE INTO video_likes (user_id, video_id, created_at) VALUES (?, ?, ?)",
    )
    .bind(&user_id)
    .bind(&id)
    .bind(now)
    .execute(&state.db)
    .await
    .map_err(action_database_error)?
    .rows_affected();

    let has_liked = if inserted > 0 {
        true
    } else {
        sqlx::query("DELETE FROM video_likes WHERE user_id = ? AND video_id = ?")
            .bind(&user_id)
            .bind(&id)
            .execute(&state.db)
            .await
            .map_err(action_database_error)?;
        false
    };

    let (likes_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM video_likes WHERE video_id = ?")
            .bind(&id)
            .fetch_one(&state.db)
            .await
            .map_err(action_database_error)?;

    Ok(Json(serde_json::json!({
        "video_id": id,
        "has_liked": has_liked,
        "likes_count": likes_count
    })))
}

/// Submits or updates a 1-5 star user rating (requires JWT authentication)
pub async fn rate_video(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<RateVideoRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<VideoActionError>)> {
    let user_id = try_extract_user_id(&headers, &state.config.jwt_secret).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(VideoActionError {
                message: "Please sign in to rate this video".into(),
            }),
        )
    })?;

    let video_exists: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM videos WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.db)
        .await
        .map_err(action_database_error)?;
    if video_exists.is_none() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(VideoActionError {
                message: "Video not found".into(),
            }),
        ));
    }

    if payload.rating < 1 || payload.rating > 5 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(VideoActionError {
                message: "Rating must be between 1 and 5 stars".into(),
            }),
        ));
    }

    let now = chrono::Utc::now().timestamp();
    let mut transaction = state.db.begin().await.map_err(action_database_error)?;
    sqlx::query(
        r#"
        INSERT INTO video_ratings (user_id, video_id, rating, created_at)
        VALUES (?, ?, ?, ?)
        ON CONFLICT(user_id, video_id) DO UPDATE SET
            rating = excluded.rating,
            created_at = excluded.created_at
        "#,
    )
    .bind(&user_id)
    .bind(&id)
    .bind(payload.rating)
    .bind(now)
    .execute(&mut *transaction)
    .await
    .map_err(action_database_error)?;

    // Recalculate average rating for the video
    let stats: (i64, Option<f64>) =
        sqlx::query_as("SELECT COUNT(*), AVG(rating) FROM video_ratings WHERE video_id = ?")
            .bind(&id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(action_database_error)?;

    let total = stats.0;
    let avg = stats.1.unwrap_or(payload.rating as f64);
    let rounded_avg = (avg * 10.0).round() / 10.0;

    // Keep the catalog aggregate consistent with the user ratings.
    sqlx::query("UPDATE videos SET rating = ? WHERE id = ?")
        .bind(rounded_avg)
        .bind(&id)
        .execute(&mut *transaction)
        .await
        .map_err(action_database_error)?;
    transaction.commit().await.map_err(action_database_error)?;

    Ok(Json(serde_json::json!({
        "video_id": id,
        "user_rating": payload.rating,
        "average_rating": rounded_avg,
        "total_ratings": total
    })))
}
