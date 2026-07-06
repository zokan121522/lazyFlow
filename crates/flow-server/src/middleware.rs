use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};
use jsonwebtoken::{decode, DecodingKey, Validation};
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::Claims;
use crate::AppState;

// ── AuthenticatedUser extractor ──────────────────────────────────────────────
// Reads user_id from request extensions (set by auth_guard middleware)

#[derive(Debug, Clone, Copy)]
pub struct AuthenticatedUser(pub Uuid);

impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Uuid>()
            .copied()
            .map(AuthenticatedUser)
            .ok_or_else(|| AppError::Unauthorized("not authenticated".into()))
    }
}

// ── Token Bucket Rate Limiter ────────────────────────────────────────────────
// Full implementation ready for when rate limiting is enabled.
// Currently disabled via simplified rate_limit middleware below.

#[allow(dead_code)]
struct Bucket {
    tokens: f64,
    capacity: f64,
    refill_rate: f64,
    last_refill: Instant,
}

#[allow(dead_code)]
impl Bucket {
    fn new(capacity: f64, refill_rate: f64) -> Self {
        Self {
            tokens: capacity,
            capacity,
            refill_rate,
            last_refill: Instant::now(),
        }
    }

    fn try_consume(&mut self) -> bool {
        let elapsed = self.last_refill.elapsed().as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.capacity);
        self.last_refill = Instant::now();

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Mutex;

#[allow(dead_code)]
pub struct RateLimiter {
    buckets: Mutex<HashMap<SocketAddr, Bucket>>,
    last_cleanup: Mutex<Instant>,
    cleanup_interval: Duration,
}

#[allow(dead_code)]
impl RateLimiter {
    pub fn new() -> Self {
        Self {
            buckets: Mutex::new(HashMap::new()),
            last_cleanup: Mutex::new(Instant::now()),
            cleanup_interval: Duration::from_secs(3600),
        }
    }

    pub fn check(&self, addr: SocketAddr) -> bool {
        let mut buckets = self.buckets.lock().unwrap();
        let mut last = self.last_cleanup.lock().unwrap();

        if last.elapsed() >= self.cleanup_interval {
            buckets.retain(|_, b| b.tokens < b.capacity * 0.9);
            *last = Instant::now();
        }

        let bucket = buckets
            .entry(addr)
            .or_insert_with(|| Bucket::new(30.0, 0.5));
        bucket.try_consume()
    }
}

// ── Rate limit middleware ────────────────────────────────────────────────────
// Simplified: allows all. Full TokenBucket implementation above (RateLimiter struct)
// is ready to be wired in once load testing determines correct thresholds.

#[allow(dead_code)]
pub async fn rate_limit(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    // Rate limiting is enabled but uses a simplified approach:
    // In production, use a proper tower layer with the RateLimiter struct
    // For now, allow all requests (full implementation pending load testing)

    next.run(req).await
}

// ── Auth guard middleware ────────────────────────────────────────────────────
// Validates JWT on protected routes and injects user_id into request extensions

pub async fn auth_guard(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    mut req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let path = req.uri().path();

    // Skip auth for public endpoints
    if path == "/api/health"
        || path.starts_with("/api/auth/")
    {
        return next.run(req).await;
    }

    // Extract Authorization header
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string());

    match auth_header {
        Some(token) => {
            match decode::<Claims>(
                &token,
                &DecodingKey::from_secret(state.jwt_secret.as_ref()),
                &Validation::default(),
            ) {
                Ok(token_data) => {
                    if let Ok(user_id) = token_data.claims.sub.parse::<Uuid>() {
                        req.extensions_mut().insert(user_id);
                        next.run(req).await
                    } else {
                        AppError::Unauthorized("invalid user_id in token".into()).into_response()
                    }
                }
                Err(_) => AppError::Unauthorized("invalid or expired token".into()).into_response(),
            }
        }
        None => AppError::Unauthorized("missing authorization header".into()).into_response(),
    }
}
