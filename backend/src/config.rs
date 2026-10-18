use std::env;

#[derive(Clone)]
pub struct Config {
    pub server_host: String,
    pub server_port: u16,
    pub database_url: String,
    pub hmac_secret: String,
    pub jwt_secret: String,
    pub cache_admin_token: Option<String>,
    pub cors_allowed_origins: Vec<String>,
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
    for path in candidate_paths {
        if let Ok(contents) = std::fs::read_to_string(path) {
            for line in contents.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some((key, value)) = line.split_once('=') {
                    let key = key.trim();
                    let value = value.trim().trim_matches('"').trim_matches('\'');
                    if env::var_os(key).is_none() {
                        env::set_var(key, value);
                    }
                }
            }
        }
    }
}

fn required_secret(name: &str) -> Result<String, String> {
    let value = env::var(name)
        .map_err(|_| format!("{name} must be set to a random secret of at least 32 bytes"))?;
    if value.trim().len() < 32 || value.starts_with("replace-with-") {
        return Err(format!(
            "{name} must be set to a random secret of at least 32 bytes"
        ));
    }
    Ok(value)
}

fn parse_env<T>(name: &str, default: T) -> Result<T, String>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    match env::var(name) {
        Ok(value) => value
            .parse()
            .map_err(|error| format!("{name} has an invalid value: {error}")),
        Err(env::VarError::NotPresent) => Ok(default),
        Err(error) => Err(format!("{name} is not valid Unicode: {error}")),
    }
}

fn normalize_origin(origin: &str) -> Result<String, String> {
    let parsed = url::Url::parse(origin)
        .map_err(|_| "CORS_ALLOWED_ORIGINS contains an invalid URL".to_string())?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.path() != "/"
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err("CORS_ALLOWED_ORIGINS must contain HTTP(S) origins without paths, credentials, queries, or fragments".into());
    }
    Ok(parsed.origin().ascii_serialization())
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        load_dotenv_if_present();

        let server_host = env::var("SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let server_port = match env::var("PORT") {
            Ok(port) => port
                .parse()
                .map_err(|error| format!("PORT has an invalid value: {error}"))?,
            Err(_) => parse_env("SERVER_PORT", 8080)?,
        };
        if server_port == 0 {
            return Err("The server port must be between 1 and 65535".into());
        }
        let database_url = env::var("DATABASE_URL")
            .unwrap_or_else(|_| "sqlite://streamflow.db?mode=rwc".to_string());
        let hmac_secret = required_secret("HMAC_SECRET")?;
        let jwt_secret = required_secret("JWT_SECRET")?;
        if hmac_secret == jwt_secret {
            return Err("HMAC_SECRET and JWT_SECRET must be different values".into());
        }

        let cache_admin_token = env::var("CACHE_ADMIN_TOKEN")
            .ok()
            .filter(|value| !value.trim().is_empty());
        if let Some(token) = &cache_admin_token {
            if token.len() < 32 || token.starts_with("replace-with-") {
                return Err(
                    "CACHE_ADMIN_TOKEN must be a random secret of at least 32 bytes".into(),
                );
            }
        }

        let cors_allowed_origins = env::var("CORS_ALLOWED_ORIGINS")
            .unwrap_or_else(|_| "http://localhost:3000,http://127.0.0.1:3000".to_string())
            .split(',')
            .map(str::trim)
            .filter(|origin| !origin.is_empty())
            .map(normalize_origin)
            .collect::<Result<Vec<_>, _>>()?;
        if cors_allowed_origins.is_empty() {
            return Err("CORS_ALLOWED_ORIGINS must contain at least one HTTP(S) origin".into());
        }

        let segment_cache_mb = parse_env("SEGMENT_CACHE_MB", 64_u64)?;
        if !(1..=128).contains(&segment_cache_mb) {
            return Err("SEGMENT_CACHE_MB must be between 1 and 128".into());
        }

        let token_validity_secs = parse_env("TOKEN_VALIDITY_SECS", 3600_u64)?;
        if !(60..=86_400).contains(&token_validity_secs) {
            return Err("TOKEN_VALIDITY_SECS must be between 60 and 86400".into());
        }

        let smtp_host = env::var("SMTP_HOST")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let smtp_port = parse_env("SMTP_PORT", 587_u16)?;
        if smtp_port == 0 {
            return Err("SMTP_PORT must be between 1 and 65535".into());
        }
        let smtp_user = env::var("SMTP_USER")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let smtp_pass = env::var("SMTP_PASS")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let smtp_from = env::var("SMTP_FROM")
            .unwrap_or_else(|_| "StreamFlow Security <noreply@streamflow.tv>".to_string());

        Ok(Config {
            server_host,
            server_port,
            database_url,
            hmac_secret,
            jwt_secret,
            cache_admin_token,
            cors_allowed_origins,
            segment_cache_mb,
            token_validity_secs,
            smtp_host,
            smtp_port,
            smtp_user,
            smtp_pass,
            smtp_from,
        })
    }
}
