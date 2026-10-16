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
    pub smtp_host: Option<String>,
    pub smtp_port: u16,
    pub smtp_user: Option<String>,
    pub smtp_pass: Option<String>,
    pub smtp_from: String,
}

fn load_dotenv_if_present() {
    let candidate_paths = [".env", "backend/.env", "../backend/.env"];
    for path in &candidate_paths {
        if let Ok(contents) = std::fs::read_to_string(path) {
            for line in contents.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some((k, v)) = line.split_once('=') {
                    let key = k.trim();
                    let val = v.trim().trim_matches('"').trim_matches('\'');
                    if std::env::var(key).is_err() {
                        std::env::set_var(key, val);
                    }
                }
            }
            break;
        }
    }
}

impl Config {
    pub fn from_env() -> Self {
        load_dotenv_if_present();

        let server_host = env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let server_port = env::var("SERVER_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8080);
        let database_url = env::var("DATABASE_URL")
            .unwrap_or_else(|_| "sqlite://streamflow.db?mode=rwc".to_string());
        let hmac_secret = env::var("HMAC_SECRET")
            .unwrap_or_else(|_| "streamflow_hmac_secret_key_default".to_string());
        let jwt_secret = env::var("JWT_SECRET")
            .unwrap_or_else(|_| "streamflow_jwt_secret_key_default".to_string());
        let segment_cache_mb = env::var("SEGMENT_CACHE_MB")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(256); // 256MB in-memory segment cache
        let token_validity_secs = env::var("TOKEN_VALIDITY_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3600); // 1 hour stream session tokens

        let smtp_host = env::var("SMTP_HOST").ok().filter(|s| !s.trim().is_empty());
        let smtp_port = env::var("SMTP_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(587);
        let smtp_user = env::var("SMTP_USER").ok().filter(|s| !s.trim().is_empty());
        let smtp_pass = env::var("SMTP_PASS").ok().filter(|s| !s.trim().is_empty());
        let smtp_from = env::var("SMTP_FROM").unwrap_or_else(|_| "StreamFlow Security <noreply@streamflow.tv>".to_string());

        Config {
            server_host,
            server_port,
            database_url,
            hmac_secret,
            jwt_secret,
            segment_cache_mb,
            token_validity_secs,
            smtp_host,
            smtp_port,
            smtp_user,
            smtp_pass,
            smtp_from,
        }
    }
}

