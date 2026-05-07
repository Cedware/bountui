use crate::boundary;
use crate::boundary::{ApiClient, BoundaryConnectionHandle};
use chrono::{DateTime, Utc};
use futures::future::join_all;
use log::{error, info};
use std::collections::HashMap;
use std::future::{pending, Future};
use std::sync::{Arc, Mutex};
use tokio::select;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

#[derive(thiserror::Error, Debug)]
pub enum ConnectionError {
    #[error("Boundary error: {0:?}")]
    BoundaryError(#[from] boundary::Error),
    #[error("Failed to stop the connection: The session id '{0}' is unknown")]
    StopFailedUnknownSessionId(String),
}

struct ConnectionEntry {
    cancellation_token: CancellationToken,
    join_handle: JoinHandle<()>,
    credentials: Option<Vec<boundary::CredentialEntry>>,
}

#[cfg_attr(test, mockall::automock)]
pub trait ConnectionManager {
    fn connect(&self, target_id: &str, port: u16) -> impl Future<Output=Result<boundary::ConnectResponse, boundary::Error>>;
    fn shutdown(&self) -> impl Future<Output=Result<(), Vec<ConnectionError>>>;
    fn stop(&self, id: &str) -> impl Future<Output=Result<(), ConnectionError>>;
    fn get_credentials(&self) -> HashMap<String, Vec<boundary::CredentialEntry>>;
}

pub struct DefaultConnectionManager<C> {
    connections: Arc<Mutex<HashMap<String, ConnectionEntry>>>,
    boundary_client: C,
}

impl<C> DefaultConnectionManager<C> {
    pub fn new(boundary_client: C) -> Self {
        DefaultConnectionManager {
            connections: Arc::new(Mutex::new(HashMap::new())),
            boundary_client,
        }
    }

    async fn wait_until_session_is_expired(expiration_time: DateTime<Utc>) {
        let expires_in = expiration_time - Utc::now();
        match expires_in.to_std() {
            Ok(duration) => {
                tokio::time::sleep(duration).await;
            }
            Err(e) => {
                error!("Could not convert expiration time to duration: {:?}", e);
                pending::<()>().await;
            }
        }
    }

    fn spawn_connection_task<H>(connections: Arc<Mutex<HashMap<String, ConnectionEntry>>>, mut connection_handle: H, cancellation_token: CancellationToken, expiration_time: DateTime<Utc>, session_id: String) -> JoinHandle<()>
    where
        H: BoundaryConnectionHandle + 'static,
    {
        tokio::spawn(async move {
            let stop_result = select! {
                    _ = cancellation_token.cancelled() =>  {
                        info!("Session was cancelled via cancellation token");
                        connection_handle.stop().await
                    },
                    _ = connection_handle.wait() =>  {
                        info!("Connection handle was stopped via connection handle");
                        Ok(())
                    },
                    _ = Self::wait_until_session_is_expired(expiration_time)  => {
                        info!("Boundary session expired");
                        connection_handle.stop().await
                    },
                };
            if let Err(e) = stop_result {
                error!("Connection handle was stopped with and error {:?}", e)
            }
            connections.lock().unwrap().remove(&session_id);
        })
    }

