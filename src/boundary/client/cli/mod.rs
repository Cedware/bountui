mod command_runner;

use std::net::TcpListener;
use crate::boundary::client::cli::command_runner::Child;
use crate::boundary::client::response::{
    AuthenticateResponse, ErrorResponse, ItemResponse, ListResponse,
};
use crate::boundary::models::{ConnectResponse, Target};
use crate::boundary::Error::CliError;
use crate::boundary::{ApiClient, Error, Scope, Session};
use futures::{select, FutureExt};
use log::error;
use serde::Deserialize;
use std::process::{Output, Stdio};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_util::sync::CancellationToken;
use crate::boundary::client::cli::command_runner::{CommandRunner, DefaultCommandRunner};

#[derive(Clone)]
pub struct CliClient<R> {
    bin_path: String,
    command_runner: R
}



impl Default for CliClient<DefaultCommandRunner> {
    fn default() -> Self {
        Self {
            bin_path: "boundary".to_string(),
            command_runner: DefaultCommandRunner
        }
    }
}

impl <R> CliClient<R> {
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

impl <R> ApiClient for CliClient<R> where R: CommandRunner + Send + Sync + 'static, R::Child: Send + Sync + 'static, <<R as CommandRunner>::Child as Child>::Stdout : Unpin + Send + Sync + 'static {
    async fn get_scopes(&self, parent: &Option<String>, recursive: bool) -> Result<Vec<Scope>, Error> {
        let mut args = vec!["scopes", "list", "-format", "json"];
        parent.iter().for_each(|p| {
            args.push("-scope-id");
            args.push(p);
        });
        if recursive {
            args.push("-recursive");
        }
        let mut command = tokio::process::Command::new(&self.bin_path);
        let configured_command = command.args(&args);
        let output = self.command_runner.output(configured_command).await?;
        let response = self.get_result_from_output(&output);
        response.map(|r: ListResponse<Scope>| r.items.unwrap_or_default())
    }

    async fn get_targets(&self, scope: &Option<String>) -> Result<Vec<Target>, Error> {
        let mut args = vec!["targets", "list", "-format", "json"];
        scope.iter().for_each(|s| {
            args.push("-scope-id");
            args.push(s);
        });
        let mut command = tokio::process::Command::new(&self.bin_path);
        let configured_command = command.args(&args);
        let output = self.command_runner.output(configured_command).await?;
        let result = self.get_result_from_output(&output);
        result.map(|r: ListResponse<Target>| r.items.unwrap_or_default())
    }

    async fn get_sessions(&self, scope: &str) -> Result<Vec<Session>, Error> {
        let args = vec!["sessions", "list", "-scope-id", scope, "-format", "json"];
        let mut command = tokio::process::Command::new(&self.bin_path);
        let configured_command = command.args(&args);
        let output = self.command_runner.output(configured_command).await?;
        let result = self.get_result_from_output(&output);
        result.map(|r: ListResponse<Session>| r.items.unwrap_or_default())
    }

    async fn get_user_sessions(&self, user_id: &str) -> Result<Vec<Session>, Error> {
        let scopes = self.get_scopes(&None, true).await?
            .into_iter().filter(|s| s.authorized_collection_actions.get("sessions").map(|action| action.contains(&"list".to_string())).unwrap_or(false))
            .collect::<Vec<_>>();
        let results = futures::future::join_all(
            scopes.iter().map(|scope| {
                let scope_id = &scope.id;
                self.get_sessions(scope_id)
            })
        ).await;
        let mut sessions = Vec::new();
        for result in results {
            match result {
                Ok(session_list) => {
                    sessions.append(&mut session_list.into_iter().filter(|s| s.user_id == user_id).collect::<Vec<_>>());
                }
                Err(e) => return Err(e),
            }
        }
        Ok(sessions)
    }

