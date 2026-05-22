use serde::{Deserialize, Serialize};

/// Serializable struct containing the cached auth token and associated user id.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CachedAuth {
    pub token: String,
    pub user_id: String,
}

/// Trait for caching Boundary auth tokens.
pub trait AuthCache: Send + Sync {
    /// Returns `Some(CachedAuth)` if a valid cached token exists, `None` otherwise.
    fn get_cached_token(&self) -> Option<CachedAuth>;

    /// Store the auth token and associated user_id.
    fn cache_token(&self, token: &str, user_id: &str) -> anyhow::Result<()>;

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
        serde_json::from_str(&password).ok()
    }

    fn cache_token(&self, token: &str, user_id: &str) -> anyhow::Result<()> {
        let cached = CachedAuth {
            token: token.to_string(),
            user_id: user_id.to_string(),
        };
        let json = serde_json::to_string(&cached)?;
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

    fn cache_token(&self, _token: &str, _user_id: &str) -> anyhow::Result<()> {
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

    /// Hand-written mock that allows fine-grained control over the cached token.
    pub struct MockAuthCache {
        cached: std::sync::Mutex<Option<CachedAuth>>,
        cache_calls: std::sync::Mutex<Vec<(String, String)>>,
        available: bool,
    }

    impl Default for MockAuthCache {
        fn default() -> Self {
            Self {
                cached: std::sync::Mutex::new(None),
                cache_calls: std::sync::Mutex::new(Vec::new()),
                available: true,
            }
        }
    }

    impl MockAuthCache {
        /// Create a mock with a pre-cached token (simulates a cache hit).
        pub fn with_cached_token(token: &str, user_id: &str) -> Self {
            Self {
                cached: std::sync::Mutex::new(Some(CachedAuth {
                    token: token.to_string(),
                    user_id: user_id.to_string(),
                })),
                cache_calls: std::sync::Mutex::new(Vec::new()),
                available: true,
            }
        }

        /// Create a mock without any cached token (simulates a cache miss).
        pub fn without_cache() -> Self {
            Self::default()
        }

        /// Create a mock where the keyring is not available.
        pub fn unavailable() -> Self {
            Self {
                cached: std::sync::Mutex::new(None),
                cache_calls: std::sync::Mutex::new(Vec::new()),
                available: false,
            }
        }

        /// Return the list of (token, user_id) pairs that were cached via `cache_token`.
        pub fn cache_call_args(&self) -> Vec<(String, String)> {
            self.cache_calls.lock().unwrap().clone()
        }
    }

    impl AuthCache for MockAuthCache {
        fn get_cached_token(&self) -> Option<CachedAuth> {
            self.cached.lock().unwrap().clone()
        }

        fn cache_token(&self, token: &str, user_id: &str) -> anyhow::Result<()> {
            self.cache_calls
                .lock()
                .unwrap()
                .push((token.to_string(), user_id.to_string()));
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

    #[test]
    fn test_mock_auth_cache_with_token() {
        let cache = MockAuthCache::with_cached_token("tk123", "user-1");
        assert!(cache.is_available());
        let cached = cache.get_cached_token().unwrap();
        assert_eq!(cached.token, "tk123");
        assert_eq!(cached.user_id, "user-1");
    }

    #[test]
    fn test_mock_auth_cache_without_token() {
        let cache = MockAuthCache::without_cache();
        assert!(cache.is_available());
        assert!(cache.get_cached_token().is_none());
    }

    #[test]
    fn test_mock_auth_cache_unavailable() {
        let cache = MockAuthCache::unavailable();
        assert!(!cache.is_available());
        assert!(cache.get_cached_token().is_none());
    }

    #[test]
    fn test_mock_auth_cache_records_cache_calls() {
        let cache = MockAuthCache::without_cache();
        cache.cache_token("tk456", "user-2").unwrap();
        let calls = cache.cache_call_args();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "tk456");
        assert_eq!(calls[0].1, "user-2");
    }

    #[test]
    fn test_mock_clear_cache() {
        let cache = MockAuthCache::with_cached_token("tk789", "user-3");
        assert!(cache.get_cached_token().is_some());
        cache.clear_cache().unwrap();
        assert!(cache.get_cached_token().is_none());
    }
}
