//! Authentication: JWT token generation/verification and bcrypt password hashing.

use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

/// JWT secret key configuration.
#[derive(Clone)]
pub struct JwtConfig {
    pub secret: String,
}

/// JWT claims payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// User ID.
    pub sub: String,
    /// Username.
    pub username: String,
    /// User role names.
    pub roles: Vec<String>,
    /// Issued at (Unix timestamp).
    pub iat: u64,
    /// Expiration (Unix timestamp).
    pub exp: u64,
}

/// Default token expiration: 24 hours.
const TOKEN_EXPIRATION_SECS: u64 = 24 * 60 * 60;

/// Generate a JWT token for a user.
pub fn generate_token(
    secret: &str,
    user_id: i64,
    username: &str,
    roles: Vec<String>,
) -> Result<String, String> {
    let now = chrono::Utc::now().timestamp() as u64;
    let claims = Claims {
        sub: user_id.to_string(),
        username: username.to_string(),
        roles,
        iat: now,
        exp: now + TOKEN_EXPIRATION_SECS,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| format!("Failed to generate token: {}", e))
}

/// Verify and decode a JWT token.
pub fn verify_token(secret: &str, token: &str) -> Result<Claims, String> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|e| format!("Invalid token: {}", e))?;
    Ok(token_data.claims)
}

/// Hash a password with bcrypt.
pub fn hash_password(password: &str) -> Result<String, String> {
    bcrypt::hash(password, bcrypt::DEFAULT_COST).map_err(|e| format!("Failed to hash password: {}", e))
}

/// Verify a password against a bcrypt hash.
pub fn verify_password(password: &str, hash: &str) -> Result<bool, String> {
    bcrypt::verify(password, hash).map_err(|e| format!("Failed to verify password: {}", e))
}

/// Extract token from an Authorization header value.
/// Expects format: `Bearer <token>`
pub fn extract_bearer_token(auth_header: &str) -> Option<&str> {
    let parts: Vec<&str> = auth_header.splitn(2, ' ').collect();
    if parts.len() == 2 && parts[0].eq_ignore_ascii_case("Bearer") {
        Some(parts[1].trim())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_hashing() {
        let hash = hash_password("test_password").unwrap();
        assert!(verify_password("test_password", &hash).unwrap());
        assert!(!verify_password("wrong_password", &hash).unwrap());
    }

    #[test]
    fn test_jwt_roundtrip() {
        let secret = "test_secret_key";
        let roles = vec!["admin".to_string()];
        let token = generate_token(secret, 1, "admin", roles.clone()).unwrap();
        let claims = verify_token(secret, &token).unwrap();
        assert_eq!(claims.sub, "1");
        assert_eq!(claims.username, "admin");
        assert_eq!(claims.roles, roles);
    }

    #[test]
    fn test_jwt_invalid_secret() {
        let token = generate_token("secret1", 1, "admin", vec!["admin".to_string()]).unwrap();
        assert!(verify_token("secret2", &token).is_err());
    }

    #[test]
    fn test_extract_bearer_token() {
        assert_eq!(extract_bearer_token("Bearer abc123"), Some("abc123"));
        assert_eq!(extract_bearer_token("bearer abc123"), Some("abc123"));
        assert_eq!(extract_bearer_token("abc123"), None);
        assert_eq!(extract_bearer_token("Basic abc123"), None);
    }
}
