use crate::boundary;
use crate::bountui::Message;
use std::marker::PhantomData;

pub struct LoginPage<C: boundary::ApiClient + Clone + Send + Sync + 'static> {
    _client: PhantomData<C>,
}

impl<C> LoginPage<C>
where
    C: boundary::ApiClient + Clone + Send + Sync + 'static,
{
    pub fn new(boundary_client: C, message_tx: tokio::sync::mpsc::Sender<Message>) -> Self {
        tokio::spawn(async move {
            match boundary_client.authenticate().await {
                Ok(auth_response) => {
                    let _ = message_tx.send(Message::Authenticated(auth_response)).await;
                }
                Err(e) => {
                    log::error!("Authentication failed: {e}");
                    let _ = message_tx
                        .send(Message::ShowAlert(
                            "Authentication failed".to_string(),
                            format!("Authentication failed. Please try again.\nReason: {e}"),
                        ))
                        .await;
                }
            }
        });

        Self {
            _client: PhantomData,
        }
    }
}
