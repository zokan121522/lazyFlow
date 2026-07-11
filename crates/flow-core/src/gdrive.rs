// ── Google Drive sync client ─────────────────────────────────────────────
//
// Uses OAuth 2.0 Device Authorization Flow (RFC 8628) — ideal for terminal
// apps since no redirect URI is needed.
//
// # Setup (one-time)
//
// 1. Go to https://console.cloud.google.com → Create project → Enable
//    "Google Drive API"
// 2. Credentials → OAuth 2.0 Client ID → "Desktop application"
// 3. Copy the Client ID and either:
//    - Set `FLOW_GDRIVE_CLIENT_ID` env var, or
//    - Enter it in the TUI settings popup on first connect
// 4. Board is synced as `lazyflow-board.json` in your Google Drive root.
//
// ──────────────────────────────────────────────────────────────────────────

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

// ── OAuth / Drive API endpoints ─────────────────────────────────────────

const DEVICE_CODE_URL: &str = "https://oauth2.googleapis.com/device/code";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const DRIVE_FILES_URL: &str = "https://www.googleapis.com/drive/v3/files";
const DRIVE_UPLOAD_URL: &str = "https://www.googleapis.com/upload/drive/v3/files";
const OAUTH_SCOPE: &str = "https://www.googleapis.com/auth/drive.file";
const GDRIVE_BOARD_FILE: &str = "lazyflow-board.json";

// ── Default paths ───────────────────────────────────────────────────────

fn default_token_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".flow/gdrive_token.json")
}

fn default_config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".flow/gdrive_config.json")
}

fn default_client_id() -> String {
    std::env::var("FLOW_GDRIVE_CLIENT_ID").unwrap_or_default()
}

// ── Response types (API) ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    #[serde(rename = "verification_url")]
    pub verification_url: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub expires_in: u64,
    #[serde(default)]
    pub refresh_token: Option<String>,
    pub scope: String,
    pub token_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenErrorResponse {
    pub error: String,
    #[serde(default)]
    pub error_description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileListResponse {
    #[serde(default)]
    pub files: Vec<FileResource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileResource {
    pub id: String,
    pub name: String,
}

// ── Token persistence ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredToken {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: u64, // unix seconds
}

// ── Config persistence ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GDriveConfig {
    pub auto_sync: bool,
    #[serde(default)]
    pub client_id: String,
}

impl Default for GDriveConfig {
    fn default() -> Self {
        Self {
            auto_sync: true,
            client_id: String::new(),
        }
    }
}

// ── Error type ──────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum GDriveError {
    Transport(String),
    Api { status: u16, message: String },
    Auth(String),
    Config(String),
    Io(String),
}

impl std::fmt::Display for GDriveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GDriveError::Transport(msg) => write!(f, "connection: {msg}"),
            GDriveError::Api { status, message } => {
                write!(f, "API error (HTTP {status}): {message}")
            }
            GDriveError::Auth(msg) => write!(f, "auth: {msg}"),
            GDriveError::Config(msg) => write!(f, "config: {msg}"),
            GDriveError::Io(msg) => write!(f, "I/O: {msg}"),
        }
    }
}

impl std::error::Error for GDriveError {}

impl From<reqwest::Error> for GDriveError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_timeout() || e.is_connect() {
            GDriveError::Transport(format!("{e}"))
        } else {
            GDriveError::Transport(format!("{e}"))
        }
    }
}

impl From<std::io::Error> for GDriveError {
    fn from(e: std::io::Error) -> Self {
        GDriveError::Io(format!("{e}"))
    }
}

impl From<serde_json::Error> for GDriveError {
    fn from(e: serde_json::Error) -> Self {
        GDriveError::Config(format!("JSON error: {e}"))
    }
}

// ── Public status ───────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GDriveStatus {
    Disconnected,
    Authorizing {
        verification_url: String,
        user_code: String,
    },
    Connected {
        /// Email-like identifier from token info (may be empty).
        account: String,
    },
    Error(String),
}

// ── Poll result ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PollResult {
    /// Keep polling (user hasn't authorized yet).
    Waiting,
    /// Authorization complete — token acquired.
    Success,
    /// The device code expired (user took too long).
    Expired,
    /// User denied the authorization request.
    Denied,
    /// A non-fatal transport/API error occurred.
    TransientError(String),
}

