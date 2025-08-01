use crate::boundary;
use crate::boundary::BoundaryConnectionHandle;
use chrono::Utc;
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
    #[error("Failed to stop the connection: The cancellation token was not found")]
    StopFailedMissingToken,
    #[error("Failed to stop the connection: The join handle was not found")]
    StopFailedMissingJoinHandle,
}

pub struct ConnectionManager<C> {
    cancellation_tokens: Arc<Mutex<HashMap<String, CancellationToken>>>,
    join_handles: Arc<Mutex<HashMap<String, JoinHandle<()>>>>,
    boundary_client: C,
}

impl<C> ConnectionManager<C> {
    pub fn new(boundary_client: C) -> Self {
        ConnectionManager {
            cancellation_tokens: Arc::new(Mutex::new(HashMap::new())),
            join_handles: Arc::new(Mutex::new(HashMap::new())),
            boundary_client,
        }
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
        let cancellation_token = CancellationToken::new();
        let (response, mut connection_handle) =
            self.boundary_client.connect(&target_id, port).await?;
        {
            let cancellation_token = cancellation_token.clone();
            self.cancellation_tokens
                .lock()
                .unwrap()
                .insert(response.session_id.clone(), cancellation_token);
        }

        let session_id = response.session_id.clone();
        let expires_in = response.expiration - Utc::now();
        let cancellation_tokens = self.cancellation_tokens.clone();
        let join_handles = self.join_handles.clone();
        let session_expired_future = async move {
            match expires_in.to_std() {
                Ok(duration) => {
                    tokio::time::sleep(duration).await;
                }
                Err(e) => {
                    error!("Could not convert expiration time to duration: {:?}", e);
                    pending::<()>().await;
                }
            }
        };
        let join_handle = {
            let session_id = session_id.clone();
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
                    _ = session_expired_future => {
                        info!("Boundary session expired");
                        connection_handle.stop().await
                    },
                };
                if let Err(e) = stop_result {
                    error!("Connection handle was stopped with and error {:?}", e)
                }
                join_handles.lock().unwrap().remove(&session_id);
                cancellation_tokens.lock().unwrap().remove(&session_id);
            })
        };
        self.join_handles
            .lock()
            .unwrap()
            .insert(session_id.clone(), join_handle);

        Ok(response)
    }

    pub async fn stop(&self, id: &str) -> Result<(), ConnectionError>
    where
        C: boundary::ApiClient,
    {
        let join_handle = self
            .join_handles
            .lock()
            .unwrap()
            .remove(id)
            .ok_or(ConnectionError::StopFailedMissingJoinHandle)?;
        self.boundary_client.cancel_session(id).await?;
        let cancellation_token = self
            .cancellation_tokens
            .lock()
            .unwrap()
            .remove(id)
            .ok_or(ConnectionError::StopFailedMissingToken)?;
        cancellation_token.cancel();
        let _ = join_handle.await; //Even when the task failed the stop is considered successful
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
    ) -> MockApiClient {
        let connect_response = ConnectResponse {
            session_id: "session-id".to_owned(),
            credentials: vec![],
            expiration: Utc::now().add(session_life_time),
        };
        let mut boundary_client = MockApiClient::new();
        boundary_client
            .expect_connect()
            .return_once(move |_, _| Ok((connect_response, mock_connection_handle)));

        boundary_client
            .expect_cancel_session()
            .returning(|_| Ok(()));
        boundary_client
    }

    #[tokio::test(start_paused = true)]
    async fn test_connection_is_closed_after_sessions_is_expired() {
        let session_life_time = TimeDelta::seconds(10);
        let expected_stop_invocations = 1;

        let connection_handle = configure_connection_handle(expected_stop_invocations);
        let boundary_client =
            configure_boundary_client(session_life_time, connection_handle.clone());
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
            configure_boundary_client(session_life_time, connection_handle.clone());
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
            configure_boundary_client(session_life_time, connection_handle.clone());
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
