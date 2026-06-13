use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use crate::errors::AppError;

/// Claims embedded in the JWT.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject – the user's UUID
    pub sub: String,
    /// Username (convenience; don't rely on this for auth decisions, use `sub`)
    pub username: String,
    /// Expiry (Unix timestamp)
    pub exp: i64,
    /// Issued-at
    pub iat: i64,

    pub is_admin: bool
}

const TOKEN_EXPIRY_HOURS: i64 = 72;

/// Sign a new JWT for the given user.
pub fn create_token(user_id: &str, username: &str, is_admin: bool, secret: &str) -> Result<String, AppError> {
    let now = Utc::now();
    let claims = Claims {
        sub: user_id.to_owned(),
        username: username.to_owned(),
        iat: now.timestamp(),
        exp: (now + Duration::hours(TOKEN_EXPIRY_HOURS)).timestamp(),
        is_admin,
    };
    Ok(encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
        
    )?)
}

/// Validate a JWT and return its claims.
pub fn validate_token(token: &str, secret: &str) -> Result<Claims, AppError> {
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )?;
    Ok(data.claims)
}