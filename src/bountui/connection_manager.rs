use crate::boundary;
use crate::boundary::BoundaryConnectionHandle;
use chrono::{DateTime, Utc};
use log::{error, info};
use std::collections::HashMap;
use std::future::pending;
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

pub struct ConnectionManager<C> {
    connections: Arc<Mutex<HashMap<String, ConnectionEntry>>>,
    boundary_client: C,
}

impl<C> ConnectionManager<C> {
    pub fn new(boundary_client: C) -> Self {
        ConnectionManager {
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

    pub async fn connect(
        &self,
        target_id: &str,
        port: u16,
    ) -> Result<boundary::ConnectResponse, boundary::Error>
    where
        C: boundary::ApiClient,
        C::ConnectionHandle: 'static,
    {
        let (response, connection_handle) =
            self.boundary_client.connect(&target_id, port).await?;
        let cancellation_token = CancellationToken::new();
        let join_handle = Self::spawn_connection_task(self.connections.clone(), connection_handle, cancellation_token.clone(), response.expiration, response.session_id.clone());
        self.connections.lock().unwrap().insert(response.session_id.clone(), ConnectionEntry { cancellation_token, join_handle });
        Ok(response)
    }

    pub async fn stop(&self, id: &str) -> Result<(), ConnectionError>
    where
        C: boundary::ApiClient,
    {
        self.boundary_client.cancel_session(id).await?;
        let connection_entry = self.connections.lock().unwrap()
            .remove(id)
            .ok_or(ConnectionError::StopFailedUnknownSessionId(id.to_string()))?;
        connection_entry.cancellation_token.cancel();
        let _ = connection_entry.join_handle.await; //Even when the task failed the stop is considered successful
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::boundary::client::{MockApiClient, MockBoundaryConnectionHandle};
    use crate::boundary::ConnectResponse;
    use crate::bountui::connection_manager::ConnectionManager;
    use chrono::{TimeDelta, Utc};
    use futures::FutureExt;
    use mockall::predicate::eq;
    use std::future::pending;
    use std::ops::Add;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex;

    fn configure_connection_handle<R: Into<mockall::TimesRange>>(
        expected_stop_invocations: R,
    ) -> Arc<Mutex<MockBoundaryConnectionHandle>> {
        let mut connection_handle = MockBoundaryConnectionHandle::new();
        connection_handle
            .expect_wait()
            .times(1)
            .returning(|| pending().boxed());

        connection_handle
            .expect_stop()
            .times(expected_stop_invocations)
            .returning(|| async { Ok(()) }.boxed());
        Arc::new(Mutex::new(connection_handle))
    }

    fn configure_boundary_client(
        session_life_time: TimeDelta,
        mock_connection_handle: Arc<Mutex<MockBoundaryConnectionHandle>>,
        expect_cancellation: bool,
    ) -> MockApiClient {
        let session_id = "session-id";
        let connect_response = ConnectResponse {
            session_id: session_id.to_string(),
            credentials: vec![],
            expiration: Utc::now().add(session_life_time),
        };
        let mut boundary_client = MockApiClient::new();
        boundary_client
            .expect_connect()
            .return_once(move |_, _| Ok((connect_response, mock_connection_handle)));


        if (expect_cancellation) {
            boundary_client
                .expect_cancel_session()
                .times(1)
                .with(eq(session_id))
                .returning(|_| Ok(()));
        }

        boundary_client
    }

    #[tokio::test(start_paused = true)]
    async fn test_connection_is_closed_after_sessions_is_expired() {
        let session_life_time = TimeDelta::seconds(10);
        let expected_stop_invocations = 1;

        let connection_handle = configure_connection_handle(expected_stop_invocations);
        let boundary_client =
            configure_boundary_client(session_life_time, connection_handle.clone(), false);
        let sut = ConnectionManager::new(boundary_client);
        sut.connect("target_id", 8080).await.unwrap();
        tokio::time::sleep(Duration::from_secs(11)).await;
        connection_handle.lock().await.checkpoint();
    }

    #[tokio::test(start_paused = true)]
    async fn test_connection_is_not_closed_before_session_is_expired() {
        let expected_stop_invocations = 0;
        let connection_handle = configure_connection_handle(expected_stop_invocations);
        let session_life_time = TimeDelta::seconds(10);
        let boundary_client =
            configure_boundary_client(session_life_time, connection_handle.clone(), false);
        let sut = ConnectionManager::new(boundary_client);
        sut.connect("target_id", 8080).await.unwrap();
        tokio::time::sleep(Duration::from_secs(5)).await;
        connection_handle.lock().await.checkpoint();
    }

    #[tokio::test(start_paused = true)]
    async fn test_stop_session() {
        let expected_stop_invocations = 1;
        let connection_handle = configure_connection_handle(expected_stop_invocations);
        let session_life_time = TimeDelta::hours(8);
        let boundary_client =
            configure_boundary_client(session_life_time, connection_handle.clone(), true);
        let sut = ConnectionManager::new(boundary_client);
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
}