    async fn connect(
        &self,
        target_id: &str,
        port: u16,
        cancellation_token: CancellationToken,
    ) -> Result<ConnectResponse, Error> {

        //Check if the port is available
        TcpListener::bind(format!("127.0.0.1:{port}"))?;


        let mut command = tokio::process::Command::new(&self.bin_path);
        let configured_command = command.args([
            "connect",
            "-target-id",
            target_id,
            "-listen-port",
            &port.to_string(),
            "-format",
            "json",
        ]).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = self.command_runner.spawn(configured_command)?;

        let stdout = child
            .stdout()
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
        let mut command = tokio::process::Command::new(&self.bin_path);
        let configured_command = command.args(&args);
        let output = self.command_runner.output(configured_command).await?;
        let result = self.get_result_from_output(&output);
        result.map(|r: ItemResponse<Session>| r.item)
    }

    async fn authenticate(&self) -> Result<AuthenticateResponse, Error> {
        let args = vec!["authenticate", "-format", "json"];
        let mut command = tokio::process::Command::new(&self.bin_path);
        let configured_command = command.args(&args);
        let output = self.command_runner.output(configured_command).await?;
        let result = self.get_result_from_output(&output);
        result.map(|auth_resp: ItemResponse<AuthenticateResponse>| auth_resp.item)
    }
}


#[cfg(test)]
mod test {
    use std::net::TcpListener;
    use std::os::unix::process::ExitStatusExt;
    use std::process::Output;
    use mockall::predicate;
    use tokio::io;
    use crate::boundary::{ApiClient, CliClient, ConnectResponse, Error, Scope};
    use crate::boundary::client::cli::command_runner::{MockChild, MockCommandRunner};
    use crate::boundary::client::response::ListResponse;

    async fn create_output_result(status: i32, stdout: String, stderr: String) -> io::Result<Output> {
        Ok(Output {
            status: std::process::ExitStatus::from_raw(status),
            stdout: stdout.into_bytes(),
            stderr: stderr.into_bytes()
        })
    }

    #[tokio::test]
    async fn test_get_scopes() {
        let mut command_runner = MockCommandRunner::new();

        let response = ListResponse {
            items: Some(vec![
                Scope::builder()
                    .name("scope1".to_string())
                    .id("scope1".to_string())
                    .description("scope1".to_string())
                    .type_name("scope".to_string())
                    .authorized_collection_actions(std::collections::HashMap::new())
                    .build()
            ])
        };
        let response_json = serde_json::to_string(&response).unwrap();

        command_runner.expect_output()
            .times(1)
            .with(predicate::always())
            .returning(move |_| {
                Box::pin(create_output_result(0, response_json.to_string(), "".to_string()))
            });
        let client = CliClient {
            bin_path: "boundary".to_string(),
            command_runner
        };

        let scopes = client.get_scopes(&None, false).await.unwrap();
        assert_eq!(scopes, response.items.unwrap());
    }

    #[tokio::test]
    async fn test_connect() {
        let mut command_runner = MockCommandRunner::new();

        let expected_response = ConnectResponse {
            credentials: vec![],
            session_id: "session_id".to_string(),
        };
        let response_json = serde_json::to_vec(&expected_response).unwrap();

        command_runner.expect_spawn()
            .times(1)
            .with(predicate::always())
            .returning(move |_| {
                let mut child = MockChild::new();
                let response_json = response_json.clone();
                child.expect_stdout()
                    .returning(move || Some(tokio_test::io::Builder::new().read(&response_json.clone()).build()));
                Ok(child)
            });

        let sut = CliClient {
            bin_path: "boundary".to_string(),
            command_runner
        };

        let tcp_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = tcp_listener.local_addr().unwrap().port();
        let response = sut.connect("target_id", port, tokio_util::sync::CancellationToken::new()).await;
        assert!(matches!(response, Err(Error::Io(_))), "connect did not return expected io error, while the port is already in use");
        drop(tcp_listener);
        let response = sut.connect("target_id", port, tokio_util::sync::CancellationToken::new()).await;println!("{:?}", response);
        assert!(matches!(response, Ok(r) if r == expected_response), "Connect did not return the expected response");

    }

}