    async fn stop_connection_entry(&self, id: &str, connection_entry: ConnectionEntry) -> Result<(), ConnectionError>
    where
        C: ApiClient,
    {
        self.boundary_client.cancel_session(id).await?;
        connection_entry.cancellation_token.cancel();
        let _ = connection_entry.join_handle.await; //Even when the task failed the stop is considered successful
        Ok(())
    }
}

impl<C> ConnectionManager for DefaultConnectionManager<C>
where
    C: boundary::ApiClient,
    C::ConnectionHandle: 'static,
{
    async fn connect(
        &self,
        target_id: &str,
        port: u16,
    ) -> Result<boundary::ConnectResponse, boundary::Error>

    {
        let (response, connection_handle) =
            self.boundary_client.connect(&target_id, port).await?;
        let cancellation_token = CancellationToken::new();
        let join_handle = Self::spawn_connection_task(self.connections.clone(), connection_handle, cancellation_token.clone(), response.expiration, response.session_id.clone());
        let credentials = if response.credentials.is_empty() {
            None
        } else {
            Some(response.credentials.clone())
        };
        self.connections.lock().unwrap().insert(response.session_id.clone(), ConnectionEntry { cancellation_token, join_handle, credentials });
        Ok(response)
    }

    async fn shutdown(&self) -> Result<(), Vec<ConnectionError>>
    {
        info!("Shutting down connection manager");
        let stop_futures: Vec<_> = self.connections.lock().unwrap()
            .drain()
            .map(|(id, entry)| async move { self.stop_connection_entry(&id, entry).await })
            .collect();
        let stop_results = join_all(stop_futures).await;
        let mut errors = Vec::new();
        for result in stop_results {
            if let Err(e) = result {
                errors.push(e);
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    async fn stop(&self, id: &str) -> Result<(), ConnectionError>
    {
        let connection_entry = self.connections.lock().unwrap()
            .remove(id)
            .ok_or(ConnectionError::StopFailedUnknownSessionId(id.to_string()))?;
        self.stop_connection_entry(id, connection_entry).await
    }

    fn get_credentials(&self) -> HashMap<String, Vec<boundary::CredentialEntry>> {
        self.connections.lock().unwrap()
            .iter()
            .filter_map(|(id, entry)| {
                entry.credentials.as_ref().map(|creds| (id.clone(), creds.clone()))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::boundary;
    use crate::boundary::{Scope, Target};
    use crate::bountui::connection_manager::{ConnectionManager, DefaultConnectionManager};
    use chrono::TimeDelta;
    use std::collections::HashMap;
    use std::ops::Add;
    use std::time::Duration;

    const TARGET_ID: &str = "target-1";
    const SCOPE_ID: &str = "scope-1";

    fn create_boundary_client() -> boundary::MockClient {
        let mut scopes = HashMap::new();
        scopes.insert(None, vec![Scope {
            id: SCOPE_ID.to_string(),
            name: "scope 1".to_string(),
            description: "scope 1".to_string(),
            type_name: "".to_string(),
            authorized_collection_actions: Default::default(),
        }]);

        let mut targets = HashMap::new();
        targets.insert(Some("scope-1".to_string()), vec![Target {
            id: TARGET_ID.to_string(),
            name: "target 1".to_string(),
            description: "target 1".to_string(),
            type_name: "".to_string(),
            authorized_collection_actions: Default::default(),
            authorized_actions: vec![],
            scope_id: "scope-1".to_string(),
            attributes: None,
        }]);

        boundary::MockClient::builder()
            .session_lifetime(TimeDelta::hours(8))
            .scopes(scopes)
            .targets(targets)
            .build()
    }

    #[tokio::test(start_paused = true)]
    async fn test_connection_is_closed_after_sessions_is_expired() {
        let boundary_client = create_boundary_client();
        let sut = DefaultConnectionManager::new(boundary_client.clone());
        let connect_response = sut.connect(TARGET_ID, 8080).await.unwrap();
        tokio::time::sleep(TimeDelta::hours(8).add(TimeDelta::minutes(1)).to_std().unwrap()).await;
        let connection_handle = boundary_client.get_connection_handle(&connect_response.session_id).await.unwrap();
        assert!(connection_handle.is_stopped(), "The connection handle should be stopped after the session is expired");

    }


    #[tokio::test(start_paused = true)]
    async fn test_connection_is_not_closed_before_session_is_expired() {
        let boundary_client = create_boundary_client();
        let sut = DefaultConnectionManager::new(boundary_client.clone());
        let connect_response = sut.connect(TARGET_ID, 8080).await.unwrap();
        tokio::time::sleep(Duration::from_secs(5)).await;
        let connection_handle = boundary_client.get_connection_handle(&connect_response.session_id).await.unwrap();
        assert!(!connection_handle.is_stopped(), "The connection handle should not be stopped before the session is expired");
    }

    #[tokio::test(start_paused = true)]
    async fn test_stop_session() {
        let boundary_client = create_boundary_client();
        let sut = DefaultConnectionManager::new(boundary_client.clone());
        let resp = sut
            .connect(TARGET_ID, 8080)
            .await
            .expect("Should be able to connect to target");
        tokio::time::sleep(Duration::from_secs(5)).await;
        sut.stop(&resp.session_id)
            .await
            .expect("Should be able to stop session");
        let connection_handle = boundary_client.get_connection_handle(&resp.session_id).await.expect("Should be able to get connection handle");
        assert!(connection_handle.is_stopped(), "The connection handle should stopped");
    }

    #[tokio::test(start_paused = true)]
    async fn test_shutdown() {
        let boundary_client = create_boundary_client();
        let sut = DefaultConnectionManager::new(boundary_client.clone());

        let connect_response_1 = sut.connect(TARGET_ID, 8080).await.expect("Should be able to connect to target");
        let connect_response_2 = sut.connect(TARGET_ID, 8081).await.expect("Should be able to connect to target");
        let connect_response_3 = sut.connect(TARGET_ID, 8082).await.expect("Should be able to connect to target");

        tokio::time::sleep(Duration::from_secs(5)).await;
        sut.shutdown().await.expect("Shutdown should succeed");

        let connection_handle_1 = boundary_client.get_connection_handle(&connect_response_1.session_id).await.expect("Should be able to get connection handle");
        let connection_handle_2 = boundary_client.get_connection_handle(&connect_response_2.session_id).await.expect("Should be able to get connection handle");
        let connection_handle_3 = boundary_client.get_connection_handle(&connect_response_3.session_id).await.expect("Should be able to get connection handle");

        assert!(connection_handle_1.is_stopped(), "The connection handle should stopped");
        assert!(connection_handle_2.is_stopped(), "The connection handle should stop");
        assert!(connection_handle_3.is_stopped(), "The connection handle should stop");
    }
}
