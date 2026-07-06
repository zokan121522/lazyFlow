use std::fmt;
use std::fs;
use std::path::PathBuf;

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

// ── Board path (preserved from original) ──────────────────────────────────────

/// Get the board path from env or default.
pub fn board_path() -> PathBuf {
    if let Ok(p) = std::env::var("FLOW_BOARD_PATH") {
        PathBuf::from(p)
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".flow/boards/principal")
    } else {
        PathBuf::from(".flow/boards/principal")
    }
}

fn default_token_path() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".flow/auth_token.json")
    } else {
        PathBuf::from(".flow/auth_token.json")
    }
}

// ── Error type ────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum SyncError {
    /// The server is unreachable or returned a transport error.
    Transport(String),
    /// The server returned a non-success HTTP status with an error body.
    Api {
        status: u16,
        message: String,
    },
    /// No auth token available — need to log in first.
    NotAuthenticated,
    /// I/O error reading/writing the token file or other local data.
    Io(String),
}

impl fmt::Display for SyncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SyncError::Transport(msg) => write!(f, "connection error: {msg}"),
            SyncError::Api { status, message } => {
                write!(f, "server error (HTTP {status}): {message}")
            }
            SyncError::NotAuthenticated => {
                write!(f, "not authenticated — please log in first")
            }
            SyncError::Io(msg) => write!(f, "file I/O error: {msg}"),
        }
    }
}

impl std::error::Error for SyncError {}

impl From<reqwest::Error> for SyncError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_timeout() || e.is_connect() {
            SyncError::Transport(format!("{e}"))
        } else {
            SyncError::Transport(format!("{e}"))
        }
    }
}

impl From<std::io::Error> for SyncError {
    fn from(e: std::io::Error) -> Self {
        SyncError::Io(format!("{e}"))
    }
}

impl From<serde_json::Error> for SyncError {
    fn from(e: serde_json::Error) -> Self {
        SyncError::Io(format!("JSON serialization error: {e}"))
    }
}

// ── API response models (client-side) ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChallengeResponse {
    pub challenge: String,
    pub difficulty: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    pub token: String,
    pub user_id: String,
    pub username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardResponse {
    pub id: String,
    pub column: String,
    pub title: String,
    pub body: String,
}

// ── Token persistence ─────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
struct StoredToken {
    token: String,
    server_url: String,
    username: String,
}

// ── SyncClient ────────────────────────────────────────────────────────────────

/// HTTP client for the flow-server cloud backend.
///
/// Handles authentication (login/register), board CRUD, and token persistence.
/// All methods use `reqwest::blocking` — suitable for CLI and TUI contexts.
///
/// # Environment variables
///
/// * `FLOW_SERVER_URL` — base URL of the flow-server (default: `http://localhost:3000`)
pub struct SyncClient {
    client: Client,
    base_url: String,
    token: Option<String>,
    username: Option<String>,
    token_path: PathBuf,
}

impl SyncClient {
    /// Create a new unauthenticated client.
    ///
    /// Reads `FLOW_SERVER_URL` from the environment, defaulting to
    /// `http://localhost:3000`. Also attempts to load a persisted token
    /// from `~/.flow/auth_token.json`.
    pub fn new() -> Self {
        let base_url = std::env::var("FLOW_SERVER_URL")
            .unwrap_or_else(|_| "http://localhost:3000".to_string());
        let base_url = base_url.trim_end_matches('/').to_string();

        let token_path = default_token_path();
        let (token, username) = Self::load_token_from(&token_path);

        Self {
            client: Client::new(),
            base_url,
            token,
            username,
            token_path,
        }
    }

    /// Create a client with an explicit token (bypasses token file).
    pub fn with_token(token: &str, username: &str, server_url: &str) -> Self {
        let base_url = server_url.trim_end_matches('/').to_string();
        let token_path = default_token_path();

        Self {
            client: Client::new(),
            base_url,
            token: Some(token.to_owned()),
            username: Some(username.to_owned()),
            token_path,
        }
    }

    // ── Auth helpers ──────────────────────────────────────────────────────

    /// Returns `true` if a JWT token is available (may be expired — server will reject).
    pub fn is_authenticated(&self) -> bool {
        self.token.is_some()
    }

    /// Returns the authenticated username, if available.
    pub fn username(&self) -> Option<&str> {
        self.username.as_deref()
    }

    /// Returns the current server URL.
    pub fn server_url(&self) -> &str {
        &self.base_url
    }

    /// Clear the stored token (logout).
    pub fn logout(&mut self) -> Result<(), SyncError> {
        self.token = None;
        self.username = None;
        if self.token_path.exists() {
            fs::remove_file(&self.token_path)?;
        }
        Ok(())
    }

    // ── Health ────────────────────────────────────────────────────────────

    /// Check if the server is reachable.
    pub fn health(&self) -> Result<String, SyncError> {
        let resp = self
            .client
            .get(format!("{}/api/health", self.base_url))
            .send()?;

        let status = resp.status();
        let body = resp.text()?;

        if status.is_success() {
            Ok(body.trim().to_string())
        } else {
            Err(SyncError::Api {
                status: status.as_u16(),
                message: body,
            })
        }
    }

