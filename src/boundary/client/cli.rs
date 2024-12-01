use crate::boundary::client::response::{
    AuthenticateResponse, ErrorResponse, ItemResponse, ListResponse,
};
use crate::boundary::models::{ConnectResponse, Target};
use crate::boundary::Error::CliError;
use crate::boundary::{ApiClient, Error, Scope, Session};
use futures::{select, FutureExt};
use log::error;
use serde::Deserialize;
use std::process::Output;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio_util::sync::CancellationToken;
#[derive(Clone)]
pub struct CliClient {
    bin_path: String,
}

impl Default for CliClient {
    fn default() -> Self {
        Self {
            bin_path: "boundary".to_string(),
        }
    }
}

impl CliClient {
    fn parse_success_response<'a, T: Deserialize<'a>>(
        &self,
        json: &'a [u8],
    ) -> Result<T, serde_json::Error> {
        let response = serde_json::from_slice(json)?;
        Ok(response)
    }

    fn parse_error_response(&self, json: &[u8]) -> Result<Error, serde_json::Error> {
        let response: ErrorResponse = serde_json::from_slice(json)?;
        Ok(Error::ApiError(
            response.status_code,
            response.api_error.message,
        ))
    }

    fn get_result_from_output<'a, T>(&self, output: &'a Output) -> Result<T, Error>
    where
        T: Deserialize<'a>,
    {
        match output.status.code() {
            None => Err(CliError(
                None,
                String::from_utf8_lossy(&output.stderr).to_string(),
            )),
            Some(0) => Ok(self.parse_success_response(&output.stdout)?),
            Some(1) => Err(self.parse_error_response(&output.stderr)?),
            Some(c) => Err(CliError(
                Some(c),
                String::from_utf8_lossy(&output.stderr).to_string(),
            )),
        }
    }
}

impl ApiClient for CliClient {
    async fn get_scopes(&self, parent: &Option<String>) -> Result<Vec<Scope>, Error> {
        let mut args = vec!["scopes", "list", "-format", "json"];
        parent.iter().for_each(|p| {
            args.push("-scope-id");
            args.push(p);
        });
        let command = Command::new(&self.bin_path).args(&args).output().await?;
        let response = self.get_result_from_output(&command);
        response.map(|r: ListResponse<Scope>| r.items.unwrap_or_default())
    }

    async fn get_targets(&self, scope: &Option<String>) -> Result<Vec<Target>, Error> {
        let mut args = vec!["targets", "list", "-format", "json"];
        scope.iter().for_each(|s| {
            args.push("-scope-id");
            args.push(s);
        });
        let output = Command::new(&self.bin_path).args(&args).output().await?;
        let result = self.get_result_from_output(&output);
        result.map(|r: ListResponse<Target>| r.items.unwrap_or_default())
    }

    async fn get_sessions(&self, scope: &str) -> Result<Vec<Session>, Error> {
        let args = vec!["sessions", "list", "-scope-id", scope, "-format", "json"];
        let output = Command::new(&self.bin_path).args(&args).output();
        let result = self.get_result_from_output(&output.await?);
        result.map(|r: ListResponse<Session>| r.items.unwrap_or_default())
    }

    async fn connect(
        &self,
        target_id: &str,
        port: u16,
        cancellation_token: CancellationToken,
    ) -> Result<ConnectResponse, Error> {
        let mut child = Command::new(&self.bin_path)
            .args([
                "connect",
                "-target-id",
                target_id,
                "-listen-port",
                &port.to_string(),
                "-format",
                "json",
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        let stdout = child
            .stdout
            .take()
            .expect("This should never happen since we are piping stdout");
        let std_read = BufReader::new(stdout);

        let response = std_read
            .lines()
            .next_line()
            .await?
            .ok_or(CliError(None, "No response from boundary".to_string()))?;
        let response: ConnectResponse = serde_json::from_str(&response)?;

        tokio::spawn(async move {
            let mut child_future = Box::pin(child.wait()).fuse();
            select! {
                _ = cancellation_token.cancelled().fuse() => {
                    drop(child_future);
                    if let Err(e) = child.kill().await {
                        error!("Failed to kill child process: {}", e);
                    }
                },
                response = child_future => {
                    if let Err(e) = response {
                        error!("Failed to wait for child process: {}", e);
                    }
                }
            }
        });

        Ok(response)
    }

    async fn cancel_session(&self, session_id: &str) -> Result<Session, Error> {
        let args = vec!["sessions", "cancel", "-id", session_id, "-format", "json"];
        let command_output = Command::new(&self.bin_path).args(&args).output().await?;
        let result = self.get_result_from_output(&command_output);
        result.map(|r: ItemResponse<Session>| r.item)
    }

    async fn authenticate(&self) -> Result<AuthenticateResponse, Error> {
        let args = vec!["authenticate", "-format", "json"];
        let command = Command::new(&self.bin_path).args(&args).output().await?;
        let result = self.get_result_from_output(&command);
        result.map(|auth_resp: ItemResponse<AuthenticateResponse>| auth_resp.item)
    }
}
