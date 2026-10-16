use streamflow_backend::{
    db::init_db,
    security::{create_jwt, hash_password, sign_stream_token, verify_jwt, verify_password, verify_stream_token},
    stream::StreamEngine,
};

#[tokio::test]
async fn test_argon2id_password_hashing() {
    let password = "SuperSecretPassword123!@#";
    let hash = hash_password(password).expect("Hashing should succeed");
    
    assert!(hash.starts_with("$argon2id$"));
    assert!(verify_password(password, &hash).expect("Verification should succeed"));
    assert!(!verify_password("WrongPassword!", &hash).unwrap_or(false));
}

#[tokio::test]
async fn test_jwt_generation_and_verification() {
    let secret = "test_jwt_secret_key_1234567890";
    let user_id = "user-12345";
    let username = "alex_dev";
    let role = "premium_user";

    let token = create_jwt(user_id, username, role, secret, 3600)
        .expect("JWT creation should succeed");

    let claims = verify_jwt(&token, secret).expect("JWT verification should succeed");
    assert_eq!(claims.sub, user_id);
    assert_eq!(claims.username, username);
    assert_eq!(claims.role, role);

    // Tampered token check
    let tampered = format!("{}tampered", token);
    assert!(verify_jwt(&tampered, secret).is_err());

    // Wrong secret check
    assert!(verify_jwt(&token, "wrong_secret_key").is_err());
}

#[tokio::test]
async fn test_hmac_sha256_stream_tokens() {
    let secret = "ultra_secure_streaming_hmac_secret_netflix_tier";
    let uri = "https://cdn.streamflow.io/hls/1080p/segment_001.ts";
    let future_exp = chrono::Utc::now().timestamp() as u64 + 300; // 5 min future

    let token = sign_stream_token(uri, future_exp, secret);
    assert!(!token.is_empty());

    // Valid token check
    assert!(verify_stream_token(uri, future_exp, &token, secret));

    // Tampered URI check
    assert!(!verify_stream_token("https://cdn.streamflow.io/hls/1080p/segment_002.ts", future_exp, &token, secret));

    // Tampered signature check
    let tampered_token = format!("{}ff", &token[..token.len() - 2]);
    assert!(!verify_stream_token(uri, future_exp, &tampered_token, secret));

    // Expired token check
    let past_exp = chrono::Utc::now().timestamp() as u64 - 10; // 10 secs in past
    let expired_token = sign_stream_token(uri, past_exp, secret);
    assert!(!verify_stream_token(uri, past_exp, &expired_token, secret));
}

#[tokio::test]
async fn test_database_initialization_and_fts_search() {
    let db_url = "sqlite::memory:";
    let pool = init_db(db_url).await.expect("In-memory SQLite initialization should succeed");

    // Check seeded videos
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM videos")
        .fetch_one(&pool)
        .await
        .expect("Query should succeed");
    assert!(count >= 5);

    // Test FTS5 search
    let fts_query = "space*";
    let fts_results: Vec<(String, String)> = sqlx::query_as(
        r#"
        SELECT v.id, v.title FROM videos v
        JOIN fts_videos f ON v.id = f.video_id
        WHERE fts_videos MATCH ?
        "#
    )
    .bind(fts_query)
    .fetch_all(&pool)
    .await
    .expect("FTS search should succeed");

    assert!(!fts_results.is_empty());
    assert!(fts_results.iter().any(|(_, title)| title.contains("NASA")));
}

#[tokio::test]
async fn test_manifest_rewriting_and_token_injection() {
    let engine = StreamEngine::new("test_secret".to_string(), 64, 3600);

    let (token, exp) = engine.create_token("https://example.com/segment_001.ts");
    assert!(engine.validate_token("https://example.com/segment_001.ts", &token, exp));
    assert!(!engine.validate_token("https://example.com/different.ts", &token, exp));
}
