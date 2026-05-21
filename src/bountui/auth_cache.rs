use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Serialize, Deserialize)]
struct CachedAuthToken {
    token: String,
    user_id: String,
}

pub fn save_auth_token(path: &Path, token: &str, user_id: &str) -> Result<(), String> {
    let cached = CachedAuthToken {
        token: token.to_string(),
        user_id: user_id.to_string(),
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {e}"))?;
    }
    let json =
        serde_json::to_string_pretty(&cached).map_err(|e| format!("Failed to serialize: {e}"))?;
    fs::write(path, json).map_err(|e| format!("Failed to write: {e}"))?;
    Ok(())
}

pub fn load_auth_token(path: &Path) -> Result<Option<(String, String)>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let content =
        fs::read_to_string(path).map_err(|e| format!("Failed to read auth token: {e}"))?;
    let cached: CachedAuthToken =
        serde_json::from_str(&content).map_err(|e| format!("Failed to parse auth token: {e}"))?;
    Ok(Some((cached.token, cached.user_id)))
}

pub fn clear_auth_token(path: &Path) -> Result<(), String> {
    if path.exists() {
        fs::remove_file(path).map_err(|e| format!("Failed to remove auth token: {e}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn save_and_load_roundtrip() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path();

        save_auth_token(path, "at_abc123", "u_xyz789").unwrap();

        let (token, user_id) = load_auth_token(path).unwrap().unwrap();
        assert_eq!(token, "at_abc123");
        assert_eq!(user_id, "u_xyz789");
    }

    #[test]
    fn load_nonexistent_file() {
        let result = load_auth_token(Path::new("/tmp/does_not_exist_42.json")).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn clear_removes_file() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path();

        save_auth_token(path, "at_x", "u_y").unwrap();
        assert!(path.exists());

        clear_auth_token(path).unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn clear_nonexistent_file_is_okay() {
        let result = clear_auth_token(Path::new("/tmp/does_not_exist_99.json"));
        assert!(result.is_ok());
    }

    #[test]
    fn overwrites_existing_token() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path();

        save_auth_token(path, "at_old", "u_old").unwrap();
        save_auth_token(path, "at_new", "u_new").unwrap();

        let (token, user_id) = load_auth_token(path).unwrap().unwrap();
        assert_eq!(token, "at_new");
        assert_eq!(user_id, "u_new");
    }

    #[test]
    fn corrupt_file_returns_error() {
        use std::io::Write;
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"not json").unwrap();
        let path = file.path();

        let result = load_auth_token(path);
        assert!(result.is_err());
    }
}