// ── GDriveClient ────────────────────────────────────────────────────────

pub struct GDriveClient {
    client: Client,
    client_id: String,
    token: Option<StoredToken>,
    token_path: PathBuf,
    config_path: PathBuf,
    status: GDriveStatus,

    // Device auth polling state
    device_code: Option<String>,
    poll_interval: u64,
    last_poll: Instant,
    expires_at: Option<Instant>,

    // Cached file ID for the board file in Drive
    board_file_id: Option<String>,

    // Last sync time (human-readable, updated on success)
    last_sync: Option<String>,
}

impl GDriveClient {
    /// Create a new client. Attempts to load a stored token and config.
    pub fn new() -> Self {
        let token_path = default_token_path();
        let config_path = default_config_path();

        let stored = Self::load_token_from(&token_path);
        let config = Self::load_config_from(&config_path);

        let client_id = if !config.client_id.is_empty() {
            config.client_id.clone()
        } else {
            default_client_id()
        };

        let status = if stored.is_some() {
            GDriveStatus::Connected {
                account: String::new(),
            }
        } else {
            GDriveStatus::Disconnected
        };

        Self {
            client: Client::new(),
            client_id,
            token: stored,
            token_path,
            config_path,
            status,
            device_code: None,
            poll_interval: 5,
            last_poll: Instant::now(),
            expires_at: None,
            board_file_id: None,
            last_sync: None,
        }
    }

    // ── Status ──────────────────────────────────────────────────────────

    /// Returns the current GDrive status for the TUI indicator.
    pub fn status(&self) -> &GDriveStatus {
        &self.status
    }

    /// Returns the human-readable last sync time.
    pub fn last_sync(&self) -> Option<&str> {
        self.last_sync.as_deref()
    }

    /// Whether the client has a valid (or refreshable) token.
    pub fn is_authenticated(&self) -> bool {
        self.token.is_some()
    }

    /// Whether the token is expired (based on stored `expires_at`).
    pub fn is_token_expired(&self) -> bool {
        self.token
            .as_ref()
            .map(|t| {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                now >= t.expires_at
            })
            .unwrap_or(false)
    }

    /// Whether the token has a refresh token.
    pub fn has_refresh_token(&self) -> bool {
        self.token
            .as_ref()
            .and_then(|t| t.refresh_token.as_deref())
            .is_some()
    }

    /// Check if the client has a configured client_id.
    pub fn has_client_id(&self) -> bool {
        !self.client_id.is_empty()
    }

    /// Override the client ID.
    pub fn set_client_id(&mut self, client_id: String) {
        self.client_id = client_id;
    }

    /// Get current client_id (masked for display).
    pub fn client_id_label(&self) -> &str {
        if self.client_id.is_empty() {
            "(not set)"
        } else if self.client_id.len() > 20 {
            // Show first 8 + last 8 chars
            "…configured…"
        } else {
            &self.client_id
        }
    }

    /// Get the GDriveConfig (for TUI to read/write).
    pub fn get_config(&self) -> GDriveConfig {
        let path = self.config_path.clone();
        Self::load_config_from(&path)
    }

    /// Save GDriveConfig and update local settings.
    pub fn save_config(&mut self, config: &GDriveConfig) -> Result<(), GDriveError> {
        // Update client_id if config provides one
        if !config.client_id.is_empty() {
            self.client_id = config.client_id.clone();
        }
        Self::save_config_to(&self.config_path, config)
    }

    // ── OAuth Device Flow ───────────────────────────────────────────────

