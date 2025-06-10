pub mod cli;
mod response;

use crate::boundary::client::response::AuthenticateResponse;
use crate::boundary::error::Error;
use crate::boundary::models::{ConnectResponse, SessionWithTarget, Target};
use crate::boundary::{Scope, Session};
use std::future::Future;
use mockall::automock;
use tokio_util::sync::CancellationToken;

#[automock]
pub trait ApiClient {
    async fn get_scopes<'a>(
        &self,
        parent: Option<&'a str>,
        recursive: bool,
    ) -> Result<Vec<Scope>, Error>;
    fn get_targets<'a>(
        &self,
        scope: Option<&'a str>,
    ) -> impl Future<Output = Result<Vec<Target>, Error>> + Send;

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



pub trait ApiClientExt: ApiClient + Sync {

    fn combine_sessions_with_target(sessions: Vec<Session>, targets: Vec<Target>) -> Vec<SessionWithTarget> {
        sessions.into_iter().map(|s| {
            let target = targets.iter().find(|t| s.target_id == t.id).cloned();
            target.map(|t| SessionWithTarget::new(s, t))
        }).flatten().collect()
    }

    fn get_sessions_with_target(&self, scope: &str) -> impl Future<Output = Result<Vec<SessionWithTarget>, Error>> + Send {
        async {
            let targets = self.get_targets(Some(scope)).await?;
            let sessions = self.get_sessions(scope).await?;
            Ok(Self::combine_sessions_with_target(sessions, targets))
        }
    }

    fn get_user_sessions_with_target(&self, user_id: &str) -> impl Future<Output = Result<Vec<SessionWithTarget>, Error>> + Send {
        async {
            let targets = self.get_targets(None).await?;
            let user_sessions = self.get_user_sessions(user_id).await?;
            Ok(Self::combine_sessions_with_target(user_sessions, targets))
        }
    }
}

impl <T: ApiClient + Sync> ApiClientExt for T {}