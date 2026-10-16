use sqlx::{sqlite::SqlitePoolOptions, Pool, Sqlite};
use std::time::Duration;
use tracing::info;
use uuid::Uuid;

use crate::security::hash_password;

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
    // Seed default demo account if not exists
    let demo_exists: Option<(String,)> = sqlx::query_as("SELECT id FROM users WHERE username = 'demo'")
        .fetch_optional(pool)
        .await?;

    if demo_exists.is_none() {
        if let Ok(hashed_pw) = hash_password("demo123") {
            let demo_id = Uuid::new_v4().to_string();
            let now = chrono::Utc::now().timestamp();
            let _ = sqlx::query(
                "INSERT OR IGNORE INTO users (id, username, email, password_hash, role, created_at) VALUES (?, 'demo', 'demo@streamflow.io', ?, 'user', ?)"
            )
            .bind(&demo_id)
            .bind(&hashed_pw)
            .bind(now)
            .execute(pool)
            .await;
            info!("Demo account seeded: username: demo, password: demo123");
        }
    }

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM videos")
        .fetch_one(pool)
        .await?;

    if count.0 >= 20 {
        return Ok(());
    }

    info!("Seeding expanded catalog of 25+ high-quality 1080p streams and VODs...");

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
            "https://images.unsplash.com/photo-1535378917042-10a22c95931a?w=800&q=80",
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
            "red-bull-tv-hd",
            "Red Bull TV HD Extreme Sports",
            "World championship downhill biking, cliff diving, surfing monster waves, Formula 1 behind-the-scenes, and extreme sporting spectacles live in 1080p.",
            "Sports",
            "1080p HD",
            "https://rbmn-live.akamaized.net/hls/live/590964/BoRB-AT/master.m3u8",
            "https://images.unsplash.com/photo-1517649763962-0c623266ddc0?w=800&q=80",
            "https://images.unsplash.com/photo-1551698618-1dfe5d97d256?w=1600&q=85",
            0,
            1, // live
            1, // featured
            214000,
            4.9,
            "sports, redbull, extreme, surfing, biking, action, racing"
        ),
        (
            "sportsgrid-hd",
            "SportsGrid Live 1080p",
            "Real-time sports analytics, live coverage, fantasy analysis, game breakdowns, and odds analysis from top commentators.",
            "Sports",
            "1080p HD",
            "https://sportsgrid-klowdtv.amagi.tv/playlist.m3u8",
            "https://images.unsplash.com/photo-1508098682722-e99c43a406b2?w=800&q=80",
            "https://images.unsplash.com/photo-1461896836934-ffe607ba8211?w=1600&q=85",
            0,
            1, // live
            0,
            98200,
            4.6,
            "sports, live, grid, analysis, football, basketball"
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
        ),
        (
            "highway-thru-hell",
            "Highway Thru Hell 1080p",
            "Heavy rescue tow truck operators battle extreme weather and hazardous terrain on the treacherous Coquihalla Highway in British Columbia.",
            "Entertainment",
            "1080p HD",
            "https://amg00627-amg00627c15-samsung-au-2899.playouts.now.amagi.tv/playlist/amg00627-banijayfast-highwaythruhell-samsungau/playlist.m3u8",
            "https://images.unsplash.com/photo-1519003722824-194d4455a60c?w=800&q=80",
            "https://images.unsplash.com/photo-1506744038136-46273834b3fb?w=1600&q=85",
            0,
            1, // live
            0,
            81500,
            4.8,
            "reality, trucks, highway, rescue, extreme, snow"
        ),
        (
            "surf-cinema",
            "Surf Cinema 1080p HD",
            "Pioneering surf filmmakers showcase legendary swells, barrel riders, pristine Pacific breaks, and nomadic beach expeditions.",
            "Sports",
            "1080p HD",
            "https://amg13231-actve-amg13231c4-samsung-au-2990.playouts.now.amagi.tv/playlist/amg13231-actvefast-surfer-samsungau/playlist.m3u8",
            "https://images.unsplash.com/photo-1502680390469-be75c86b636f?w=800&q=80",
            "https://images.unsplash.com/photo-1455729552865-3658a5d39692?w=1600&q=85",
            0,
            1, // live
            0,
            44200,
            4.9,
            "sports, surfing, ocean, waves, cinema, beach"
        ),
        (
            "pbs-science",
            "PBS Science Frontier 1080p",
            "Deep dives into quantum physics, biotechnology, cognitive neuroscience, cosmology, and planetary science produced by premier documentary filmmakers.",
            "Science",
            "1080p HD",
            "https://amg02333-pbs-amg02333c13-samsung-au-6858.playouts.now.amagi.tv/playlist.m3u8",
            "https://images.unsplash.com/photo-1507668077129-56e32842fceb?w=800&q=80",
            "https://images.unsplash.com/photo-1532094349884-543bc11b234d?w=1600&q=85",
            0,
            1, // live
            0,
            57300,
            4.9,
            "science, pbs, nature, physics, biology, cosmos"
        ),
        (
            "pbs-history",
            "PBS History Vault 1080p",
            "Archival footage, monumental historical discoveries, archaeological breakthroughs, and landmark human events.",
            "Documentary",
            "1080p HD",
            "https://amg02333-pbs-amg02333c1-samsung-au-1253.playouts.now.amagi.tv/playlist/amg02333-pbsdistributionfast-pbshistory-samsungau/playlist.m3u8",
            "https://images.unsplash.com/photo-1461360370896-922624d12aa1?w=800&q=80",
            "https://images.unsplash.com/photo-1457369804613-52c61a468e7d?w=1600&q=85",
            0,
            1, // live
            0,
            61200,
            4.8,
            "history, pbs, archives, documentary, civilization"
        ),
        (
            "motorsport-1",
            "MTRSPT 1 Racing Network",
            "Rally cross, GT endurance, drifting exhibitions, superbikes, and high-octane motorsport championships around the globe.",
            "Sports",
            "1080p HD",
            "https://amg02873-kravemedia-mtrspt1-samsungau-2anp4.amagi.tv/playlist/amg02873-kravemedia-mtrspt1-samsungau/playlist.m3u8",
            "https://images.unsplash.com/photo-1568605117036-5fe5e7bab0b7?w=800&q=80",
            "https://images.unsplash.com/photo-1552519507-da3b142c6e3d?w=1600&q=85",
            0,
            1, // live
            0,
            93200,
            4.7,
            "racing, motorsport, cars, rally, speed, sports"
        ),
        (
            "pulse-thrillers",
            "Pulse Thriller Cinema",
            "Psychological thrillers, gripping suspense dramas, espionage intrigue, and edge-of-your-seat narrative mysteries.",
            "Movies",
            "1080p HD",
            "https://amg01076-lightningintern-pulse-samsungau-4a45e.amagi.tv/playlist/amg01076-lightningintern-pulse-samsungau/playlist.m3u8",
            "https://images.unsplash.com/photo-1478720568477-152d9b164e26?w=800&q=80",
            "https://images.unsplash.com/photo-1518676590629-3dcbd9c5a5c9?w=1600&q=85",
            0,
            1, // live
            0,
            68100,
            4.6,
            "movies, thriller, suspense, drama, mystery"
        ),
        (
            "real-crime-hd",
            "Real Crime Files 1080p",
            "Forensic investigations, cold case resolutions, courtroom drama, and true crime documentaries revealing unbelievable real-life mysteries.",
            "Documentary",
            "1080p HD",
            "https://lds-realcrime-samsungau.amagi.tv/playlist.m3u8",
            "https://images.unsplash.com/photo-1453873531674-2151abc01707?w=800&q=80",
            "https://images.unsplash.com/photo-1589829545856-d10d557cf95f?w=1600&q=85",
            0,
            1, // live
            0,
            75900,
            4.7,
            "crime, forensic, investigation, mystery, documentary"
        ),
        (
            "leadstory-news",
            "LeadStory Breaking News HD",
            "Fast-paced headlines, live global coverage, market intelligence, and breaking international developments.",
            "News",
            "1080p HD",
            "https://amg02703-leadstory-leadstory-samsungau-rr75f.amagi.tv/playlist/amg02703-leadstory-leadstory-samsungau/playlist.m3u8",
            "https://images.unsplash.com/photo-1585829365295-ab7cd400c167?w=800&q=80",
            "https://images.unsplash.com/photo-1495020689067-958852a7765e?w=1600&q=85",
            0,
            1, // live
            0,
            51200,
            4.5,
            "news, breaking, live, global, headlines"
        ),
        (
            "wicked-tuna",
            "The Wicked Tuna Channel HD",
            "Commercial fishermen in Gloucester, Massachusetts face the unforgiving North Atlantic to reel in lucrative bluefin tuna.",
            "Entertainment",
            "1080p HD",
            "https://amg00353-lionsgatestudio-wickedtuna-samsungau-26lq4.amagi.tv/playlist/amg00353-lionsgatestudio-wickedtuna-samsungau/playlist.m3u8",
            "https://images.unsplash.com/photo-1544551763-46a013bb70d5?w=800&q=80",
            "https://images.unsplash.com/photo-1518837695005-2083093ee35b?w=1600&q=85",
            0,
            1, // live
            0,
            67000,
            4.8,
            "fishing, ocean, adventure, reality, atlantic"
        ),
        (
            "hotel-inspector",
            "The Hotel Inspector HD",
            "Renowned hotelier Alex Polizzi turns around struggling British guest houses, bed and breakfasts, and boutique hotels.",
            "Lifestyle",
            "1080p HD",
            "https://amg00654-itv-amg00654c4-samsung-au-998.playouts.now.amagi.tv/playlist/amg00654-itvstudiosfast-hotelinspector-samsungau/playlist.m3u8",
            "https://images.unsplash.com/photo-1566073771259-6a8506099945?w=800&q=80",
            "https://images.unsplash.com/photo-1582719508461-905c673771fd?w=1600&q=85",
            0,
            1, // live
            0,
            43100,
            4.6,
            "hotel, hospitality, lifestyle, travel, business"
        ),
        (
            "eight-out-of-ten-cats",
            "8 Out of 10 Cats Comedy Channel",
            "Hilarious British panel show testing knowledge of current events, opinion polls, and bizarre statistics hosted by Jimmy Carr.",
            "Entertainment",
            "1080p HD",
            "https://amg00627-amg00627c37-samsung-au-4294.playouts.now.amagi.tv/playlist/amg00627-banijayfast-8outof10cats-samsungau/playlist.m3u8",
            "https://images.unsplash.com/photo-1514306191717-452ec28c7814?w=800&q=80",
            "https://images.unsplash.com/photo-1507676184212-d03ab07a01bf?w=1600&q=85",
            0,
            1, // live
            0,
            104000,
            4.9,
            "comedy, entertainment, british, humor, panel"
        )
    ];

    for v in initial_videos {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO videos (
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
        let _ = sqlx::query(
            r#"
            INSERT OR REPLACE INTO fts_videos (video_id, title, description, category, tags)
            VALUES (?, ?, ?, ?, ?)
            "#
        )
        .bind(v.0)
        .bind(v.1)
        .bind(v.2)
        .bind(v.3)
        .bind(v.13)
        .execute(pool)
        .await;
    }

    Ok(())
}
