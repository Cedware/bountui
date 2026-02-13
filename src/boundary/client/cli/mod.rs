mod command_runner;

use crate::boundary::client::cli::command_runner::Child;
use crate::boundary::client::cli::command_runner::{CommandRunner, DefaultCommandRunner};
use crate::boundary::client::response::{
    AuthenticateResponse, ErrorResponse, ItemResponse, ListResponse,
};
use crate::boundary::client::BoundaryConnectionHandle;
use crate::boundary::models::{ConnectResponse, Target};
use crate::boundary::Error::CliError;
use crate::boundary::{ApiClient, Error, Scope, Session};
use serde::Deserialize;
use std::net::TcpListener;
use std::process::{Output, Stdio};
use log::debug;
use serde::de::IgnoredAny;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::OnceCell;
use semver::Version;

/// Parse the Boundary CLI version from the `boundary version` command output.
/// Extracts the version string from "Version Number: X.Y.Z" format.
fn parse_boundary_version(output: &str) -> Result<Version, String> {
    for line in output.lines() {
        if let Some(version_str) = line.trim().strip_prefix("Version Number:") {
            return Version::parse(version_str.trim()).map_err(|e| {
                format!("invalid version '{}': {}", version_str.trim(), e)
            });
        }
    }
    Err("Version Number line not found in output".to_string())
}

#[derive(Clone)]
pub struct CliClient<R> {
    bin_path: String,
    command_runner: R,
    cached_version: Arc<OnceCell<Result<Version, String>>>,
}

impl Default for CliClient<DefaultCommandRunner> {
    fn default() -> Self {
        Self {
            bin_path: "boundary".to_string(),
            command_runner: DefaultCommandRunner,
            cached_version: Arc::new(OnceCell::new()),
        }
    }
}

impl<R> CliClient<R> {
    fn parse_success_response<'a, T: Deserialize<'a>>(
        &self,
        json: &'a [u8],
    ) -> Result<T, serde_json::Error> {
        let response_text = String::from_utf8_lossy(json);
        debug!("Response: {}", response_text);
        let response = serde_json::from_slice(json)?;
        Ok(response)
    }

