use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub server_host: String,
    pub server_port: u16,
    pub database_url: String,
    pub hmac_secret: String,
    pub jwt_secret: String,
    pub segment_cache_mb: u64,
    pub token_validity_secs: u64,
}

impl Config {
    pub fn from_env() -> Self {
        let server_host = env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let server_port = env::var("SERVER_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8080);
        let database_url = env::var("DATABASE_URL")
            .unwrap_or_else(|_| "sqlite://streamflow.db?mode=rwc".to_string());
        let hmac_secret = env::var("HMAC_SECRET")
            .unwrap_or_else(|_| "netflix_level_streamflow_ultra_secure_hmac_secret_key_2026".to_string());
        let jwt_secret = env::var("JWT_SECRET")
            .unwrap_or_else(|_| "streamflow_jwt_secret_token_signing_key_super_secure".to_string());
        let segment_cache_mb = env::var("SEGMENT_CACHE_MB")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(256); // 256MB in-memory LRU segment cache
        let token_validity_secs = env::var("TOKEN_VALIDITY_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3600); // 1 hour valid stream session tokens

        Config {
            server_host,
            server_port,
            database_url,
            hmac_secret,
            jwt_secret,
            segment_cache_mb,
            token_validity_secs,
        }
    }
}
