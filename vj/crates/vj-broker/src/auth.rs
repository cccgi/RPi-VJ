use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use uuid::Uuid;

pub const DEFAULT_PORTAL_AUTH_PATH: &str = "/var/lib/rpi-vj/portal-auth.json";
pub const MAX_AUTH_ATTEMPTS: usize = 5;
pub const AUTH_RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);
pub const TOKEN_EXPIRY_SECS: u64 = 86400; // 24 hours

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Invalid setup code or credentials")]
    InvalidCredentials,

    #[error("Too many failed attempts. Rate limit active. Please wait {seconds_remaining} seconds.")]
    RateLimited { seconds_remaining: u64 },

    #[error("Invalid or expired session token")]
    Unauthorized,
}

pub type Result<T> = std::result::Result<T, AuthError>;

fn get_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Constant time string comparison to prevent timing attacks
pub fn constant_time_eq(a: &str, b: &str) -> bool {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    if a_bytes.len() != b_bytes.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a_bytes.iter().zip(b_bytes.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthTokenInfo {
    pub token: String,
    pub created_at_epoch: u64,
    pub expires_at_epoch: u64,
    pub client_ip: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortalAuthData {
    pub setup_code: String,
    pub tokens: HashMap<String, AuthTokenInfo>,
}

#[derive(Debug)]
struct FailedAttemptTracker {
    attempts: Vec<Instant>,
}

#[derive(Debug)]
pub struct PortalAuthManager {
    auth_file_path: PathBuf,
    data: PortalAuthData,
    failed_attempts: Mutex<HashMap<String, FailedAttemptTracker>>,
}

impl PortalAuthManager {
    pub fn new<P: AsRef<Path>>(auth_file_path: P) -> Result<Self> {
        let path = auth_file_path.as_ref().to_path_buf();
        let data = Self::load_or_create(&path)?;

        Ok(Self {
            auth_file_path: path,
            data,
            failed_attempts: Mutex::new(HashMap::new()),
        })
    }

    /// Load auth data from path or generate a new one-time setup code
    fn load_or_create(path: &Path) -> Result<PortalAuthData> {
        if path.is_file() {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(data) = serde_json::from_str::<PortalAuthData>(&content) {
                    return Ok(data);
                }
            }
        }

        // Generate a new secure setup code: RPI-VJ-XXXX-YYYY
        let part1 = &Uuid::new_v4().to_string()[..4].to_uppercase();
        let part2 = &Uuid::new_v4().to_string()[..4].to_uppercase();
        let setup_code = format!("RPI-VJ-{}-{}", part1, part2);

        let data = PortalAuthData {
            setup_code: setup_code.clone(),
            tokens: HashMap::new(),
        };

        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let json = serde_json::to_string_pretty(&data)?;
        let tmp_path = path.with_extension("tmp");
        if let Ok(mut f) = File::create(&tmp_path) {
            let _ = f.write_all(json.as_bytes());
            let _ = f.sync_all();
            drop(f);
            let _ = fs::rename(&tmp_path, path);
        }

        println!("[auth] Generated new portal setup code: {}", setup_code);
        Ok(data)
    }

    pub fn get_setup_code(&self) -> &str {
        &self.data.setup_code
    }

    /// Set a custom setup code (primarily for tests)
    pub fn set_setup_code(&mut self, code: &str) -> Result<()> {
        self.data.setup_code = code.to_string();
        self.save()
    }

    /// Authenticate using setup code, enforcing rate-limiting per IP
    pub fn login(&mut self, code: &str, client_ip: &str) -> Result<String> {
        let now = Instant::now();

        // 1. Check rate-limiting
        {
            let mut tracker_map = self.failed_attempts.lock().unwrap();
            let tracker = tracker_map
                .entry(client_ip.to_string())
                .or_insert_with(|| FailedAttemptTracker { attempts: Vec::new() });

            // Retain attempts within the rate-limit window
            tracker
                .attempts
                .retain(|&t| now.saturating_duration_since(t) <= AUTH_RATE_LIMIT_WINDOW);

            if tracker.attempts.len() >= MAX_AUTH_ATTEMPTS {
                if let Some(&first) = tracker.attempts.first() {
                    let elapsed = now.saturating_duration_since(first);
                    let remaining = AUTH_RATE_LIMIT_WINDOW
                        .saturating_sub(elapsed)
                        .as_secs()
                        .max(1);
                    return Err(AuthError::RateLimited {
                        seconds_remaining: remaining,
                    });
                }
            }
        }

        // 2. Validate setup code
        if !constant_time_eq(code.trim(), &self.data.setup_code) {
            let mut tracker_map = self.failed_attempts.lock().unwrap();
            let tracker = tracker_map
                .entry(client_ip.to_string())
                .or_insert_with(|| FailedAttemptTracker { attempts: Vec::new() });
            tracker.attempts.push(now);

            return Err(AuthError::InvalidCredentials);
        }

        // 3. Clear failed attempts on success
        {
            let mut tracker_map = self.failed_attempts.lock().unwrap();
            tracker_map.remove(client_ip);
        }

        // 4. Generate token and persist
        let token = format!("vj_tok_{}", Uuid::new_v4().simple());
        let current_epoch = get_epoch_secs();
        let expires_at = current_epoch + TOKEN_EXPIRY_SECS;

        let token_info = AuthTokenInfo {
            token: token.clone(),
            created_at_epoch: current_epoch,
            expires_at_epoch: expires_at,
            client_ip: client_ip.to_string(),
        };

        // Prune expired tokens
        self.data
            .tokens
            .retain(|_, info| info.expires_at_epoch > current_epoch);

        self.data.tokens.insert(token.clone(), token_info);
        self.save()?;

        Ok(token)
    }

    /// Validate token existence and expiry
    pub fn validate_token(&self, token: &str) -> bool {
        let current_epoch = get_epoch_secs();
        if let Some(info) = self.data.tokens.get(token) {
            info.expires_at_epoch > current_epoch
        } else {
            false
        }
    }

    /// Revoke token
    pub fn revoke_token(&mut self, token: &str) -> Result<bool> {
        let removed = self.data.tokens.remove(token).is_some();
        if removed {
            self.save()?;
        }
        Ok(removed)
    }

    /// Save state to auth file
    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.auth_file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp_path = self.auth_file_path.with_extension("tmp");
        let json = serde_json::to_string_pretty(&self.data)?;
        {
            let mut f = File::create(&tmp_path)?;
            f.write_all(json.as_bytes())?;
            f.sync_all()?;
        }
        fs::rename(&tmp_path, &self.auth_file_path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_setup_and_login() {
        let dir = tempfile::tempdir().unwrap();
        let auth_path = dir.path().join("portal-auth.json");

        let mut mgr = PortalAuthManager::new(&auth_path).unwrap();
        let code = mgr.get_setup_code().to_string();
        assert!(code.starts_with("RPI-VJ-"));

        // Login with correct code
        let token_res = mgr.login(&code, "192.168.1.10");
        assert!(token_res.is_ok());
        let token = token_res.unwrap();
        assert!(mgr.validate_token(&token));

        // Invalid token
        assert!(!mgr.validate_token("invalid_token"));
    }

    #[test]
    fn test_auth_rate_limiting() {
        let dir = tempfile::tempdir().unwrap();
        let auth_path = dir.path().join("portal-auth.json");

        let mut mgr = PortalAuthManager::new(&auth_path).unwrap();
        let ip = "192.168.1.50";

        // 5 consecutive failed attempts
        for _ in 0..MAX_AUTH_ATTEMPTS {
            let res = mgr.login("WRONG-CODE", ip);
            assert!(matches!(res, Err(AuthError::InvalidCredentials)));
        }

        // 6th attempt must be rate-limited
        let res = mgr.login("WRONG-CODE", ip);
        assert!(matches!(res, Err(AuthError::RateLimited { .. })));

        // Even with correct code, must be rate-limited while blocked
        let code = mgr.get_setup_code().to_string();
        let res2 = mgr.login(&code, ip);
        assert!(matches!(res2, Err(AuthError::RateLimited { .. })));

        // Different IP is not blocked
        let res_diff_ip = mgr.login(&code, "192.168.1.51");
        assert!(res_diff_ip.is_ok());
    }
}
