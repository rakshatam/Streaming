use sqlx::{sqlite::SqlitePoolOptions, Pool, Sqlite};
use std::time::Duration;
use tracing::info;

pub type DbPool = Pool<Sqlite>;

pub async fn init_db(database_url: &str) -> Result<DbPool, sqlx::Error> {
    let pool = SqlitePoolOptions::new()
        .max_connections(50)
        .min_connections(5)
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(Duration::from_secs(600))
        .connect(database_url)
        .await?;

    // Enable high-performance concurrency PRAGMAs
    sqlx::query("PRAGMA journal_mode = WAL;").execute(&pool).await?;
    sqlx::query("PRAGMA synchronous = NORMAL;").execute(&pool).await?;
    sqlx::query("PRAGMA cache_size = -64000;").execute(&pool).await?; // 64 MB
    sqlx::query("PRAGMA mmap_size = 268435456;").execute(&pool).await?; // 256 MB
    sqlx::query("PRAGMA temp_store = MEMORY;").execute(&pool).await?;

    // Create tables
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY,
            username TEXT UNIQUE NOT NULL,
            email TEXT UNIQUE NOT NULL,
            password_hash TEXT NOT NULL,
            role TEXT NOT NULL DEFAULT 'user',
            created_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS videos (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            description TEXT NOT NULL,
            category TEXT NOT NULL,
            quality TEXT NOT NULL DEFAULT '1080p',
            stream_url TEXT NOT NULL,
            thumbnail_url TEXT NOT NULL,
            backdrop_url TEXT NOT NULL,
            duration_secs INTEGER NOT NULL DEFAULT 0,
            is_live INTEGER NOT NULL DEFAULT 0,
            is_featured INTEGER NOT NULL DEFAULT 0,
            views_count INTEGER NOT NULL DEFAULT 0,
            rating REAL NOT NULL DEFAULT 4.8,
            tags TEXT NOT NULL DEFAULT '',
            created_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS watch_history (
            user_id TEXT NOT NULL,
            video_id TEXT NOT NULL,
            progress_secs INTEGER NOT NULL DEFAULT 0,
            updated_at INTEGER NOT NULL,
            PRIMARY KEY (user_id, video_id),
            FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE,
            FOREIGN KEY(video_id) REFERENCES videos(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS bookmarks (
            user_id TEXT NOT NULL,
            video_id TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            PRIMARY KEY (user_id, video_id),
            FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE,
            FOREIGN KEY(video_id) REFERENCES videos(id) ON DELETE CASCADE
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS fts_videos USING fts5(
            video_id UNINDEXED,
            title,
            description,
            category,
            tags
        );
        "#
    )
    .execute(&pool)
    .await?;

    seed_database(&pool).await?;

    info!("Database initialized successfully with WAL mode & FTS5 search.");
    Ok(pool)
}

async fn seed_database(pool: &DbPool) -> Result<(), sqlx::Error> {
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM videos")
        .fetch_one(pool)
        .await?;

    if count.0 > 0 {
        return Ok(());
    }

    info!("Seeding curated high-quality 1080p streams and VODs...");

    let now = chrono::Utc::now().timestamp();

    let initial_videos = vec![
        (
            "tears-of-steel",
            "Tears of Steel (1080p / 4K Multi-Bitrate)",
            "Set in a dystopian future, a group of warriors and scientists gather at the Oude Kerk in Amsterdam to stage a desperate attempt to rescue the world from destructive robots.",
            "Sci-Fi",
            "1080p 60fps",
            "https://demo.unified-streaming.com/k8s/features/stable/video/tears-of-steel/tears-of-steel.ism/.m3u8",
            "https://images.unsplash.com/photo-1534447677768-be436bb09401?w=800&q=80",
            "https://images.unsplash.com/photo-1451187580459-43490279c0fa?w=1600&q=85",
            734,
            0,
            1, // featured
            128500,
            4.9,
            "sci-fi, robot, cyberpunk, vfx, 4k, amsterdam, action"
        ),
        (
            "big-buck-bunny",
            "Big Buck Bunny (1080p 60fps Remaster)",
            "A large and lovable rabbit deals with bullying forest creatures in this iconic open-source animation masterpiece rendered in high fidelity 1080p 60fps.",
            "Animation",
            "1080p 60fps",
            "https://test-streams.mux.dev/x36xhzz/x36xhzz.m3u8",
            "https://images.unsplash.com/photo-1579783902614-a3fb3927b675?w=800&q=80",
            "https://images.unsplash.com/photo-1518709268805-4e9042af9f23?w=1600&q=85",
            596,
            0,
            1, // featured
            345000,
            4.8,
            "animation, comedy, 60fps, open-source, family, blender"
        ),
        (
            "action-hollywood",
            "Action Hollywood Movies HD",
            "Non-stop high adrenaline blockbusters, martial arts, vehicle chases, and explosive cinema streamed live in crystal clear 1080p HD.",
            "Movies",
            "1080p HD",
            "https://amg01076-lightningintern-actionhollywood-samsungau-rs69y.amagi.tv/playlist/amg01076-lightningintern-actionhollywood-samsungau/playlist.m3u8",
            "https://images.unsplash.com/photo-1536440136628-849c177e76a1?w=800&q=80",
            "https://images.unsplash.com/photo-1489599849927-2ee91cede3ba?w=1600&q=85",
            0,
            1, // live
            1, // featured
            89400,
            4.7,
            "movies, action, live, thriller, blockbusters, cinema"
        ),
        (
            "history-warfare",
            "History and Warfare Now 1080p",
            "Immersive military history, strategic battles, tactical combat analysis, and deep historical documentaries streamed in full 1080p from Cloudfront CDN.",
            "Documentary",
            "1080p HD",
            "https://d28gzbv9fhdm3h.cloudfront.net/playlist/amg00376-magellantv-historyandwarfarenowaunzin-samsungau/playlist.m3u8",
            "https://images.unsplash.com/photo-1509198397868-475647b2a1e5?w=800&q=80",
            "https://images.unsplash.com/photo-1516339901601-2e1b62dc0c45?w=1600&q=85",
            0,
            1, // live
            0,
            45200,
            4.9,
            "documentary, history, warfare, military, battle, tactical"
        ),
        (
            "nasa-tv-hd",
            "NASA TV HD Live Stream",
            "Live coverage of rocket launches, International Space Station missions, spacewalks, deep space discoveries, and astronomical events in 1080p HD.",
            "Science",
            "1080p 60fps",
            "https://ntv1.akamaized.net/hls/live/2014075/NASA-NTV1-HLS/master.m3u8",
            "https://images.unsplash.com/photo-1451187580459-43490279c0fa?w=800&q=80",
            "https://images.unsplash.com/photo-1446776811953-b23d57bd21aa?w=1600&q=85",
            0,
            1, // live
            1, // featured
            560200,
            5.0,
            "space, nasa, live, iss, science, astronomy, rocket, mars"
        ),
        (
            "dw-english-hd",
            "DW News English 1080p HD Live",
            "International news, investigative journalism, global geopolitics, economic trends, and cultural reports broadcasting live 24/7.",
            "News",
            "1080p HD",
            "https://dwamdstream102.akamaized.net/hls/live/2015525/dwstream102/index.m3u8",
            "https://images.unsplash.com/photo-1504711434969-e33886168f5c?w=800&q=80",
            "https://images.unsplash.com/photo-1495020689067-958852a7765e?w=1600&q=85",
            0,
            1, // live
            0,
            72100,
            4.6,
            "news, world, live, journalism, global, politics"
        ),
        (
            "hardcore-pawn",
            "Hardcore Pawn 1080p HD Channel",
            "Unfiltered negotiations, rare historical artifacts, sports memorabilia, and dramatic pawn shop transactions in downtown Detroit.",
            "Entertainment",
            "1080p HD",
            "https://amg00627-banijay-amg00627c8-samsung-au-1827.playouts.now.amagi.tv/playlist.m3u8",
            "https://images.unsplash.com/photo-1526304640581-d334cdbbf45e?w=800&q=80",
            "https://images.unsplash.com/photo-1559526324-4b87b5e36e44?w=1600&q=85",
            0,
            1, // live
            0,
            63200,
            4.5,
            "entertainment, reality, pawn, detroit, negotiations, collectors"
        ),
        (
            "escape-country",
            "Escape to the Country HD",
            "Breathtaking landscapes, historic estates, rustic cottages, and country home tours across scenic rural Britain in full 1080p.",
            "Lifestyle",
            "1080p HD",
            "https://amg00145-amg00145c11-samsung-au-6579.playouts.now.amagi.tv/playlist.m3u8",
            "https://images.unsplash.com/photo-1513694203232-719a280e022f?w=800&q=80",
            "https://images.unsplash.com/photo-1500382017468-9049fed747ef?w=1600&q=85",
            0,
            1, // live
            0,
            38900,
            4.7,
            "lifestyle, travel, countryside, homes, architecture, nature"
        )
    ];

    for v in initial_videos {
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO videos (
                id, title, description, category, quality, stream_url,
                thumbnail_url, backdrop_url, duration_secs, is_live,
                is_featured, views_count, rating, tags, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(v.0)
        .bind(v.1)
        .bind(v.2)
        .bind(v.3)
        .bind(v.4)
        .bind(v.5)
        .bind(v.6)
        .bind(v.7)
        .bind(v.8)
        .bind(v.9)
        .bind(v.10)
        .bind(v.11)
        .bind(v.12)
        .bind(v.13)
        .bind(now)
        .execute(pool)
        .await?;

        // Seed FTS5 table
        sqlx::query(
            r#"
            INSERT INTO fts_videos (video_id, title, description, category, tags)
            VALUES (?, ?, ?, ?, ?)
            "#
        )
        .bind(v.0)
        .bind(v.1)
        .bind(v.2)
        .bind(v.3)
        .bind(v.13)
        .execute(pool)
        .await?;
    }

    Ok(())
}
