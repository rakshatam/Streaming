use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    security::{create_jwt, hash_password, verify_jwt, verify_password},
    AppState,
};

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username_or_email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: UserDto,
}

#[derive(Debug, Serialize)]
pub struct AuthError {
    pub message: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct UserDto {
    pub id: String,
    pub username: String,
    pub email: String,
    pub role: String,
    pub created_at: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct UserRecord {
    id: String,
    username: String,
    email: String,
    password_hash: String,
    role: String,
    created_at: i64,
}

pub async fn register(
    State(state): State<AppState>,
    Json(payload): Json<RegisterRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<AuthError>)> {
    if payload.username.trim().is_empty() || payload.email.trim().is_empty() || payload.password.len() < 6 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(AuthError {
                message: "Username, valid email, and password of at least 6 characters required".into(),
            }),
        ));
    }

    let password_hash = hash_password(&payload.password)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(AuthError { message: e })))?;

    let user_id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp();

    let res = sqlx::query(
        r#"
        INSERT INTO users (id, username, email, password_hash, role, created_at)
        VALUES (?, ?, ?, ?, 'user', ?)
        "#
    )
    .bind(&user_id)
    .bind(&payload.username)
    .bind(&payload.email)
    .bind(&password_hash)
    .bind(now)
    .execute(&state.db)
    .await;

    if let Err(e) = res {
        return Err((
            StatusCode::CONFLICT,
            Json(AuthError {
                message: format!("User with this username or email already exists: {}", e),
            }),
        ));
    }

    let token = create_jwt(&user_id, &payload.username, "user", &state.config.jwt_secret, 7 * 86400)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(AuthError { message: e })))?;

    let user = UserDto {
        id: user_id,
        username: payload.username,
        email: payload.email,
        role: "user".into(),
        created_at: now,
    };

    Ok((StatusCode::CREATED, Json(AuthResponse { token, user })))
}

pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<AuthError>)> {
    let user = sqlx::query_as::<_, UserRecord>(
        r#"
        SELECT id, username, email, password_hash, role, created_at
        FROM users
        WHERE username = ? OR email = ?
        "#
    )
    .bind(&payload.username_or_email)
    .bind(&payload.username_or_email)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(AuthError { message: e.to_string() })))?;

    let user = match user {
        Some(u) => u,
        None => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(AuthError {
                    message: "Invalid username or password".into(),
                }),
            ))
        }
    };

    let is_valid = verify_password(&payload.password, &user.password_hash)
        .unwrap_or(false);

    if !is_valid {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(AuthError {
                message: "Invalid username or password".into(),
            }),
        ));
    }

    let token = create_jwt(&user.id, &user.username, &user.role, &state.config.jwt_secret, 7 * 86400)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(AuthError { message: e })))?;

    let user_dto = UserDto {
        id: user.id,
        username: user.username,
        email: user.email,
        role: user.role,
        created_at: user.created_at,
    };

    Ok((StatusCode::OK, Json(AuthResponse { token, user: user_dto })))
}

pub async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<AuthError>)> {
    let auth_header = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or_else(|| (
            StatusCode::UNAUTHORIZED,
            Json(AuthError {
                message: "Missing or invalid authorization header".to_string(),
            }),
        ))?;

    let claims = verify_jwt(auth_header, &state.config.jwt_secret)
        .map_err(|e| (StatusCode::UNAUTHORIZED, Json(AuthError { message: e })))?;

    let user = sqlx::query_as::<_, UserDto>(
        r#"
        SELECT id, username, email, role, created_at
        FROM users
        WHERE id = ?
        "#
    )
    .bind(&claims.sub)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(AuthError { message: e.to_string() })))?
    .ok_or_else(|| (
        StatusCode::NOT_FOUND,
        Json(AuthError {
            message: "User not found".to_string(),
        }),
    ))?;

    Ok(Json(user))
}