    /// Step 1: Request a device code from Google.
    ///
    /// Call this when the user clicks "Connect". The returned
    /// `DeviceCodeResponse` contains the URL + code to display.
    pub fn start_device_auth(&mut self) -> Result<DeviceCodeResponse, GDriveError> {
        if self.client_id.is_empty() {
            return Err(GDriveError::Config(
                "No Google OAuth Client ID configured. \
                 Set FLOW_GDRIVE_CLIENT_ID env var or enter it in settings."
                    .to_string(),
            ));
        }

        let params = serde_json::json!({
            "client_id": self.client_id,
            "scope": OAUTH_SCOPE,
        });

        let resp = self
            .client
            .post(DEVICE_CODE_URL)
            .json(&params)
            .send()?;

        let status = resp.status();
        let body = resp.text()?;

        if !status.is_success() {
            return Err(GDriveError::Api {
                status: status.as_u16(),
                message: body,
            });
        }

        let device_resp: DeviceCodeResponse = serde_json::from_str(&body)?;

        // Store polling state
        self.device_code = Some(device_resp.device_code.clone());
        self.poll_interval = device_resp.interval.max(5); // min 5s
        self.last_poll = Instant::now();
        self.expires_at = Some(Instant::now() + Duration::from_secs(device_resp.expires_in));

        self.status = GDriveStatus::Authorizing {
            verification_url: device_resp.verification_url.clone(),
            user_code: device_resp.user_code.clone(),
        };

        Ok(device_resp)
    }

    /// Step 2 (repeated): Poll Google for the token.
    ///
    /// Call this periodically in the TUI event loop. Returns `PollResult`
    /// indicating whether auth is complete, still waiting, or failed.
    ///
    /// Only actually makes the HTTP request if enough time has passed
    /// since the last poll (`interval` from the device code response).
    pub fn try_poll_token(&mut self) -> PollResult {
        let device_code = match &self.device_code {
            Some(code) => code.clone(),
            None => return PollResult::Waiting,
        };

        // Check expiry
        if let Some(expires) = self.expires_at {
            if Instant::now() >= expires {
                self.device_code = None;
                self.status = GDriveStatus::Disconnected;
                return PollResult::Expired;
            }
        }

        // Rate-limit: only poll every `poll_interval` seconds
        if Instant::now() < self.last_poll + Duration::from_secs(self.poll_interval) {
            return PollResult::Waiting;
        }
        self.last_poll = Instant::now();

        let params = serde_json::json!({
            "client_id": self.client_id,
            "device_code": device_code,
            "grant_type": "urn:ietf:params:oauth:grant-type:device_code",
        });

        let resp = match self.client.post(TOKEN_URL).json(&params).send() {
            Ok(r) => r,
            Err(e) => return PollResult::TransientError(format!("{e}")),
        };

        let status = resp.status();
        let body = match resp.text() {
            Ok(b) => b,
            Err(e) => return PollResult::TransientError(format!("{e}")),
        };

        if status.is_success() {
            // Token acquired!
            match serde_json::from_str::<TokenResponse>(&body) {
                Ok(token_resp) => {
                    let expires_at = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs()
                        + token_resp.expires_in;

                    let stored = StoredToken {
                        access_token: token_resp.access_token,
                        refresh_token: token_resp.refresh_token,
                        expires_at,
                    };

                    self.token = Some(stored);
                    self.device_code = None;
                    self.expires_at = None;

                    if let Err(e) = self.save_token() {
                        eprintln!("[gdrive] failed to save token: {e}");
                    }

                    self.status = GDriveStatus::Connected {
                        account: String::new(),
                    };

                    // Try to fetch account email in background
                    self.fetch_account_email();

                    PollResult::Success
                }
                Err(e) => PollResult::TransientError(format!("parse error: {e}")),
            }
        } else if status.as_u16() == 400 {
            // Check for specific OAuth errors
            if let Ok(err_resp) = serde_json::from_str::<TokenErrorResponse>(&body) {
                match err_resp.error.as_str() {
                    "authorization_pending" => PollResult::Waiting,
                    "slow_down" => {
                        // Increase polling interval
                        self.poll_interval = (self.poll_interval + 5).min(60);
                        PollResult::Waiting
                    }
                    "access_denied" => {
                        self.device_code = None;
                        self.status = GDriveStatus::Disconnected;
                        PollResult::Denied
                    }
                    "expired_token" => {
                        self.device_code = None;
                        self.status = GDriveStatus::Disconnected;
                        PollResult::Expired
                    }
                    _ => PollResult::TransientError(format!(
                        "{}: {}",
                        err_resp.error,
                        err_resp.error_description.unwrap_or_default()
                    )),
                }
            } else {
                PollResult::TransientError(body)
            }
        } else {
            PollResult::TransientError(format!("HTTP {}: {}", status.as_u16(), body))
        }
    }