    // ── Auth endpoints ────────────────────────────────────────────────────

    /// Get a proof-of-work challenge for registration.
    pub fn get_challenge(&self) -> Result<ChallengeResponse, SyncError> {
        let resp = self
            .client
            .get(format!("{}/api/auth/challenge", self.base_url))
            .send()?;

        let status = resp.status();
        if status.is_success() {
            Ok(resp.json::<ChallengeResponse>()?)
        } else {
            let body = resp.text()?;
            Err(SyncError::Api {
                status: status.as_u16(),
                message: body,
            })
        }
    }

    /// Register a new account.
    ///
    /// You must first obtain a challenge via [`get_challenge`], compute the
    /// proof-of-work nonce, then call this method.
    pub fn register(
        &mut self,
        username: &str,
        password: &str,
        pow_challenge: &str,
        pow_nonce: u64,
    ) -> Result<AuthResponse, SyncError> {
        let body = serde_json::json!({
            "username": username,
            "password": password,
            "pow_challenge": pow_challenge,
            "pow_nonce": pow_nonce,
        });

        let resp = self
            .client
            .post(format!("{}/api/auth/register", self.base_url))
            .json(&body)
            .send()?;

        let status = resp.status();
        if status.is_success() {
            let auth: AuthResponse = resp.json()?;
            self.token = Some(auth.token.clone());
            self.username = Some(auth.username.clone());
            self.save_token()?;
            Ok(auth)
        } else {
            let body_text = resp.text()?;
            Err(SyncError::Api {
                status: status.as_u16(),
                message: body_text,
            })
        }
    }

    /// Log in with username and password.
    ///
    /// On success the JWT token is stored in memory and persisted to disk.
    pub fn login(&mut self, username: &str, password: &str) -> Result<AuthResponse, SyncError> {
        let body = serde_json::json!({
            "username": username,
            "password": password,
        });

        let resp = self
            .client
            .post(format!("{}/api/auth/login", self.base_url))
            .json(&body)
            .send()?;

        let status = resp.status();
        if status.is_success() {
            let auth: AuthResponse = resp.json()?;
            self.token = Some(auth.token.clone());
            self.username = Some(auth.username.clone());
            self.save_token()?;
            Ok(auth)
        } else {
            let body_text = resp.text()?;
            Err(SyncError::Api {
                status: status.as_u16(),
                message: body_text,
            })
        }
    }

    // ── Board endpoints ───────────────────────────────────────────────────

    /// Fetch the full board from the server.
    ///
    /// Returns the raw JSON value (the board `data` column stored as JSONB
    /// on the server). Callers deserialize into their own `Board` type.
    ///
    /// Returns `None` if the board doesn't exist yet (first-time user).
    pub fn get_board(&self) -> Result<Option<serde_json::Value>, SyncError> {
        let token = self.token.as_deref().ok_or(SyncError::NotAuthenticated)?;

        let resp = self
            .client
            .get(format!("{}/api/board", self.base_url))
            .header("Authorization", format!("Bearer {token}"))
            .send()?;

        let status = resp.status();
        match status.as_u16() {
            200 => {
                // Server returns the full Board row; extract the `data` field
                let json: serde_json::Value = resp.json()?;
                Ok(json.get("data").cloned())
            }
            404 => Ok(None), // No board yet — first use
            _ => {
                let body = resp.text()?;
                Err(SyncError::Api {
                    status: status.as_u16(),
                    message: body,
                })
            }
        }
    }

    /// Save (overwrite) the full board on the server.
    ///
    /// `data` should be a JSON object representing the board columns → cards.
    pub fn put_board(&self, data: &serde_json::Value) -> Result<serde_json::Value, SyncError> {
        let token = self.token.as_deref().ok_or(SyncError::NotAuthenticated)?;

        let resp = self
            .client
            .put(format!("{}/api/board", self.base_url))
            .header("Authorization", format!("Bearer {token}"))
            .json(data)
            .send()?;

        let status = resp.status();
        if status.is_success() {
            let json: serde_json::Value = resp.json()?;
            Ok(json)
        } else {
            let body = resp.text()?;
            Err(SyncError::Api {
                status: status.as_u16(),
                message: body,
            })
        }
    }

    // ── Card endpoints ────────────────────────────────────────────────────

    /// Create a new card on the server.
    pub fn create_card(
        &self,
        column: &str,
        title: &str,
        body: &str,
    ) -> Result<CardResponse, SyncError> {
        let token = self.token.as_deref().ok_or(SyncError::NotAuthenticated)?;

        let req_body = serde_json::json!({
            "column": column,
            "title": title,
            "body": body,
        });

        let resp = self
            .client
            .post(format!("{}/api/board/cards", self.base_url))
            .header("Authorization", format!("Bearer {token}"))
            .json(&req_body)
            .send()?;

        let status = resp.status();
        if status.is_success() || status.as_u16() == 201 {
            Ok(resp.json::<CardResponse>()?)
        } else {
            let body_text = resp.text()?;
            Err(SyncError::Api {
                status: status.as_u16(),
                message: body_text,
            })
        }
    }

