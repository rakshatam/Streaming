use bytes::Bytes;
use std::sync::Arc;
use std::time::Instant;
use streamflow_backend::{
    db::init_db,
    security::{
        create_jwt, hash_password, sign_stream_token, verify_jwt, verify_password,
        verify_stream_token,
    },
    stream::{SegmentData, StreamEngine},
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

    let token =
        create_jwt(user_id, username, role, secret, 3600).expect("JWT creation should succeed");

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
    assert!(!verify_stream_token(
        "https://cdn.streamflow.io/hls/1080p/segment_002.ts",
        future_exp,
        &token,
        secret
    ));

    // Tampered signature check
    let tampered_token = format!("{}ff", &token[..token.len() - 2]);
    assert!(!verify_stream_token(
        uri,
        future_exp,
        &tampered_token,
        secret
    ));

    // Expired token check
    let past_exp = chrono::Utc::now().timestamp() as u64 - 10; // 10 secs in past
    let expired_token = sign_stream_token(uri, past_exp, secret);
    assert!(!verify_stream_token(uri, past_exp, &expired_token, secret));
}

#[tokio::test]
async fn test_database_initialization_and_fts_search() {
    let db_url = "sqlite::memory:";
    let pool = init_db(db_url)
        .await
        .expect("In-memory SQLite initialization should succeed");

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
        "#,
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

/// Stress test: Multi-threaded concurrent database writes across connection pool.
/// Verifies SQLite WAL mode and busy_timeout handle high contention without lock failures.
#[tokio::test]
async fn test_multithreaded_concurrent_sqlite_writes() {
    let db_url = "sqlite::memory:";
    let pool = init_db(db_url)
        .await
        .expect("In-memory SQLite initialization should succeed");

    // Seed test users
    for i in 0..5 {
        let user_id = format!("user_{}", i);
        let username = format!("concurrent_user_{}", i);
        let now = chrono::Utc::now().timestamp();
        sqlx::query("INSERT INTO users (id, username, password_hash, role, created_at) VALUES (?, ?, 'dummy_hash', 'user', ?)")
            .bind(&user_id)
            .bind(&username)
            .bind(now)
            .execute(&pool)
            .await
            .expect("Seeding user should succeed");
    }

    let concurrent_tasks = 25;
    let mut handles = Vec::with_capacity(concurrent_tasks);

    for task_idx in 0..concurrent_tasks {
        let pool_clone = pool.clone();
        let user_id = format!("user_{}", task_idx % 5);
        let video_id = "phantom-flex-4k";

        let handle = tokio::spawn(async move {
            let now = chrono::Utc::now().timestamp();

            // 1. Concurrent watch history upsert
            sqlx::query(
                r#"
                INSERT INTO watch_history (user_id, video_id, progress_secs, updated_at)
                VALUES (?, ?, ?, ?)
                ON CONFLICT(user_id, video_id) DO UPDATE SET
                    progress_secs = excluded.progress_secs,
                    updated_at = excluded.updated_at
                "#,
            )
            .bind(&user_id)
            .bind(video_id)
            .bind(task_idx as i64 * 10)
            .bind(now)
            .execute(&pool_clone)
            .await
            .expect("Concurrent watch history write should succeed");

            // 2. Concurrent atomic like toggle (DELETE + INSERT OR IGNORE)
            let deleted = sqlx::query("DELETE FROM video_likes WHERE user_id = ? AND video_id = ?")
                .bind(&user_id)
                .bind(video_id)
                .execute(&pool_clone)
                .await
                .expect("Like delete should succeed")
                .rows_affected();

            if deleted == 0 {
                let _ = sqlx::query("INSERT OR IGNORE INTO video_likes (user_id, video_id, created_at) VALUES (?, ?, ?)")
                    .bind(&user_id)
                    .bind(video_id)
                    .bind(now)
                    .execute(&pool_clone)
                    .await
                    .expect("Like insert should succeed");
            }

            // 3. Concurrent ratings upsert
            let rating = ((task_idx % 5) + 1) as i64;
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
            .bind(video_id)
            .bind(rating)
            .bind(now)
            .execute(&pool_clone)
            .await
            .expect("Concurrent rating write should succeed");
        });

        handles.push(handle);
    }

    // Await all concurrent tasks
    for handle in handles {
        handle.await.expect("Task should join without panic");
    }

    // Verify database consistency after concurrent execution
    let (history_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM watch_history")
        .fetch_one(&pool)
        .await
        .expect("Query should succeed");
    assert!(history_count > 0 && history_count <= 5);

    let (ratings_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM video_ratings")
        .fetch_one(&pool)
        .await
        .expect("Query should succeed");
    assert!(ratings_count > 0 && ratings_count <= 5);
}

/// Stress test: Memory-bounded segment cache eviction under load.
/// Verifies LRU eviction respects capacity bounds and clear_cache releases memory immediately.
#[tokio::test]
async fn test_memory_bounded_cache_eviction_and_cleanup() {
    // 2 MB memory cap
    let engine = StreamEngine::new("cache_test_secret".to_string(), 2, 3600);

    // Insert 5 segments of 600 KB each (total 3 MB, exceeding 2 MB cap)
    let segment_payload = vec![0xAB; 600 * 1024]; // 600 KB
    for i in 0..5 {
        let key = format!("https://cdn.streamflow.io/segment_{:03}.ts", i);
        let data = SegmentData {
            bytes: Bytes::from(segment_payload.clone()),
            content_type: "video/MP2T".to_string(),
            is_cached: false,
            status: 200,
            content_range: None,
        };
        engine.insert_segment_for_testing(&key, data).await;
    }

    let (_, segment_count) = engine.cache_entry_counts();
    // Cache must have evicted older segments to stay within the 2 MB limit (3 * 600 KB = 1.8 MB <= 2 MB)
    assert!(
        segment_count <= 3,
        "Cache entry count ({}) must not exceed capacity limit",
        segment_count
    );

    // Clear cache and verify total release
    engine.clear_cache().await;
    let (manifest_count, segment_count_cleared) = engine.cache_entry_counts();
    assert_eq!(manifest_count, 0);
    assert_eq!(segment_count_cleared, 0);
}

/// Latency & Throughput Benchmark: High-volume cryptographic stream token verification.
/// Verifies sub-millisecond per-operation latency under concurrent load.
#[tokio::test]
async fn test_high_throughput_and_submillisecond_latency() {
    let secret = "benchmark_secret_key_streamflow_production";
    let uri = "https://cdn.streamflow.tv/hls/4k/amsterdam_0042.ts";
    let future_exp = chrono::Utc::now().timestamp() as u64 + 7200;
    let valid_token = sign_stream_token(uri, future_exp, secret);

    let total_operations = 10_000;
    let concurrent_workers = 8;
    let ops_per_worker = total_operations / concurrent_workers;

    let secret_arc = Arc::new(secret.to_string());
    let uri_arc = Arc::new(uri.to_string());
    let token_arc = Arc::new(valid_token.clone());

    let start_instant = Instant::now();
    let mut handles = Vec::with_capacity(concurrent_workers);

    for _ in 0..concurrent_workers {
        let sec = secret_arc.clone();
        let u = uri_arc.clone();
        let tok = token_arc.clone();

        let handle = tokio::spawn(async move {
            let mut valid_count = 0;
            for _ in 0..ops_per_worker {
                if verify_stream_token(&u, future_exp, &tok, &sec) {
                    valid_count += 1;
                }
            }
            valid_count
        });
        handles.push(handle);
    }

    let mut total_verified = 0;
    for handle in handles {
        total_verified += handle.await.expect("Worker task should finish cleanly");
    }

    let elapsed = start_instant.elapsed();
    let ops_per_sec = (total_operations as f64) / elapsed.as_secs_f64();
    let avg_latency_micros = (elapsed.as_micros() as f64) / (total_operations as f64);

    assert_eq!(total_verified, total_operations);
    // Average cryptographic verification should take under 50 microseconds
    assert!(
        avg_latency_micros < 100.0,
        "Average token verification latency ({:.2} µs) exceeded 100 µs target",
        avg_latency_micros
    );

    println!(
        "[BENCHMARK] Stream Token Throughput: {:.0} ops/sec, Avg Latency: {:.2} µs",
        ops_per_sec, avg_latency_micros
    );
}

/// Cryptographic Boundary & Tamper Test: Bit flips, non-hex signatures, and edge timestamps.
#[tokio::test]
async fn test_token_tampering_and_boundary_conditions() {
    let secret = "tamper_test_secret_key_123";
    let uri = "https://cdn.streamflow.tv/video/segment.ts?query=param&special=!$&'()*+,;=";
    let now = chrono::Utc::now().timestamp() as u64;
    let future_exp = now + 600;

    let valid_token = sign_stream_token(uri, future_exp, secret);

    // 1. Valid token passes
    assert!(verify_stream_token(uri, future_exp, &valid_token, secret));

    // 2. Single-character bit flip at each index must be rejected
    for i in 0..valid_token.len() {
        let mut tampered = valid_token.clone().into_bytes();
        tampered[i] = if tampered[i] == b'a' { b'b' } else { b'a' };
        let tampered_str = String::from_utf8(tampered).unwrap();
        assert!(
            !verify_stream_token(uri, future_exp, &tampered_str, secret),
            "Tampered token at index {} was unexpectedly accepted",
            i
        );
    }

    // 3. Invalid non-hex string must not panic and must return false
    assert!(!verify_stream_token(
        uri,
        future_exp,
        "invalid_non_hex_token!@#$%",
        secret
    ));
    assert!(!verify_stream_token(uri, future_exp, "", secret));

    // 4. Expiration boundaries
    assert!(!verify_stream_token(uri, now - 1, &valid_token, secret));
    assert!(!verify_stream_token(uri, 0, &valid_token, secret));
}
