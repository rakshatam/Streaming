use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tracing::{error, info};
use uuid::Uuid;

use crate::{
    security::{create_jwt, hash_password, verify_jwt, verify_password},
    AppState,
};

static PASSWORD_HASH_SLOTS: OnceLock<Arc<Semaphore>> = OnceLock::new();

fn acquire_password_hash_slot() -> Result<OwnedSemaphorePermit, (StatusCode, Json<AuthError>)> {
    PASSWORD_HASH_SLOTS
        .get_or_init(|| Arc::new(Semaphore::new(4)))
        .clone()
        .try_acquire_owned()
        .map_err(|_| {
            (
                StatusCode::TOO_MANY_REQUESTS,
                Json(AuthError {
                    message: "Authentication is busy. Please try again shortly.".into(),
                }),
            )
        })
}

fn internal_error(
    context: &'static str,
    error_value: impl std::fmt::Display,
) -> (StatusCode, Json<AuthError>) {
    error!("{context}: {error_value}");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(AuthError {
            message: "Internal server error".into(),
        }),
    )
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
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
    pub role: String,
    pub created_at: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct UserRecord {
    id: String,
    username: String,
    password_hash: String,
    role: String,
    created_at: i64,
}

/// Account registration with username and password
pub async fn register(
    State(state): State<AppState>,
    Json(payload): Json<RegisterRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<AuthError>)> {
    let username = payload.username.trim().to_string();

    // 1. Strict Username Validation
    if username.len() < 3 || username.len() > 30 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(AuthError {
                message: "Username must be between 3 and 30 characters long.".into(),
            }),
        ));
    }

    if !username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(AuthError {
                message: "Username can only contain letters, numbers, underscores, and hyphens."
                    .into(),
            }),
        ));
    }

    // 2. Strict Password Validation
    if payload.password.len() < 8 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(AuthError {
                message: "Password must be at least 8 characters long for security.".into(),
            }),
        ));
    }

    if payload.password.len() > 128 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(AuthError {
                message: "Password cannot exceed 128 characters.".into(),
            }),
        ));
    }

    // 3. Prevent duplicate username (case-insensitive check). The matching
    // database index also handles concurrent registrations safely.
    let existing: Option<(String,)> =
        sqlx::query_as("SELECT id FROM users WHERE username COLLATE NOCASE = ?")
            .bind(&username)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| internal_error("Username lookup failed", e))?;

    if existing.is_some() {
        return Err((
            StatusCode::CONFLICT,
            Json(AuthError {
                message: "This username is already taken. Please choose another username.".into(),
            }),
        ));
    }

    // 4. Hash password with Argon2id (OWASP recommended parameters)
    let _hash_slot = acquire_password_hash_slot()?;
    let password = payload.password;
    let password_hash = tokio::task::spawn_blocking(move || hash_password(&password))
        .await
        .map_err(|e| internal_error("Password hashing task failed", e))?
        .map_err(|e| internal_error("Password hashing failed", e))?;

    let user_id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp();

    // 5. Insert user record
    let insert_result = sqlx::query(
        r#"
        INSERT INTO users (id, username, password_hash, role, created_at)
        VALUES (?, ?, ?, 'user', ?)
        "#,
    )
    .bind(&user_id)
    .bind(&username)
    .bind(&password_hash)
    .bind(now)
    .execute(&state.db)
    .await;

    if let Err(e) = insert_result {
        let err_msg = e.to_string();
        if e.as_database_error()
            .map(|database_error| database_error.is_unique_violation())
            .unwrap_or(false)
        {
            return Err((
                StatusCode::CONFLICT,
                Json(AuthError {
                    message: "This username is already taken. Please choose another username."
                        .into(),
                }),
            ));
        }
        return Err(internal_error("User registration failed", err_msg));
    }

    info!("Registered user: '{}' ({})", username, user_id);

    // 6. Generate session token
    let token = create_jwt(
        &user_id,
        &username,
        "user",
        &state.config.jwt_secret,
        7 * 86400,
    )
    .map_err(|e| internal_error("Session token creation failed", e))?;

    let user_dto = UserDto {
        id: user_id,
        username,
        role: "user".into(),
        created_at: now,
    };

    Ok((
        StatusCode::CREATED,
        Json(AuthResponse {
            token,
            user: user_dto,
        }),
    ))
}

/// Account sign in
pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<AuthError>)> {
    let username = payload.username.trim();

    if username.is_empty() || payload.password.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(AuthError {
                message: "Username and password are required.".into(),
            }),
        ));
    }

    if username.len() > 30 || payload.password.len() > 128 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(AuthError {
                message: "Username or password is too long.".into(),
            }),
        ));
    }

    // Case-insensitive username lookup
    let user = sqlx::query_as::<_, UserRecord>(
        r#"
        SELECT id, username, password_hash, role, created_at
        FROM users
        WHERE username COLLATE NOCASE = ?
        "#,
    )
    .bind(username)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| internal_error("User lookup failed", e))?;

    let user = match user {
        Some(u) => u,
        None => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(AuthError {
                    message: "Invalid username or password.".into(),
                }),
            ))
        }
    };

    // Constant-time Argon2id password verification
    let _hash_slot = acquire_password_hash_slot()?;
    let password = payload.password;
    let password_hash = user.password_hash.clone();
    let is_valid = tokio::task::spawn_blocking(move || verify_password(&password, &password_hash))
        .await
        .map_err(|e| internal_error("Password verification task failed", e))?
        .unwrap_or(false);

    if !is_valid {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(AuthError {
                message: "Invalid username or password.".into(),
            }),
        ));
    }

    let token = create_jwt(
        &user.id,
        &user.username,
        &user.role,
        &state.config.jwt_secret,
        7 * 86400,
    )
    .map_err(|e| internal_error("Session token creation failed", e))?;

    let user_dto = UserDto {
        id: user.id,
        username: user.username,
        role: user.role,
        created_at: user.created_at,
    };

    Ok((
        StatusCode::OK,
        Json(AuthResponse {
            token,
            user: user_dto,
        }),
    ))
}

/// Returns authenticated session profile
pub async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<AuthError>)> {
    let auth_header = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(AuthError {
                    message: "Missing or invalid authorization header.".to_string(),
                }),
            )
        })?;

    let claims = verify_jwt(auth_header, &state.config.jwt_secret).map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            Json(AuthError {
                message: "Invalid or expired authorization token.".into(),
            }),
        )
    })?;

    let user = sqlx::query_as::<_, UserDto>(
        r#"
        SELECT id, username, role, created_at
        FROM users
        WHERE id = ?
        "#,
    )
    .bind(&claims.sub)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| internal_error("Profile lookup failed", e))?
    .ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(AuthError {
                message: "User account not found.".to_string(),
            }),
        )
    })?;

    Ok(Json(user))
}
