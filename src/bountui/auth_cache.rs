use serde::{Deserialize, Serialize};
use std::path::Path;

pub trait AuthTokenCache {
    fn save_auth_token(&self, token: &str, user_id: &str) -> Result<(), String>;
    fn load_auth_token(&self) -> Result<Option<(String, String)>, String>;
    fn clear_auth_token(&self) -> Result<(), String>;
}

#[derive(Serialize, Deserialize)]
struct CachedAuthToken {
    token: String,
    user_id: String,
}

impl AuthTokenCache for &Path {
    fn save_auth_token(&self, token: &str, user_id: &str) -> Result<(), String> {
        let cached = CachedAuthToken {
            token: token.to_string(),
            user_id: user_id.to_string(),
        };
        if let Some(parent) = self.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory: {e}"))?;
        }
        let json = serde_json::to_string_pretty(&cached)
            .map_err(|e| format!("Failed to serialize: {e}"))?;
        std::fs::write(self, json).map_err(|e| format!("Failed to write: {e}"))?;
        Ok(())
    }

    fn load_auth_token(&self) -> Result<Option<(String, String)>, String> {
        if !self.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(self)
            .map_err(|e| format!("Failed to read auth token: {e}"))?;
        let cached: CachedAuthToken =
            serde_json::from_str(&content).map_err(|e| format!("Failed to parse auth token: {e}"))?;
        Ok(Some((cached.token, cached.user_id)))
    }

    fn clear_auth_token(&self) -> Result<(), String> {
        if self.exists() {
            std::fs::remove_file(self)
                .map_err(|e| format!("Failed to remove auth token: {e}"))?;
        }
        Ok(())
    }
}

impl AuthTokenCache for std::path::PathBuf {
    fn save_auth_token(&self, token: &str, user_id: &str) -> Result<(), String> {
        self.as_path().save_auth_token(token, user_id)
    }

    fn load_auth_token(&self) -> Result<Option<(String, String)>, String> {
        self.as_path().load_auth_token()
    }

    fn clear_auth_token(&self) -> Result<(), String> {
        self.as_path().clear_auth_token()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn save_and_load_roundtrip() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path();

        path.save_auth_token("at_abc123", "u_xyz789").unwrap();

        let (token, user_id) = path.load_auth_token().unwrap().unwrap();
        assert_eq!(token, "at_abc123");
        assert_eq!(user_id, "u_xyz789");
    }

    #[test]
    fn load_nonexistent_file() {
        let path = Path::new("/tmp/does_not_exist_42.json");
        let result = path.load_auth_token().unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn clear_removes_file() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path();

        path.save_auth_token("at_x", "u_y").unwrap();
        assert!(path.exists());

        path.clear_auth_token().unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn clear_nonexistent_file_is_okay() {
        let path = Path::new("/tmp/does_not_exist_99.json");
        let result = path.clear_auth_token();
        assert!(result.is_ok());
    }

    #[test]
    fn overwrites_existing_token() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path();

        path.save_auth_token("at_old", "u_old").unwrap();
        path.save_auth_token("at_new", "u_new").unwrap();

        let (token, user_id) = path.load_auth_token().unwrap().unwrap();
        assert_eq!(token, "at_new");
        assert_eq!(user_id, "u_new");
    }

    #[test]
    fn corrupt_file_returns_error() {
        use std::io::Write;
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"not json").unwrap();
        let path = file.path();

        let result = path.load_auth_token();
        assert!(result.is_err());
    }
}