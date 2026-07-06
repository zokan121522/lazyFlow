use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// ── Auth models ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
    pub pow_challenge: String,
    pub pow_nonce: u64,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user_id: Uuid,
    pub username: String,
}

#[derive(Debug, Serialize)]
pub struct ChallengeResponse {
    pub challenge: String,
    pub difficulty: u32,
}

// ── JWT Claims ───────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // user_id
    pub exp: usize,
    pub iat: usize,
}

// ── DB models ────────────────────────────────────────────────────────────────

#[derive(Debug, FromRow)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub password_hash: String,
    #[allow(dead_code)]
    pub created_at: Option<DateTime<Utc>>,
    pub locked_until: Option<DateTime<Utc>>,
    pub failed_attempts: i32,
}

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct Board {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub name: String,
    pub data: serde_json::Value,
    pub updated_at: Option<DateTime<Utc>>,
}

// ── API models ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateCardRequest {
    pub column: String,
    pub title: String,
    #[serde(default)]
    pub body: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCardRequest {
    pub column: Option<String>,
    pub title: Option<String>,
    pub body: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CardResponse {
    pub id: String,
    pub column: String,
    pub title: String,
    pub body: String,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}
