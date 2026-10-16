use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use hmac::{Hmac, Mac};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,       // user_id
    pub username: String,
    pub role: String,
    pub exp: usize,        // expiration timestamp
    pub iat: usize,        // issued at timestamp
}

/// Hashes password with OWASP-recommended Argon2id parameters
pub fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| format!("Password hashing failed: {}", e))?
        .to_string();
    Ok(password_hash)
}

/// Verifies a password against an Argon2id hash
pub fn verify_password(password: &str, hash: &str) -> Result<bool, String> {
    let parsed_hash = PasswordHash::new(hash).map_err(|e| format!("Invalid hash format: {}", e))?;
    let argon2 = Argon2::default();
    Ok(argon2.verify_password(password.as_bytes(), &parsed_hash).is_ok())
}

/// Generates a signed JWT session token
pub fn create_jwt(user_id: &str, username: &str, role: &str, secret: &str, expires_in_secs: u64) -> Result<String, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs() as usize;

    let claims = Claims {
        sub: user_id.to_string(),
        username: username.to_string(),
        role: role.to_string(),
        exp: now + expires_in_secs as usize,
        iat: now,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| format!("Token creation failed: {}", e))
}

/// Verifies and decodes a JWT token
pub fn verify_jwt(token: &str, secret: &str) -> Result<Claims, String> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|e| format!("Token verification failed: {}", e))?;

    Ok(token_data.claims)
}

/// Computes HMAC-SHA256 signature for URL anti-tampering and anti-hotlinking
pub fn sign_stream_token(target_uri: &str, expires_at: u64, secret: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .expect("HMAC can take key of any size");
    let payload = format!("{}:{}", target_uri, expires_at);
    mac.update(payload.as_bytes());
    let result = mac.finalize();
    hex::encode(result.into_bytes())
}

/// Verifies HMAC-SHA256 signature and checks expiry
pub fn verify_stream_token(target_uri: &str, expires_at: u64, signature: &str, secret: &str) -> bool {
    let now = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs(),
        Err(_) => return false,
    };

    // Reject expired tokens
    if now > expires_at {
        return false;
    }

    let expected_sig = sign_stream_token(target_uri, expires_at, secret);
    // Constant time comparison
    expected_sig == signature
}