    /// Cancel an in-progress device authorization.
    pub fn cancel_auth(&mut self) {
        self.device_code = None;
        self.expires_at = None;
        self.status = if self.token.is_some() {
            GDriveStatus::Connected {
                account: String::new(),
            }
        } else {
            GDriveStatus::Disconnected
        };
    }

    /// Cancel device auth and disconnect.
    pub fn disconnect(&mut self) -> Result<(), GDriveError> {
        self.token = None;
        self.device_code = None;
        self.expires_at = None;
        self.board_file_id = None;
        self.last_sync = None;
        self.status = GDriveStatus::Disconnected;

        // Remove token file
        if self.token_path.exists() {
            fs::remove_file(&self.token_path)?;
        }
        Ok(())
    }

    // ── Token refresh ───────────────────────────────────────────────────

    /// Try to refresh the access token using the stored refresh token.
    /// Returns true if the token is now valid.
    pub fn try_refresh_token(&mut self) -> bool {
        let refresh_token = match self
            .token
            .as_ref()
            .and_then(|t| t.refresh_token.as_deref())
        {
            Some(rt) => rt.to_string(),
            None => return false,
        };

        let params = serde_json::json!({
            "client_id": self.client_id,
            "refresh_token": refresh_token,
            "grant_type": "refresh_token",
        });

        let resp = match self.client.post(TOKEN_URL).json(&params).send() {
            Ok(r) => r,
            Err(_) => return false,
        };

        let status = resp.status();
        let body = match resp.text() {
            Ok(b) => b,
            Err(_) => return false,
        };

        if !status.is_success() {
            return false;
        }

        match serde_json::from_str::<TokenResponse>(&body) {
            Ok(token_resp) => {
                let expires_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
                    + token_resp.expires_in;

                self.token = Some(StoredToken {
                    access_token: token_resp.access_token,
                    refresh_token: token_resp.refresh_token.or(Some(refresh_token)),
                    expires_at,
                });

                self.status = GDriveStatus::Connected {
                    account: String::new(),
                };

                let _ = self.save_token();
                true
            }
            Err(_) => false,
        }
    }

    // ── Board sync ──────────────────────────────────────────────────────

    /// Upload the board JSON to Google Drive.
    ///
    /// Creates `lazyflow-board.json` if it doesn't exist, or updates it.
    pub fn upload_board(&mut self, data: &serde_json::Value) -> Result<(), GDriveError> {
        self.ensure_token_valid()?;

        let file_id = self.resolve_board_file_id()?;
        let body = serde_json::to_vec(data)?;

        let url = if let Some(ref fid) = file_id {
            format!("{DRIVE_UPLOAD_URL}/{fid}?uploadType=media")
        } else {
            // Create with metadata + content
            let metadata = serde_json::json!({
                "name": GDRIVE_BOARD_FILE,
                "mimeType": "application/json",
            });

            // First create metadata
            let meta_resp = self
                .client
                .post(DRIVE_FILES_URL)
                .header("Authorization", format!("Bearer {}", self.access_token()?))
                .json(&metadata)
                .send()?;

            let meta_status = meta_resp.status();
            let meta_body: serde_json::Value = meta_resp.json()?;

            if !meta_status.is_success() {
                return Err(GDriveError::Api {
                    status: meta_status.as_u16(),
                    message: meta_body.to_string(),
                });
            }

            let new_id = meta_body["id"]
                .as_str()
                .ok_or_else(|| GDriveError::Api {
                    status: 200,
                    message: "missing id in create response".to_string(),
                })?
                .to_string();

            self.board_file_id = Some(new_id.clone());
            format!("{DRIVE_UPLOAD_URL}/{new_id}?uploadType=media")
        };

        let upload_resp = self
            .client
            .patch(&url)
            .header("Authorization", format!("Bearer {}", self.access_token()?))
            .header("Content-Type", "application/json")
            .body(body)
            .send()?;

        let upload_status = upload_resp.status();
        if !upload_status.is_success() {
            let body_text = upload_resp.text().unwrap_or_default();
            return Err(GDriveError::Api {
                status: upload_status.as_u16(),
                message: body_text,
            });
        }

        // Update last sync time
        let now = chrono_now();
        self.last_sync = Some(now);

        Ok(())
    }

