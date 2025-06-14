use crate::boundary;
use log::error;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

#[derive(thiserror::Error, Debug)]
pub enum ConnectionError {
    #[error("Failed to update internal state")]
    UpdateStateError,
    #[error("Boundary error: {0}")]
    BoundaryError(#[from] boundary::Error),
}

pub struct ConnectionManager<C> {
    cancellation_tokens: Arc<Mutex<HashMap<String, CancellationToken>>>,
    boundary_client: C,
}

impl<C> ConnectionManager<C> {
    pub fn new(boundary_client: C) -> Self {
        ConnectionManager {
            cancellation_tokens: Arc::new(Mutex::new(HashMap::new())),
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
    {

        let cancellation_token = CancellationToken::new();
        let response = self
            .boundary_client
            .connect(&target_id, port, cancellation_token.clone())
            .await?;
        match self.cancellation_tokens.lock() {
            Ok(mut connections) => {
                connections.insert(response.session_id.clone(), cancellation_token);
            }
            Err(e) => error!("Error while acquiring lock to connections: {:?}", e),
        }
        Ok(response)
    }

    pub async fn stop(&self, id: &str) -> Result<(), ConnectionError>
    where
        C: boundary::ApiClient,
    {
        self.boundary_client.cancel_session(id).await?;
        let mut cancellation_tokens = self
            .cancellation_tokens
            .lock()
            .map_err(|_| ConnectionError::UpdateStateError)?;
        let cancellation_token = cancellation_tokens.remove(id);
        if let Some(cancellation_token) = cancellation_token {
            cancellation_token.cancel();
        }
        Ok(())
    }
}
