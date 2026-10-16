use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use tracing::info;
use uuid::Uuid;

use crate::{
    security::{create_jwt, hash_password, verify_jwt, verify_password},
    AppState,
};

#[derive(Debug, Deserialize)]
pub struct RequestOtpRequest {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct VerifyOtpRequest {
    pub email: String,
    pub otp_code: String,
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
pub struct OtpSentResponse {
    pub message: String,
    pub email: String,
    pub expires_in_secs: u64,
    pub dev_otp_preview: String,
    pub smtp_dispatched: bool,
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

#[derive(Debug, sqlx::FromRow)]
#[allow(dead_code)]
struct OtpRecord {
    email: String,
    username: String,
    password_hash: String,
    otp_code: String,
    attempts_left: i64,
    expires_at: i64,
    created_at: i64,
}

/// Step 1 of Secure Registration: Validates inputs, creates Argon2id hash, and generates a 6-digit OTP
pub async fn request_otp(
    State(state): State<AppState>,
    Json(payload): Json<RequestOtpRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<AuthError>)> {
    let username = payload.username.trim().to_string();
    let email = payload.email.trim().to_lowercase();

    if username.len() < 3 || username.len() > 30 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(AuthError {
                message: "Username must be between 3 and 30 characters".into(),
            }),
        ));
    }

    if !email.contains('@') || !email.contains('.') {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(AuthError {
                message: "A valid email address is required for security verification".into(),
            }),
        ));
    }

    if payload.password.len() < 8 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(AuthError {
                message: "Password must be at least 8 characters long".into(),
            }),
        ));
    }

    // Check if username or email already exists
    let existing: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM users WHERE username = ? OR email = ?"
    )
    .bind(&username)
    .bind(&email)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(AuthError { message: e.to_string() })))?;

    if existing.is_some() {
        return Err((
            StatusCode::CONFLICT,
            Json(AuthError {
                message: "Username or email is already registered".into(),
            }),
        ));
    }

    // Hash password with Argon2id
    let password_hash = hash_password(&payload.password)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(AuthError { message: e })))?;

    // Generate cryptographically secure 6-digit OTP
    let otp_num: u32 = rand::thread_rng().gen_range(100000..=999999);
    let otp_code = otp_num.to_string();

    let now = chrono::Utc::now().timestamp();
    let expires_at = now + 600; // 10 minutes

    // Store pending registration with OTP
    sqlx::query(
        r#"
        INSERT OR REPLACE INTO email_otps (email, username, password_hash, otp_code, attempts_left, expires_at, created_at)
        VALUES (?, ?, ?, ?, 5, ?, ?)
        "#
    )
    .bind(&email)
    .bind(&username)
    .bind(&password_hash)
    .bind(&otp_code)
    .bind(expires_at)
    .bind(now)
    .execute(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(AuthError { message: e.to_string() })))?;

    // Dispatch real email via SMTP
    let (smtp_dispatched, message) = match crate::email::send_otp_email(&email, &username, &otp_code, &state.config).await {
        Ok(true) => (
            true,
            "A 6-digit verification code has been dispatched directly to your email inbox. Please check your inbox and spam folder.".to_string(),
        ),
        Ok(false) => (
            false,
            "SMTP server is not configured in backend/.env. For local testing, your verification code is displayed below.".to_string(),
        ),
        Err(e) => (
            false,
            format!("SMTP email delivery failed ({}). Fallback code generated below for testing.", e),
        ),
    };

    info!("[SECURE OTP DISPATCH] Generated OTP {} for user {} ({}) | SMTP Dispatched: {}", otp_code, username, email, smtp_dispatched);

    Ok((
        StatusCode::OK,
        Json(OtpSentResponse {
            message,
            email: email.clone(),
            expires_in_secs: 600,
            dev_otp_preview: otp_code,
            smtp_dispatched,
        }),
    ))
}

/// Step 2 of Secure Registration: Verifies 6-digit OTP code and creates account
pub async fn verify_otp(
    State(state): State<AppState>,
    Json(payload): Json<VerifyOtpRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<AuthError>)> {
    let email = payload.email.trim().to_lowercase();
    let entered_otp = payload.otp_code.trim();

    let record = sqlx::query_as::<_, OtpRecord>(
        "SELECT email, username, password_hash, otp_code, attempts_left, expires_at, created_at FROM email_otps WHERE email = ?"
    )
    .bind(&email)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(AuthError { message: e.to_string() })))?;

    let record = match record {
        Some(r) => r,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(AuthError {
                    message: "No pending verification found for this email. Please request a new code.".into(),
                }),
            ))
        }
    };

    let now = chrono::Utc::now().timestamp();
    if now > record.expires_at {
        let _ = sqlx::query("DELETE FROM email_otps WHERE email = ?").bind(&email).execute(&state.db).await;
        return Err((
            StatusCode::BAD_REQUEST,
            Json(AuthError {
                message: "Verification code has expired. Please request a new code.".into(),
            }),
        ));
    }

    if record.attempts_left <= 0 {
        let _ = sqlx::query("DELETE FROM email_otps WHERE email = ?").bind(&email).execute(&state.db).await;
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(AuthError {
                message: "Maximum verification attempts exceeded. Please request a new code.".into(),
            }),
        ));
    }

    if entered_otp != record.otp_code {
        let _ = sqlx::query("UPDATE email_otps SET attempts_left = attempts_left - 1 WHERE email = ?")
            .bind(&email)
            .execute(&state.db)
            .await;

        let rem = record.attempts_left - 1;
        return Err((
            StatusCode::BAD_REQUEST,
            Json(AuthError {
                message: format!("Invalid verification code. Attempts remaining: {}", rem),
            }),
        ));
    }

    // OTP Verified! Create user account formally
    let user_id = Uuid::new_v4().to_string();
    let res = sqlx::query(
        r#"
        INSERT INTO users (id, username, email, password_hash, role, created_at)
        VALUES (?, ?, ?, ?, 'user', ?)
        "#
    )
    .bind(&user_id)
    .bind(&record.username)
    .bind(&record.email)
    .bind(&record.password_hash)
    .bind(now)
    .execute(&state.db)
    .await;

    if let Err(e) = res {
        return Err((
            StatusCode::CONFLICT,
            Json(AuthError {
                message: format!("Failed to create user: {}", e),
            }),
        ));
    }

    // Clean up used OTP
    let _ = sqlx::query("DELETE FROM email_otps WHERE email = ?").bind(&email).execute(&state.db).await;

    let token = create_jwt(&user_id, &record.username, "user", &state.config.jwt_secret, 7 * 86400)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(AuthError { message: e })))?;

    let user = UserDto {
        id: user_id,
        username: record.username,
        email: record.email,
        role: "user".into(),
        created_at: now,
    };

    Ok((StatusCode::CREATED, Json(AuthResponse { token, user })))
}

/// Login: Username + Password strictly verified via Argon2id
pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<AuthError>)> {
    let username = payload.username.trim();

    if username.is_empty() || payload.password.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(AuthError {
                message: "Username and password are required".into(),
            }),
        ));
    }

    let user = sqlx::query_as::<_, UserRecord>(
        r#"
        SELECT id, username, email, password_hash, role, created_at
        FROM users
        WHERE username = ?
        "#
    )
    .bind(username)
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
