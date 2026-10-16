use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::AppState;

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

    sql.push_str(" ORDER BY is_featured DESC, views_count DESC LIMIT ? OFFSET ?");

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

    let videos = q
        .fetch_all(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(videos))
}

pub async fn get_video(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let video = sqlx::query_as::<_, Video>(
        "SELECT * FROM videos WHERE id = ?"
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or_else(|| (StatusCode::NOT_FOUND, "Video not found".to_string()))?;

    // Asynchronously increment view count without blocking
    let db = state.db.clone();
    let vid_id = id.clone();
    tokio::spawn(async move {
        let _ = sqlx::query("UPDATE videos SET views_count = views_count + 1 WHERE id = ?")
            .bind(vid_id)
            .execute(&db)
            .await;
    });

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
        "#
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
                ORDER BY views_count DESC
                LIMIT 30
                "#
            )
            .bind(&like_term)
            .bind(&like_term)
            .bind(&like_term)
            .bind(&like_term)
            .fetch_all(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

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
        "SELECT category, COUNT(*) as count FROM videos GROUP BY category ORDER BY count DESC"
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(categories))
}
