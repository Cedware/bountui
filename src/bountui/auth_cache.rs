use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The canonical in-memory representation of a cached auth token.
/// All fields are guaranteed present — the type system enforces it.
#[derive(Debug, Clone)]
pub struct CachedAuth {
    pub token: String,
    pub user_id: String,
    pub token_id: String,
    pub expiration_time: DateTime<Utc>,
}

/// Serializable wire format for persisting to the keyring.
/// `token_id` and `expiration_time` are optional — both were added after the
/// initial version and may be missing in entries written by older versions.
#[derive(Serialize, Deserialize, Debug)]
struct SerializedCachedAuth {
    token: String,
    user_id: String,
    #[serde(default)]
    token_id: Option<String>,
    #[serde(default)]
    expiration_time: Option<DateTime<Utc>>,
}

impl SerializedCachedAuth {
    /// Convert to the canonical `CachedAuth` if all required fields are present.
    fn into_cached(self) -> Option<CachedAuth> {
        Some(CachedAuth {
            token: self.token,
            user_id: self.user_id,
            token_id: self.token_id?,
            expiration_time: self.expiration_time?,
        })
    }
}

impl From<&CachedAuth> for SerializedCachedAuth {
    fn from(c: &CachedAuth) -> Self {
        SerializedCachedAuth {
            token: c.token.clone(),
            user_id: c.user_id.clone(),
            token_id: Some(c.token_id.clone()),
            expiration_time: Some(c.expiration_time),
        }
    }
}

/// Trait for caching Boundary auth tokens.
pub trait AuthCache: Send + Sync {
    /// Returns `Some(CachedAuth)` if a valid cached token exists, `None` otherwise.
    fn get_cached_token(&self) -> Option<CachedAuth>;

    /// Store the auth token and associated user_id and expiration time.
    fn cache_token(
        &self,
        token: &str,
        user_id: &str,
        expiration_time: DateTime<Utc>,
        token_id: &str,
    ) -> anyhow::Result<()>;

    /// Remove the cached credential from the keyring.
    fn clear_cache(&self) -> anyhow::Result<()>;

    /// Whether the keyring backend is available on this system.
    fn is_available(&self) -> bool;
}

/// Real implementation using the system keyring via the `keyring` crate.
///
/// The token and user_id are stored as a JSON blob in the keyring password field.
/// Service: `"bountui"`, Account: `"auth-token"`.
pub struct KeyringAuthCache {
    entry: keyring_core::Entry,
}

impl KeyringAuthCache {
    /// Try to initialize the native keyring store and create an entry.
    ///
    /// Returns `None` **and logs the reason** if no keyring backend could be set up.
    pub fn new() -> Option<Self> {
        log::info!(
            "auth_cache: initialising native keyring store (platform={})",
            std::env::consts::OS,
        );

        match keyring::use_native_store(true) {
            Ok(info) => {
                log::info!("auth_cache: native store set successfully: {info:?}");
            }
            Err(e) => {
                log::warn!(
                    "auth_cache: failed to set native keyring store: {e}. \
                     This is expected on headless systems or when no keyring daemon is running."
                );
                return None;
            }
        }

        log::info!("auth_cache: store_info: {}", keyring::store_info(),);

        match keyring_core::Entry::new("bountui", "auth-token") {
            Ok(entry) => {
                log::info!(
                    "auth_cache: entry created successfully (service=bountui, account=auth-token)"
                );
                Some(Self { entry })
            }
            Err(e) => {
                log::warn!(
                    "auth_cache: failed to create keyring entry: {e}. \
                     Caching will be disabled."
                );
                None
            }
        }
    }
}

impl AuthCache for KeyringAuthCache {
    fn get_cached_token(&self) -> Option<CachedAuth> {
        let password = self.entry.get_password().ok()?;
        let serialized: SerializedCachedAuth = serde_json::from_str(&password).ok()?;
        let cached = serialized.into_cached();
        match &cached {
            Some(c) if c.expiration_time > Utc::now() => cached,
            Some(_) => {
                log::warn!("auth_cache: cached token is expired");
                None
            }
            None => {
                log::warn!("auth_cache: cached entry is incomplete or from an older version");
                None
            }
        }
    }

    fn cache_token(
        &self,
        token: &str,
        user_id: &str,
        expiration_time: DateTime<Utc>,
        token_id: &str,
    ) -> anyhow::Result<()> {
        let cached = CachedAuth {
            token: token.to_string(),
            user_id: user_id.to_string(),
            token_id: token_id.to_string(),
            expiration_time,
        };
        let json = serde_json::to_string(&SerializedCachedAuth::from(&cached))?;
        self.entry.set_password(&json)?;
        Ok(())
    }

    fn clear_cache(&self) -> anyhow::Result<()> {
        self.entry.delete_credential()?;
        Ok(())
    }

    fn is_available(&self) -> bool {
        true
    }
}

/// Fallback implementation when no keyring backend is available.
///
/// All operations are no-ops. `is_available` returns `false`.
pub struct NoopAuthCache;

impl AuthCache for NoopAuthCache {
    fn get_cached_token(&self) -> Option<CachedAuth> {
        None
    }

    fn cache_token(
        &self,
        _token: &str,
        _user_id: &str,
        _expiration_time: DateTime<Utc>,
        _token_id: &str,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn clear_cache(&self) -> anyhow::Result<()> {
        Ok(())
    }

    fn is_available(&self) -> bool {
        false
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use bon::builder;

    /// Hand-written mock that allows fine-grained control over the cached token.
    pub struct MockAuthCache {
        cached: std::sync::Mutex<Option<CachedAuth>>,
        cache_calls: std::sync::Mutex<Vec<(String, String, DateTime<Utc>, String)>>,
        available: bool,
    }

    #[builder]
    pub fn mock_auth_cache(
        token: Option<&str>,
        user_id: Option<&str>,
        expiration_time: Option<DateTime<Utc>>,
        token_id: Option<&str>,
        #[builder(default = true)] available: bool,
    ) -> MockAuthCache {
        let cached = match (token, user_id, token_id, expiration_time) {
            (Some(token), Some(user_id), Some(token_id), Some(expiration_time)) => {
                Some(CachedAuth {
                    token: token.to_string(),
                    user_id: user_id.to_string(),
                    token_id: token_id.to_string(),
                    expiration_time,
                })
            }
            _ => None,
        };
        MockAuthCache {
            cached: std::sync::Mutex::new(cached),
            cache_calls: std::sync::Mutex::new(Vec::new()),
            available,
        }
    }

    impl AuthCache for MockAuthCache {
        fn get_cached_token(&self) -> Option<CachedAuth> {
            let cached = self.cached.lock().unwrap().clone()?;
            match cached.expiration_time {
                exp if exp > Utc::now() => Some(cached),
                _ => None,
            }
        }

        fn cache_token(
            &self,
            token: &str,
            user_id: &str,
            expiration_time: DateTime<Utc>,
            token_id: &str,
        ) -> anyhow::Result<()> {
            self.cache_calls.lock().unwrap().push((
                token.to_string(),
                user_id.to_string(),
                expiration_time,
                token_id.to_string(),
            ));
            Ok(())
        }

        fn clear_cache(&self) -> anyhow::Result<()> {
            *self.cached.lock().unwrap() = None;
            Ok(())
        }

        fn is_available(&self) -> bool {
            self.available
        }
    }
}
