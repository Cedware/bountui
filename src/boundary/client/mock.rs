use crate::boundary::client::response::{AuthenticateAttributes, AuthenticateResponse};
use crate::boundary::{
    ApiClient, BoundaryConnectionHandle, ConnectResponse, Error, Scope, Session, Target,
};
use bon::Builder;
use chrono::{Duration, Utc};
use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{Mutex, Notify};

#[derive(Builder, Clone)]
pub struct MockClient {
    #[builder(default)]
    session_lifetime: Duration,
    #[builder(default)]
    user_id: String,
    #[builder(default)]
    authenticate_should_fail: bool,
    #[builder(default = 401)]
    authenticate_error_status: u16,
    #[builder(default = "denied".to_string())]
    authenticate_error_message: String,
    scopes: HashMap<Option<String>, Vec<Scope>>,
    #[builder(default)]
    targets: HashMap<Option<String>, Vec<Target>>,
    #[builder(default)]
    sessions: Arc<Mutex<HashMap<String, Vec<Session>>>>,
    #[builder(default)]
    connection_handles: Arc<Mutex<HashMap<String, MockConnectionHandle>>>,
}

impl ApiClient for MockClient {
    type ConnectionHandle = MockConnectionHandle;

    fn get_scopes(
        &self,
        parent: Option<&str>,
        recursive: bool,
    ) -> impl Future<Output = Result<Vec<Scope>, Error>> + Send {
        Box::pin(async move {
            let scopes = match parent {
                Some(parent) => self
                    .scopes
                    .get(&Some(parent.to_string()))
                    .cloned()
                    .unwrap_or_default(),
                None => self.scopes.get(&None).cloned().unwrap_or_default(),
            };
            if !recursive {
                Ok(scopes)
            } else {
                let mut scopes_aac = Vec::new();
                for scope in scopes {
                    let child_scopes = self.get_scopes(Some(&scope.id), true).await?;
                    scopes_aac.extend(child_scopes);
                }
                Ok(scopes_aac)
            }
        })
    }

    async fn get_targets(&self, scope: Option<&str>) -> Result<Vec<Target>, Error> {
        let targets = match scope {
            Some(scope) => self
                .targets
                .get(&Some(scope.to_string()))
                .cloned()
                .unwrap_or_default(),
            None => self.targets.get(&None).cloned().unwrap_or_default(),
        };
        Ok(targets)
    }

    async fn get_sessions(&self, scope: &str) -> Result<Vec<Session>, Error> {
        Ok(self
            .sessions
            .lock()
            .await
            .get(scope)
            .cloned()
            .unwrap_or_default())
    }

    async fn get_user_sessions(&self, user_id: &str) -> Result<Vec<Session>, Error> {
        let user_sessions = self
            .sessions
            .lock()
            .await
            .iter()
            .flat_map(|(_, sessions)| sessions.iter())
            .filter(|s| s.user_id == user_id)
            .cloned()
            .collect();
        Ok(user_sessions)
    }

    async fn connect(
        &self,
        target_id: &str,
        _port: u16,
    ) -> Result<(ConnectResponse, Self::ConnectionHandle), Error> {
        let all_targets = self.get_all_targets();
        let target = all_targets
            .iter()
            .find(|t| t.id == target_id)
            .ok_or_else(|| Error::ApiError(404, format!("no target with id: {}", target_id)))?;
        let session_id = uuid::Uuid::new_v4();
        self.sessions
            .lock()
            .await
            .entry(target.scope_id.clone())
            .or_insert_with(Vec::new)
            .push(Session {
                id: session_id.to_string(),
                target_id: target_id.to_string(),
                session_type: "".to_string(),
                created_time: Default::default(),
                status: "".to_string(),
                authorized_actions: vec![],
                user_id: "".to_string(),
            });

        let connection_handle = MockConnectionHandle::default();
        self.connection_handles
            .lock()
            .await
            .insert(session_id.to_string(), connection_handle.clone());

        Ok((
            ConnectResponse {
                credentials: vec![],
                session_id: session_id.to_string(),
                expiration: Utc::now() + self.session_lifetime,
            },
            connection_handle,
        ))
    }

    async fn cancel_session(&self, session_id: &str) -> Result<(), Error> {
        self.sessions.lock().await.remove(session_id);
        Ok(())
    }

    async fn authenticate(&self) -> Result<AuthenticateResponse, Error> {
        if self.authenticate_should_fail {
            return Err(Error::ApiError(
                self.authenticate_error_status,
                self.authenticate_error_message.clone(),
            ));
        }

        Ok(AuthenticateResponse {
            attributes: AuthenticateAttributes {
                user_id: self.user_id.to_string(),
                token: format!("token_for_{}", self.user_id),
            },
        })
    }
}

impl MockClient {
    fn get_all_targets(&self) -> Vec<&Target> {
        self.targets.values().flatten().collect()
    }
    pub async fn get_connection_handle(&self, session_id: &str) -> Option<MockConnectionHandle> {
        self.connection_handles
            .lock()
            .await
            .get(session_id)
            .cloned()
    }
}

#[derive(Default, Clone)]
pub struct MockConnectionHandle {
    notify: Arc<Notify>,
    stopped: Arc<AtomicBool>,
}

impl BoundaryConnectionHandle for MockConnectionHandle {
    type Error = String;

    async fn wait(&mut self) -> Result<(), Self::Error> {
        Ok(self.notify.notified().await)
    }
    async fn stop(&mut self) -> Result<(), Self::Error> {
        self.stopped.store(true, Ordering::SeqCst);
        self.notify.notify_waiters();
        Ok(())
    }
}

impl MockConnectionHandle {
    pub fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::SeqCst)
    }
}
