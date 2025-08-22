pub mod cli;
mod response;

use crate::boundary::client::response::AuthenticateResponse;
use crate::boundary::error::Error;
use crate::boundary::models::{ConnectResponse, SessionWithTarget, Target};
use crate::boundary::{Scope, Session};
use std::future::Future;
use std::sync::Arc;
use tokio::sync::Mutex;

#[cfg_attr(test, mockall::automock(type ConnectionHandle = Arc<tokio::sync::Mutex<MockBoundaryConnectionHandle>>;
))]
pub trait ApiClient {
    type ConnectionHandle: BoundaryConnectionHandle;

    fn get_scopes<'a>(
        &self,
        parent: Option<&'a str>,
        recursive: bool,
    ) -> impl Future<Output=Result<Vec<Scope>, Error>> + Send;
    fn get_targets<'a>(
        &self,
        scope: Option<&'a str>,
    ) -> impl Future<Output=Result<Vec<Target>, Error>> + Send;

    fn get_sessions(
        &self,
        scope: &str,
    ) -> impl Future<Output=Result<Vec<Session>, Error>> + Send + Sync;

    #[warn(dead_code)]
    fn get_user_sessions(
        &self,
        user_id: &str,
    ) -> impl Future<Output=Result<Vec<Session>, Error>> + Send + Sync;

    async fn connect(
        &self,
        target_id: &str,
        port: u16,
    ) -> Result<(ConnectResponse, Self::ConnectionHandle), Error>;

    async fn cancel_session(&self, session_id: &str) -> Result<(), Error>;

    async fn authenticate(&self) -> Result<AuthenticateResponse, Error>;
}

pub trait ApiClientExt: ApiClient + Sync {
    fn combine_sessions_with_target(
        sessions: Vec<Session>,
        targets: Vec<Target>,
    ) -> Vec<SessionWithTarget> {
        sessions
            .into_iter()
            .map(|s| {
                let target = targets.iter().find(|t| s.target_id == t.id).cloned();
                target.map(|t| SessionWithTarget::new(s, t))
            })
            .flatten()
            .collect()
    }

    fn get_sessions_with_target(
        &self,
        scope: &str,
    ) -> impl Future<Output=Result<Vec<SessionWithTarget>, Error>> + Send {
        async {
            let targets = self.get_targets(Some(scope)).await?;
            let sessions = self.get_sessions(scope).await?;
            Ok(Self::combine_sessions_with_target(sessions, targets))
        }
    }

    fn get_user_sessions_with_target(
        &self,
        user_id: &str,
    ) -> impl Future<Output=Result<Vec<SessionWithTarget>, Error>> + Send {
        async {
            let targets = self.get_targets(None).await?;
            let user_sessions = self.get_user_sessions(user_id).await?;
            Ok(Self::combine_sessions_with_target(user_sessions, targets))
        }
    }
}

impl<T: ApiClient + Sync> ApiClientExt for T {}

#[cfg_attr(test, mockall::automock(type Error = crate::mock::StubError;))]
pub trait BoundaryConnectionHandle: Send {
    type Error: std::error::Error + Send;

    fn wait(&mut self) -> impl Future<Output=Result<(), Self::Error>> + Send;
    fn stop(&mut self) -> impl Future<Output=Result<(), Self::Error>> + Send;
}

impl<T> BoundaryConnectionHandle for Arc<Mutex<T>>
where
    T: BoundaryConnectionHandle,
{
    type Error = T::Error;

    async fn wait(&mut self) -> Result<(), Self::Error> {
        self.lock().await.wait().await
    }

    async fn stop(&mut self) -> Result<(), Self::Error> {
        self.lock().await.stop().await
    }
}


impl<T: ApiClient> ApiClient for Arc<T> {

    type ConnectionHandle = T::ConnectionHandle;

    fn get_scopes(&self, parent: Option<&str>, recursive: bool) -> impl Future<Output=Result<Vec<Scope>, Error>> + Send {
        T::get_scopes(self, parent, recursive)
    }

    fn get_targets(&self, scope: Option<&str>) -> impl Future<Output=Result<Vec<Target>, Error>> + Send {
        T::get_targets(self, scope)
    }

    fn get_sessions(&self, scope: &str) -> impl Future<Output=Result<Vec<Session>, Error>> + Send + Sync {
        T::get_sessions(self, scope)
    }

    fn get_user_sessions(&self, user_id: &str) -> impl Future<Output=Result<Vec<Session>, Error>> + Send + Sync {
        T::get_user_sessions(self, user_id)
    }

    async fn connect(&self, target_id: &str, port: u16) -> Result<(ConnectResponse, Self::ConnectionHandle), Error> {
        T::connect(self, target_id, port).await
    }

    async fn cancel_session(&self, session_id: &str) -> Result<(), Error> {
        T::cancel_session(self, session_id).await
    }

    async fn authenticate(&self) -> Result<AuthenticateResponse, Error> {
        T::authenticate(self).await
    }
}