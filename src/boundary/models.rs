use bon::Builder;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Builder, Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct Scope {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(rename = "type")]
    pub type_name: String,
    #[serde(default)]
    pub authorized_collection_actions: HashMap<String, Vec<String>>,
}

impl Scope {
    pub fn can_list_child_scopes(&self) -> bool {
        self.authorized_collection_actions
            .get("scopes")
            .map(|actions| actions.contains(&"list".to_string()))
            .unwrap_or(false)
    }

    pub fn can_list_targets(&self) -> bool {
        self.authorized_collection_actions
            .get("targets")
            .map(|actions| actions.contains(&"list".to_string()))
            .unwrap_or(false)
    }
}

#[derive(Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct Target {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(rename = "type")]
    pub type_name: String,
    #[serde(default)]
    pub authorized_collection_actions: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub authorized_actions: Vec<String>,
    pub scope_id: String,
}

impl PartialOrd for Target {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.name.cmp(&other.name))
    }
}

impl Target {
    pub fn can_connect(&self) -> bool {
        self.authorized_actions
            .contains(&"authorize-session".to_string())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Credential {
    pub username: String,
    pub password: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq,)]
pub struct CredentialEntry {
    pub credential: Credential,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ConnectResponse {
    #[serde(default)]
    pub credentials: Vec<CredentialEntry>,
    pub session_id: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Session {
    pub id: String,
    pub target_id: String,
    #[serde(rename = "type")]
    pub session_type: String,
    pub created_time: DateTime<Utc>,
    pub status: String,
    pub authorized_actions: Vec<String>,
}

impl Session {
    pub fn can_cancel(&self) -> bool {
        self.authorized_actions.contains(&"cancel:self".to_string())
    }
}
