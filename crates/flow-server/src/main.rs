mod auth;
mod db;
mod error;
mod handlers;
mod middleware;
mod models;

use axum::middleware as axum_mw;
use axum::routing::{get, post, put};
use axum::Router;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing_subscriber::EnvFilter;

pub struct AppState {
    pub db: sqlx::PgPool,
    pub jwt_secret: String,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("flow_server=info")),
        )
        .init();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://flow:flow@localhost:5432/flow".to_string());
    let jwt_secret = std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| "change-me-in-production".to_string());
    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse::<u16>()
        .expect("PORT must be a valid u16");

    let pool = db::init_pool(&database_url)
        .await
        .expect("Failed to connect to database");

    let state = Arc::new(AppState { db: pool, jwt_secret });

    let app = Router::new()
        // Health
        .route("/api/health", get(health))
        // Auth (public)
        .route("/api/auth/challenge", get(auth::challenge))
        .route("/api/auth/register", post(auth::register))
        .route("/api/auth/login", post(auth::login))
        // Board (protected by auth_guard)
        .route("/api/board", get(handlers::get_board).put(handlers::update_board))
        .route("/api/board/cards", post(handlers::create_card))
        .route(
            "/api/board/cards/{id}",
            put(handlers::update_card).delete(handlers::delete_card),
        )
        // Middleware stack: rate limit (outer) → auth guard (inner for protected routes)
        .route_layer(axum_mw::from_fn_with_state(
            state.clone(),
            middleware::auth_guard,
        ))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = format!("{}:{}", host, port);
    tracing::info!("flow-server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind address");
    axum::serve(listener, app)
        .await
        .expect("Server exited with error");
}

async fn health() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({"status": "ok"}))
}