    /// Download the board JSON from Google Drive.
    ///
    /// Returns `None` if the file doesn't exist yet (first time).
    pub fn download_board(&mut self) -> Result<Option<serde_json::Value>, GDriveError> {
        self.ensure_token_valid()?;

        let file_id = match self.resolve_board_file_id()? {
            Some(id) => id,
            None => return Ok(None),
        };

        let url = format!("{DRIVE_FILES_URL}/{file_id}?alt=media");

        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.access_token()?))
            .send()?;

        let status = resp.status();
        if status.is_success() {
            let data: serde_json::Value = resp.json()?;
            let now = chrono_now();
            self.last_sync = Some(format!("{} (downloaded)", now));
            Ok(Some(data))
        } else if status.as_u16() == 404 {
            Ok(None)
        } else {
            let body = resp.text().unwrap_or_default();
            Err(GDriveError::Api {
                status: status.as_u16(),
                message: body,
            })
        }
    }

    // ── Helpers ─────────────────────────────────────────────────────────

    /// Get the access token string, or error.
    fn access_token(&self) -> Result<&str, GDriveError> {
        self.token
            .as_ref()
            .map(|t| t.access_token.as_str())
            .ok_or_else(|| GDriveError::Auth("not authenticated".to_string()))
    }

    /// Ensure the token is valid — try refresh if expired.
    fn ensure_token_valid(&mut self) -> Result<(), GDriveError> {
        if !self.is_authenticated() {
            return Err(GDriveError::Auth("not authenticated".to_string()));
        }

        if self.is_token_expired() {
            if self.has_refresh_token() {
                if !self.try_refresh_token() {
                    self.status = GDriveStatus::Error("token expired, reconnect".to_string());
                    return Err(GDriveError::Auth(
                        "token expired and refresh failed. Disconnect and reconnect."
                            .to_string(),
                    ));
                }
            } else {
                self.status = GDriveStatus::Error("token expired, reconnect".to_string());
                return Err(GDriveError::Auth(
                    "token expired and no refresh token. Disconnect and reconnect."
                        .to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Find the board file in Drive, or return `None` if it doesn't exist.
    /// Caches the file ID after first lookup.
    fn resolve_board_file_id(&mut self) -> Result<Option<String>, GDriveError> {
        if let Some(ref id) = self.board_file_id {
            return Ok(Some(id.clone()));
        }

        let query = format!("name='{}' and trashed=false", GDRIVE_BOARD_FILE);
        let url = format!("{DRIVE_FILES_URL}?q={}", urlencoding(&query));

        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.access_token()?))
            .send()?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().unwrap_or_default();
            return Err(GDriveError::Api {
                status: status.as_u16(),
                message: body,
            });
        }

        let list: FileListResponse = resp.json()?;
        if let Some(file) = list.files.into_iter().next() {
            self.board_file_id = Some(file.id.clone());
            Ok(Some(file.id))
        } else {
            Ok(None)
        }
    }

    /// Fetch the account email from tokeninfo (best-effort).
    fn fetch_account_email(&mut self) {
        let token = match self.token.as_ref().map(|t| t.access_token.as_str()) {
            Some(t) => t,
            None => return,
        };

        let url = format!("https://www.googleapis.com/oauth2/v1/tokeninfo?access_token={token}");

        if let Ok(resp) = self.client.get(&url).send() {
            if let Ok(body) = resp.text() {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                    if let Some(email) = json["email"].as_str() {
                        self.status = GDriveStatus::Connected {
                            account: email.to_string(),
                        };
                    }
                }
            }
        }
    }

    // ── Token persistence ───────────────────────────────────────────────

    fn save_token(&self) -> Result<(), GDriveError> {
        let Some(ref token) = self.token else {
            return Ok(());
        };

        let json = serde_json::to_string(token)?;

        if let Some(parent) = self.token_path.parent() {
            fs::create_dir_all(parent)?;
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let f = fs::File::create(&self.token_path)?;
            f.set_permissions(fs::Permissions::from_mode(0o600))?;
            let mut f = f;
            f.write_all(json.as_bytes())?;
        }
        #[cfg(not(unix))]
        {
            fs::write(&self.token_path, json)?;
        }

        Ok(())
    }

    fn load_token_from(path: &PathBuf) -> Option<StoredToken> {
        if !path.exists() {
            return None;
        }
        let content = fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    fn load_config_from(path: &PathBuf) -> GDriveConfig {
        if !path.exists() {
            return GDriveConfig::default();
        }
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save_config_to(path: &PathBuf, config: &GDriveConfig) -> Result<(), GDriveError> {
        let json = serde_json::to_string_pretty(config)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, json)?;
        Ok(())
    }
}

impl Default for GDriveClient {
    fn default() -> Self {
        Self::new()
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn urlencoding(s: &str) -> String {
    // Minimal URL encoding for the Drive API query parameter
    s.replace('\'', "%27")
        .replace('=', "%3D")
        .replace('&', "%26")
        .replace(' ', "%20")
}

fn chrono_now() -> String {
    // Simple ISO-like timestamp without chrono dependency
    use std::time::{SystemTime, UNIX_EPOCH};
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();

    // Break into components (rough)
    let days = secs / 86400;
    let time_secs = secs % 86400;
    let hours = time_secs / 3600;
    let minutes = (time_secs % 3600) / 60;
    let seconds = time_secs % 60;

    // Days since epoch → approximate date (not perfect but readable)
    let year = 1970 + (days as f64 / 365.25) as u64;
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        year, 1, 1, hours, minutes, seconds
    )
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_client_is_disconnected() {
        let client = GDriveClient::new();
        assert_eq!(client.status(), &GDriveStatus::Disconnected);
        assert!(!client.is_authenticated());
    }

    #[test]
    fn client_id_from_env() {
        unsafe { std::env::set_var("FLOW_GDRIVE_CLIENT_ID", "test-id-123"); }
        let client = GDriveClient::new();
        assert!(client.has_client_id());
        unsafe { std::env::remove_var("FLOW_GDRIVE_CLIENT_ID"); }
    }

    #[test]
    fn default_config_is_auto_sync() {
        let config = GDriveConfig::default();
        assert!(config.auto_sync);
    }

    #[test]
    fn status_display() {
        assert_eq!(format!("{:?}", GDriveStatus::Disconnected), "Disconnected");
        assert_eq!(
            format!("{:?}", GDriveStatus::Connected { account: "a@b.com".into() }),
            "Connected { account: \"a@b.com\" }"
        );
    }

    #[test]
    fn token_expired_detection() {
        let mut client = GDriveClient::new();
        let past = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            - 3600; // 1 hour ago

        client.token = Some(StoredToken {
            access_token: "old".into(),
            refresh_token: Some("rt".into()),
            expires_at: past,
        });

        assert!(client.is_token_expired());
        assert!(client.has_refresh_token());
    }

    #[test]
    fn url_encoding_works() {
        assert_eq!(urlencoding("name='test.json'"), "name%3D%27test.json%27");
        assert_eq!(urlencoding("a b"), "a%20b");
    }

    #[test]
    fn disconnect_clears_state() {
        let mut client = GDriveClient::new();
        client.token = Some(StoredToken {
            access_token: "x".into(),
            refresh_token: None,
            expires_at: 9999999999,
        });
        client.status = GDriveStatus::Connected {
            account: "u@u.com".into(),
        };
        client.board_file_id = Some("file123".into());

        let _ = client.disconnect();
        assert_eq!(client.status(), &GDriveStatus::Disconnected);
        assert!(!client.is_authenticated());
        assert!(client.board_file_id.is_none());
    }

    #[test]
    fn cancel_auth_works() {
        let mut client = GDriveClient::new();
        client.device_code = Some("dc".into());
        client.status = GDriveStatus::Authorizing {
            verification_url: "url".into(),
            user_code: "code".into(),
        };

        client.cancel_auth();
        assert_eq!(client.status(), &GDriveStatus::Disconnected);
        assert!(client.device_code.is_none());
    }
}
