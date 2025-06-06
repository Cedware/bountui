pub mod cli;
mod response;

use crate::boundary::client::response::AuthenticateResponse;
use crate::boundary::error::Error;
use crate::boundary::models::{ConnectResponse, Target};
use crate::boundary::{Scope, Session};
use std::future::Future;
use mockall::automock;
use tokio_util::sync::CancellationToken;

#[automock]
pub trait ApiClient {
    async fn get_scopes(
        &self,
        parent: &Option<String>,
        recursive: bool,
    ) -> Result<Vec<Scope>, Error>;
    async fn get_targets(
        &self,
        scope: &Option<String>,
    ) -> Result<Vec<Target>, Error>;

    fn get_sessions(&self, scope: &str) -> impl Future<Output = Result<Vec<Session>, Error>> + Send + Sync;

    #[warn(dead_code)]
    fn get_user_sessions(
        &self,
        user_id: &str,
    ) -> impl Future<Output = Result<Vec<Session>, Error>> + Send + Sync;

    async fn connect(
        &self,
        target_id: &str,
        port: u16,
        cancellation_token: CancellationToken,
    ) -> Result<ConnectResponse, Error>;

    async fn cancel_session(&self, session_id: &str) -> Result<Session, Error>;

    async fn authenticate(&self) -> Result<AuthenticateResponse, Error>;

}