    /// Update an existing card on the server.
    ///
    /// Only the fields that are `Some` will be updated on the server side.
    pub fn update_card(
        &self,
        card_id: &str,
        column: Option<&str>,
        title: Option<&str>,
        body: Option<&str>,
    ) -> Result<CardResponse, SyncError> {
        let token = self.token.as_deref().ok_or(SyncError::NotAuthenticated)?;

        let mut req_body = serde_json::Map::new();
        if let Some(c) = column {
            req_body.insert("column".to_string(), serde_json::Value::String(c.to_owned()));
        }
        if let Some(t) = title {
            req_body.insert("title".to_string(), serde_json::Value::String(t.to_owned()));
        }
        if let Some(b) = body {
            req_body.insert("body".to_string(), serde_json::Value::String(b.to_owned()));
        }

        let resp = self
            .client
            .put(format!("{}/api/board/cards/{card_id}", self.base_url))
            .header("Authorization", format!("Bearer {token}"))
            .json(&req_body)
            .send()?;

        let status = resp.status();
        if status.is_success() {
            Ok(resp.json::<CardResponse>()?)
        } else {
            let body_text = resp.text()?;
            Err(SyncError::Api {
                status: status.as_u16(),
                message: body_text,
            })
        }
    }

    /// Delete a card from the server.
    pub fn delete_card(&self, card_id: &str) -> Result<(), SyncError> {
        let token = self.token.as_deref().ok_or(SyncError::NotAuthenticated)?;

        let resp = self
            .client
            .delete(format!("{}/api/board/cards/{card_id}", self.base_url))
            .header("Authorization", format!("Bearer {token}"))
            .send()?;

        let status = resp.status();
        if status.is_success() || status.as_u16() == 204 {
            Ok(())
        } else {
            let body = resp.text()?;
            Err(SyncError::Api {
                status: status.as_u16(),
                message: body,
            })
        }
    }

    // ── Token persistence ─────────────────────────────────────────────────

    fn save_token(&self) -> Result<(), SyncError> {
        let Some(token) = &self.token else {
            return Ok(()); // Nothing to save
        };
        let Some(username) = &self.username else {
            return Ok(());
        };

        let stored = StoredToken {
            token: token.clone(),
            server_url: self.base_url.clone(),
            username: username.clone(),
        };

        // Ensure the parent directory exists
        if let Some(parent) = self.token_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string(&stored)?;
        // Use restrictive permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let f = fs::File::create(&self.token_path)?;
            f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
            use std::io::Write;
            let mut f = f;
            f.write_all(json.as_bytes())?;
        }
        #[cfg(not(unix))]
        {
            fs::write(&self.token_path, json)?;
        }

        Ok(())
    }

    fn load_token_from(path: &PathBuf) -> (Option<String>, Option<String>) {
        if !path.exists() {
            return (None, None);
        }
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return (None, None),
        };
        let stored: StoredToken = match serde_json::from_str(&content) {
            Ok(s) => s,
            Err(_) => return (None, None),
        };
        (Some(stored.token), Some(stored.username))
    }

    /// Clear the persisted token (e.g. if server rejects it as expired).
    pub fn clear_token(&mut self) {
        self.token = None;
        self.username = None;
        let _ = fs::remove_file(&self.token_path);
    }
}

impl Default for SyncClient {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn board_path_uses_env() {
        std::env::set_var("FLOW_BOARD_PATH", "/tmp/test-flow-board");
        let p = board_path();
        assert_eq!(p, PathBuf::from("/tmp/test-flow-board"));
        std::env::remove_var("FLOW_BOARD_PATH");
    }

    #[test]
    fn sync_client_default_url() {
        let client = SyncClient::new();
        assert_eq!(client.base_url, "http://localhost:3000");
    }

    #[test]
    fn sync_client_env_url() {
        std::env::set_var("FLOW_SERVER_URL", "https://flow.example.com:8080");
        let client = SyncClient::new();
        assert_eq!(client.base_url, "https://flow.example.com:8080");
        std::env::remove_var("FLOW_SERVER_URL");
    }

    #[test]
    fn sync_client_trailing_slash() {
        std::env::set_var("FLOW_SERVER_URL", "http://localhost:4000/");
        let client = SyncClient::new();
        assert_eq!(client.base_url, "http://localhost:4000");
        std::env::remove_var("FLOW_SERVER_URL");
    }

    #[test]
    fn is_authenticated_false_by_default() {
        let client = SyncClient::new();
        assert!(!client.is_authenticated());
    }

    #[test]
    fn with_token_sets_auth() {
        let client = SyncClient::with_token("my.jwt.token", "testuser", "http://localhost:3000");
        assert!(client.is_authenticated());
        assert_eq!(client.username(), Some("testuser"));
    }

    #[test]
    fn logout_clears_token() {
        let mut client =
            SyncClient::with_token("token", "user", "http://localhost:3000");
        assert!(client.is_authenticated());
        // We don't have a token file in tests, but logout should still clear memory
        let _ = client.logout();
        assert!(!client.is_authenticated());
        assert_eq!(client.username(), None);
    }
}
