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
    fn get_scopes(
        &self,
        parent: &Option<String>,
    ) -> impl Future<Output = Result<Vec<Scope>, Error>>;
    fn get_targets(
        &self,
        scope: &Option<String>,
    ) -> impl Future<Output = Result<Vec<Target>, Error>>;
    
    fn get_sessions(&self, scope: &str) -> impl Future<Output = Result<Vec<Session>, Error>>;

    fn connect(
        &self,
        target_id: &str,
        port: u16,
        cancellation_token: CancellationToken,
    ) -> impl Future<Output = Result<ConnectResponse, Error>>;

    fn cancel_session(&self, session_id: &str) -> impl Future<Output = Result<Session, Error>>;

    fn authenticate(&self) -> impl Future<Output = Result<AuthenticateResponse, Error>>;
}
