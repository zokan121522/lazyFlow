use axum::extract::State;
use axum::Json;
use chrono::Utc;
use jsonwebtoken::{encode, EncodingKey, Header};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{
    AuthResponse, ChallengeResponse, Claims, LoginRequest, RegisterRequest, User,
};
use crate::AppState;

// ── GET /api/auth/challenge ──────────────────────────────────────────────────
// Returns a challenge string + difficulty for proof-of-work

pub async fn challenge(
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Json<ChallengeResponse> {
    let username = params.get("username").map(|s| s.as_str()).unwrap_or("anonymous");
    let ts = Utc::now().timestamp();
    let challenge = format!("register_{}_{}", username, ts);

    Json(ChallengeResponse {
        challenge,
        difficulty: 4, // 4 zero nibbles = ~65k hashes average
    })
}

// ── POST /api/auth/register ──────────────────────────────────────────────────

pub async fn register(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    // Validate input
    if req.username.len() < 3 || req.username.len() > 64 {
        return Err(AppError::BadRequest("username must be 3-64 characters".into()));
    }
    if req.password.len() < 8 {
        return Err(AppError::BadRequest("password must be at least 8 characters".into()));
    }

    // Verify proof-of-work
    if !verify_pow(&req.pow_challenge, req.pow_nonce, 4) {
        return Err(AppError::BadRequest("invalid proof-of-work".into()));
    }

    // Check challenge is not too old (5 min)
    if let Some(ts_str) = req.pow_challenge.strip_prefix("register_") {
        let parts: Vec<&str> = ts_str.split('_').collect();
        if let Some(timestamp) = parts.last().and_then(|s| s.parse::<i64>().ok()) {
            let age = Utc::now().timestamp() - timestamp;
            if age > 300 {
                return Err(AppError::BadRequest("challenge expired, request a new one".into()));
            }
        }
    }

    // Hash password
    let password_hash = bcrypt::hash(&req.password, bcrypt::DEFAULT_COST)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Insert user
    let user = sqlx::query_as::<_, User>(
        "INSERT INTO users (username, password_hash)
         VALUES ($1, $2)
         RETURNING id, username, password_hash, created_at, locked_until, failed_attempts",
    )
    .bind(&req.username)
    .bind(&password_hash)
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(ref db_err) = e {
            if db_err.constraint() == Some("users_username_key") {
                return AppError::Conflict("username already taken".into());
            }
        }
        AppError::Internal(e.to_string())
    })?;

    // Create default board
    sqlx::query(
        "INSERT INTO boards (owner_id, name, data)
         VALUES ($1, 'default', '{}'::jsonb)
         ON CONFLICT DO NOTHING",
    )
    .bind(user.id)
    .execute(&state.db)
    .await?;

    // Generate JWT
    let token = generate_jwt(user.id, &state.jwt_secret)?;

    Ok(Json(AuthResponse {
        token,
        user_id: user.id,
        username: user.username,
    }))
}

// ── POST /api/auth/login ─────────────────────────────────────────────────────

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    let user = sqlx::query_as::<_, User>(
        "SELECT id, username, password_hash, created_at, locked_until, failed_attempts
         FROM users WHERE username = $1",
    )
    .bind(&req.username)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::Unauthorized("invalid username or password".into()))?;

    // Check if account is locked
    if let Some(locked_until) = user.locked_until {
        if locked_until > Utc::now() {
            let remaining = (locked_until - Utc::now()).num_minutes();
            return Err(AppError::Locked(format!(
                "account locked. Try again in {} minutes",
                remaining + 1
            )));
        }
    }

    // Verify password
    let valid = bcrypt::verify(&req.password, &user.password_hash)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    if !valid {
        // Increment failed attempts
        let new_attempts = user.failed_attempts + 1;
        if new_attempts >= 5 {
            // Lock for 15 minutes
            let lock_until = Utc::now() + chrono::Duration::minutes(15);
            sqlx::query("UPDATE users SET failed_attempts = $1, locked_until = $2 WHERE id = $3")
                .bind(new_attempts)
                .bind(lock_until)
                .bind(user.id)
                .execute(&state.db)
                .await?;

            return Err(AppError::Locked(
                "account locked due to too many failed attempts. Try again in 15 minutes".into(),
            ));
        } else {
            sqlx::query("UPDATE users SET failed_attempts = $1 WHERE id = $2")
                .bind(new_attempts)
                .bind(user.id)
                .execute(&state.db)
                .await?;
        }

        return Err(AppError::Unauthorized("invalid username or password".into()));
    }

    // Reset failed attempts on successful login
    sqlx::query("UPDATE users SET failed_attempts = 0, locked_until = NULL WHERE id = $1")
        .bind(user.id)
        .execute(&state.db)
        .await?;

    // Generate JWT
    let token = generate_jwt(user.id, &state.jwt_secret)?;

    Ok(Json(AuthResponse {
        token,
        user_id: user.id,
        username: user.username,
    }))
}

// ── JWT generation ───────────────────────────────────────────────────────────

fn generate_jwt(user_id: Uuid, secret: &str) -> Result<String, AppError> {
    let now = Utc::now().timestamp() as usize;
    let claims = Claims {
        sub: user_id.to_string(),
        exp: now + 86400, // 24 hours
        iat: now,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_ref()),
    )
    .map_err(|e| AppError::Internal(e.to_string()))
}

// ── Proof-of-Work ────────────────────────────────────────────────────────────

fn verify_pow(challenge: &str, nonce: u64, difficulty: u32) -> bool {
    let input = format!("{}{}", challenge, nonce);
    let hash = Sha256::digest(input.as_bytes());
    let hex = format!("{:x}", hash);
    let prefix = "0".repeat(difficulty as usize);
    hex.starts_with(&prefix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_pow_valid() {
        // For challenge "test" and difficulty 2, find a valid nonce
        let challenge = "test_12345";
        let mut nonce = 0u64;
        loop {
            if verify_pow(challenge, nonce, 2) {
                break;
            }
            nonce += 1;
        }
        assert!(verify_pow(challenge, nonce, 2));
    }

    #[test]
    fn test_verify_pow_invalid() {
        let challenge = "test_12345";
        // Most nonces should not be valid
        assert!(!verify_pow(challenge, 0, 8)); // very unlikely
    }
}
