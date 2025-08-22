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
    #[error("Boundary error: {0}")]
    BoundaryError(#[from] boundary::Error),
    #[error("Failed to stop the connection: The session id '{0}' is unknown")]
    StopFailedUnknownSessionId(String),
}

struct ConnectionEntry {
    cancellation_token: CancellationToken,
    join_handle: JoinHandle<()>,
}

#[cfg_attr(test, mockall::automock)]
pub trait ConnectionManager {
    fn connect(&self, target_id: &str, port: u16) -> impl Future<Output=Result<boundary::ConnectResponse, boundary::Error>>;
    fn shutdown(&self) -> impl Future<Output=Result<(), Vec<ConnectionError>>>;
    fn stop(&self, id: &str) -> impl Future<Output=Result<(), ConnectionError>>;
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
        self.connections.lock().unwrap().insert(response.session_id.clone(), ConnectionEntry { cancellation_token, join_handle });
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
}

#[cfg(test)]
mod tests {
    use crate::boundary::client::{MockApiClient, MockBoundaryConnectionHandle};
    use crate::boundary::ConnectResponse;
    use crate::bountui::connection_manager::{ConnectionManager, DefaultConnectionManager};
    use crate::mock::StubError;
    use chrono::{TimeDelta, Utc};
    use futures::FutureExt;
    use mockall::predicate::eq;
    use std::future::pending;
    use std::ops::Add;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex;
    use tokio_test::assert_ok;

    struct SessionConfig {
        session_id: String,
        connection_handle: Arc<Mutex<MockBoundaryConnectionHandle>>,
        expect_cancellation: bool,
        life_time: TimeDelta,
    }

    fn configure_connection_handle(
        stop_result: Option<Result<(), StubError>>,
    ) -> Arc<Mutex<MockBoundaryConnectionHandle>> {
        let mut connection_handle = MockBoundaryConnectionHandle::new();
        connection_handle
            .expect_wait()
            .times(1)
            .returning(|| pending().boxed());

        if let Some(stop_result) = stop_result {
            connection_handle
                .expect_stop()
                .times(1)
                .return_once(|| async { stop_result }.boxed());
        }

        Arc::new(Mutex::new(connection_handle))
    }

    fn configure_boundary_client(
        sessions: Vec<SessionConfig>,
    ) -> MockApiClient {
        let mut boundary_client = MockApiClient::new();

        for session_config in sessions {
            let session_id = session_config.session_id.clone();
            let connection_handle = session_config.connection_handle;
            boundary_client
                .expect_connect()
                .times(1)
                .return_once(move |_, _| {
                    let connect_response = ConnectResponse {
                        session_id,
                        credentials: vec![],
                        expiration: Utc::now().add(session_config.life_time),
                    };
                    Ok((connect_response, connection_handle))
                });

            if session_config.expect_cancellation {
                boundary_client
                    .expect_cancel_session()
                    .times(1)
                    .with(eq(session_config.session_id))
                    .return_once(|_| Ok(()));
            }
        }


        boundary_client
    }

    #[tokio::test(start_paused = true)]
    async fn test_connection_is_closed_after_sessions_is_expired() {
        let session_id = "session-id";
        let connection_handle_stop_result = Ok(());
        let connection_handle_1 = configure_connection_handle(Some(connection_handle_stop_result));
        let session_config = SessionConfig {
            session_id: session_id.to_string(),
            expect_cancellation: false,
            life_time: TimeDelta::hours(8),
            connection_handle: connection_handle_1.clone(),
        };

        let boundary_client =
            configure_boundary_client(vec![session_config]);
        let sut = DefaultConnectionManager::new(boundary_client);
        sut.connect("target_id", 8080).await.unwrap();
        tokio::time::sleep(TimeDelta::hours(8).add(TimeDelta::minutes(1)).to_std().unwrap()).await;
        connection_handle_1.lock().await.checkpoint();
    }

    #[tokio::test(start_paused = true)]
    async fn test_connection_is_not_closed_before_session_is_expired() {
        let session_id = "session-id";
        let connection_handle_stop_result = None;
        let connection_handle = configure_connection_handle(connection_handle_stop_result);
        let session_config = SessionConfig {
            session_id: session_id.to_string(),
            expect_cancellation: false,
            life_time: TimeDelta::seconds(10),
            connection_handle: connection_handle.clone(),
        };
        let boundary_client =
            configure_boundary_client(vec![session_config]);
        let sut = DefaultConnectionManager::new(boundary_client);
        sut.connect("target_id", 8080).await.unwrap();
        tokio::time::sleep(Duration::from_secs(5)).await;
        connection_handle.lock().await.checkpoint();
    }

    #[tokio::test(start_paused = true)]
    async fn test_stop_session() {
        let session_id = "session-id";
        let connection_handle_stop_result = Some(Ok(()));
        let connection_handle = configure_connection_handle(connection_handle_stop_result);
        let session_config = SessionConfig {
            session_id: session_id.to_string(),
            expect_cancellation: true,
            life_time: TimeDelta::hours(8),
            connection_handle: connection_handle.clone(),
        };
        let boundary_client = configure_boundary_client(vec![session_config]);
        let sut = DefaultConnectionManager::new(boundary_client);
        let resp = sut
            .connect("target_id", 8080)
            .await
            .expect("Should be able to connect to target");
        tokio::time::sleep(Duration::from_secs(5)).await;
        sut.stop(&resp.session_id)
            .await
            .expect("Should be able to stop session");
        connection_handle.lock().await.checkpoint();
    }

    #[tokio::test(start_paused = true)]
    async fn test_shutdown() {
        let connection_handle_1_stop_result = Some(Ok(()));
        let connection_handle_1 = configure_connection_handle(connection_handle_1_stop_result);
        let session_1_config = SessionConfig {
            session_id: "session_id_1".to_string(),
            expect_cancellation: true,
            life_time: TimeDelta::hours(8),
            connection_handle: connection_handle_1.clone(),
        };

        let connection_handle_2_stop_result = Some(Ok(()));
        let connection_handle_2 = configure_connection_handle(connection_handle_2_stop_result);
        let session_2_config = SessionConfig {
            session_id: "session_id_2".to_string(),
            expect_cancellation: true,
            life_time: TimeDelta::hours(8),
            connection_handle: connection_handle_2.clone(),
        };

        let connection_handle_3_stop_result = Some(Ok(()));
        let connection_handle_3 = configure_connection_handle(connection_handle_3_stop_result);
        let session_3_config = SessionConfig {
            session_id: "session_id_3".to_string(),
            expect_cancellation: true,
            life_time: TimeDelta::hours(8),
            connection_handle: connection_handle_3.clone(),
        };

        let boundary_client = configure_boundary_client(vec![
            session_1_config,
            session_2_config,
            session_3_config]
        );
        let sut = DefaultConnectionManager::new(boundary_client);
        sut.connect("target_id_1", 8081).await.unwrap();
        sut.connect("target_id_2", 8082).await.unwrap();
        sut.connect("target_id_3", 8083).await.unwrap();

        tokio::time::sleep(Duration::from_secs(5)).await;
        let result = sut.shutdown().await;

        assert_ok!(result, "The result should be Ok");
    }
}