    fn parse_error_response(&self, json: &[u8]) -> Result<Error, serde_json::Error> {
        let response_text = String::from_utf8_lossy(json);
        debug!("Response: {}", response_text);
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

impl<R> CliClient<R>
where
    R: CommandRunner + Send + Sync + 'static,
{
    async fn get_version(&self) -> Result<Version, Error> {
        self.cached_version
            .get_or_init(|| async {
                let mut command = tokio::process::Command::new(&self.bin_path);
                command.arg("version");
                match self.command_runner.output(&mut command).await {
                    Ok(output) if output.status.success() => {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        parse_boundary_version(&stdout).map_err(|e| e.to_string())
                    }
                    Ok(output) => Err(format!(
                        "Boundary version command failed with status {:?}: {}",
                        output.status.code(),
                        String::from_utf8_lossy(&output.stderr)
                    )),
                    Err(e) => Err(format!("Failed to run boundary version: {}", e)),
                }
            })
            .await
            .clone()
            .map_err(Error::VersionParseError)
    }
}

impl<R> ApiClient for CliClient<R>
where
    R: CommandRunner + Send + Sync + 'static,
    R::Child: BoundaryConnectionHandle + Send + Sync + 'static,
    <<R as CommandRunner>::Child as Child>::Stdout: Unpin + Send + Sync + 'static,
{
    type ConnectionHandle = R::Child;

    async fn get_scopes(&self, parent: Option<&str>, recursive: bool) -> Result<Vec<Scope>, Error> {
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

    async fn get_targets(&self, scope: Option<&str>) -> Result<Vec<Target>, Error> {
        let mut args = vec!["targets", "list", "-format", "json"];
        match scope {
            Some(scope) => {
                args.push("-scope-id");
                args.push(scope);
            }
            None => {
                args.push("-recursive");
            }
        }
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
        let scopes = self
            .get_scopes(None, true)
            .await?
            .into_iter()
            .filter(|s| {
                s.authorized_collection_actions
                    .get("sessions")
                    .map(|action| action.contains(&"list".to_string()))
                    .unwrap_or(false)
            })
            .collect::<Vec<_>>();
        let results = futures::future::join_all(scopes.iter().map(|scope| {
            let scope_id = &scope.id;
            self.get_sessions(scope_id)
        }))
        .await;
        let mut sessions = Vec::new();
        for result in results {
            match result {
                Ok(session_list) => {
                    sessions.append(
                        &mut session_list
                            .into_iter()
                            .filter(|s| s.user_id == user_id)
                            .collect::<Vec<_>>(),
                    );
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
    ) -> Result<(ConnectResponse, R::Child), Error> {
        //Check if the port is available
        TcpListener::bind(format!("127.0.0.1:{port}"))?;

        let port_str = port.to_string();
        let mut args = vec![
            "connect",
            "-target-id",
            target_id,
            "-listen-port",
            &port_str,
            "-format",
            "json",
        ];

        let version = self.get_version().await?;
        if version >= Version::new(0, 21, 0) {
            args.push("-inactive-timeout");
            args.push("-1");
        }

        let mut command = tokio::process::Command::new(&self.bin_path);
        let configured_command = command
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
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

        Ok((response, child))
    }

    async fn cancel_session(&self, session_id: &str) -> Result<(), Error> {
        let args = vec!["sessions", "cancel", "-id", session_id, "-format", "json"];
        let mut command = tokio::process::Command::new(&self.bin_path);
        let configured_command = command.args(&args);
        let output = self.command_runner.output(configured_command).await?;
        let _: IgnoredAny = self.get_result_from_output(&output)?;
        Ok(())
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
    use crate::boundary::client::cli::command_runner::{MockChild, MockCommandRunner};
    use crate::boundary::client::response::ListResponse;
    use crate::boundary::{ApiClient, CliClient, ConnectResponse, Error, Scope};
    use chrono::{TimeDelta, Utc};
    use mockall::predicate;
    use std::net::TcpListener;
    use std::ops::Add;
    use std::os::unix::process::ExitStatusExt;
    use std::process::Output;
    use std::sync::Arc;
    use tokio::io;
    use tokio_test::assert_ok;

    async fn create_output_result(
        status: i32,
        stdout: String,
        stderr: String,
    ) -> io::Result<Output> {
        Ok(Output {
            status: std::process::ExitStatus::from_raw(status),
            stdout: stdout.into_bytes(),
            stderr: stderr.into_bytes(),
        })
    }

    #[tokio::test]
    async fn test_get_scopes() {
        let mut command_runner = MockCommandRunner::new();

        let response = ListResponse {
            items: Some(vec![Scope::builder()
                .name("scope1".to_string())
                .id("scope1".to_string())
                .description("scope1".to_string())
                .type_name("scope".to_string())
                .authorized_collection_actions(std::collections::HashMap::new())
                .build()]),
        };
        let response_json = serde_json::to_string(&response).unwrap();

        command_runner
            .expect_output()
            .times(1)
            .with(predicate::always())
            .returning(move |_| {
                Box::pin(create_output_result(
                    0,
                    response_json.to_string(),
                    "".to_string(),
                ))
            });
        let client = CliClient {
            bin_path: "boundary".to_string(),
            command_runner,
            cached_version: Arc::new(tokio::sync::OnceCell::new()),
        };

        let scopes = client.get_scopes(None, false).await.unwrap();
        assert_eq!(scopes, response.items.unwrap());
    }

    #[tokio::test]
    async fn test_connect() {
        let mut command_runner = MockCommandRunner::new();

        let expected_response = ConnectResponse {
            credentials: vec![],
            session_id: "session_id".to_string(),
            expiration: Utc::now().add(TimeDelta::seconds(20)),
        };
        let response_json = serde_json::to_vec(&expected_response).unwrap();

        // Mock the version command output (version < 0.21.0, no inactive-timeout support)
        command_runner
            .expect_output()
            .times(1)
            .with(predicate::always())
            .returning(move |_| {
                Box::pin(create_output_result(
                    0,
                    "Version Number: 0.20.0\n".to_string(),
                    String::new(),
                ))
            });

        command_runner
            .expect_spawn()
            .times(1)
            .with(predicate::always())
            .returning(move |_| {
                let mut child = MockChild::new();
                let response_json = response_json.clone();
                child.expect_stdout().returning(move || {
                    Some(
                        tokio_test::io::Builder::new()
                            .read(&response_json.clone())
                            .build(),
                    )
                });
                Ok(child)
            });

        let sut = CliClient {
            bin_path: "boundary".to_string(),
            command_runner,
            cached_version: Arc::new(tokio::sync::OnceCell::new()),
        };

        let tcp_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = tcp_listener.local_addr().unwrap().port();
        let response = sut.connect("target_id", port).await;
        assert!(
            matches!(response, Err(Error::Io(_))),
            "connect did not return expected io error, while the port is already in use"
        );
        drop(tcp_listener);
        let result = sut.connect("target_id", port).await;
        println!("{:?}", response);
        assert_ok!(&result, "connect should return Ok");
        let (response, _) = result.unwrap();
        assert_eq!(
            response, expected_response,
            "The response should equal the expected response"
        );
    }

    #[tokio::test]
    async fn test_cancel_session_success() {
        let mut command_runner = MockCommandRunner::new();

        // JSON returned by boundary sessions cancel -format json
        let response_json = r#"{
   "status_code":200,
   "item":{
      "id":"id",
      "target_id":"target_id",
      "scope":{
         "id":"scope_id",
         "type":"project",
         "name":"scope name",
         "description":"scope_description",
         "parent_scope_id":"parent_scope_id"
      },
      "created_time":"2025-09-07T06:24:03.179388Z",
      "updated_time":"2025-09-07T06:24:26.346325Z",
      "version":2,
      "type":"tcp",
      "expiration_time":"2025-09-07T14:24:03.184663Z",
      "auth_token_id":"at_id",
      "user_id":"u_id",
      "host_set_id":"hsst_id",
      "host_id":"hst_id",
      "scope_id":"p_id",
      "endpoint":"tcp://endpoint:443",
      "states":[
         {
            "status":"canceling",
            "start_time":"2025-09-07T06:24:26.346325Z"
         },
         {
            "status":"pending",
            "start_time":"2025-09-07T06:24:03.179388Z",
            "end_time":"2025-09-07T06:24:26.346325Z"
         }
      ],
      "status":"canceling",
      "certificate":"certificate_content",
      "authorized_actions":[
         "cancel:self",
         "read:self"
      ]
   }
}"#;

        command_runner
            .expect_output()
            .times(1)
            .with(predicate::always())
            .returning(move |_| {
                Box::pin(create_output_result(0, response_json.to_string(), String::new()))
            });

        let client = CliClient {
            bin_path: "boundary".to_string(),
            command_runner,
            cached_version: Arc::new(tokio::sync::OnceCell::new()),
        };

        let result = client.cancel_session("id").await;
        assert_ok!(&result, "cancel_session should return Ok when JSON is valid");
    }

    #[tokio::test]
    async fn test_connect_with_inactive_timeout_support() {
        let mut command_runner = MockCommandRunner::new();

        let expected_response = ConnectResponse {
            credentials: vec![],
            session_id: "session_id".to_string(),
            expiration: Utc::now().add(TimeDelta::seconds(20)),
        };
        let response_json = serde_json::to_vec(&expected_response).unwrap();

        // Mock the version command output (version >= 0.21.0, supports inactive-timeout)
        command_runner
            .expect_output()
            .times(1)
            .with(predicate::always())
            .returning(move |_| {
                Box::pin(create_output_result(
                    0,
                    "Version Number: 0.21.0\n".to_string(),
                    String::new(),
                ))
            });

        command_runner
            .expect_spawn()
            .times(1)
            .with(predicate::always())
            .returning(move |_| {
                let mut child = MockChild::new();
                let response_json = response_json.clone();
                child.expect_stdout().returning(move || {
                    Some(
                        tokio_test::io::Builder::new()
                            .read(&response_json.clone())
                            .build(),
                    )
                });
                Ok(child)
            });

        let sut = CliClient {
            bin_path: "boundary".to_string(),
            command_runner,
            cached_version: Arc::new(tokio::sync::OnceCell::new()),
        };

        let tcp_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = tcp_listener.local_addr().unwrap().port();
        drop(tcp_listener);

        let result = sut.connect("target_id", port).await;
        assert_ok!(&result, "connect should return Ok with version >= 0.21.0");
        let (response, _) = result.unwrap();
        assert_eq!(
            response, expected_response,
            "The response should equal the expected response"
        );
    }

    mod parse_boundary_version_tests {
        use super::super::parse_boundary_version;
        use semver::Version;

        #[test]
        fn test_parse_valid_version() {
            let output = r#"Version information:
  Version Number: 0.21.0
  Git Revision:   abc123
"#;
            let version = parse_boundary_version(output);
            assert_eq!(version, Ok(Version::new(0, 21, 0)));
        }

        #[test]
        fn test_parse_version_with_extra_whitespace() {
            let output = "Version Number:   1.2.3  \n";
            let version = parse_boundary_version(output);
            assert_eq!(version, Ok(Version::new(1, 2, 3)));
        }

        #[test]
        fn test_parse_missing_version_line() {
            let output = "Some random output\nNo version here";
            let version = parse_boundary_version(output);
            assert!(version.is_err());
            assert!(version.unwrap_err().contains("Version Number line not found"));
        }

        #[test]
        fn test_parse_empty_output() {
            let version = parse_boundary_version("");
            assert!(version.is_err());
            assert!(version.unwrap_err().contains("Version Number line not found"));
        }

        #[test]
        fn test_parse_invalid_version_format() {
            let output = "Version Number: not-a-valid-version\n";
            let version = parse_boundary_version(output);
            assert!(version.is_err());
            assert!(version.unwrap_err().contains("invalid version"));
        }
    }
}
