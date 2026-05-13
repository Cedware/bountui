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
use log::debug;
use semver::Version;
use serde::de::IgnoredAny;
use serde::Deserialize;
use std::net::TcpListener;
use std::process::{Output, Stdio};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::OnceCell;

const CONNECT_TIMEOUT_MS: i32 = 5000;

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
        // Check if the port is available
        TcpListener::bind(format!("127.0.0.1:{port}"))
            .map_err(|_| Error::PortNotAvailable(port))?;

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

        let mut response_lines = std_read
            .lines();

        let a = tokio::time::timeout(std::time::Duration::from_millis(CONNECT_TIMEOUT_MS as u64), response_lines.next_line())
            .await;

        let response = a.map_err(|_e| Error::ConnectTimeoutError)??
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
    use crate::boundary::client::cli::command_runner::mock::{MockChild, MockCommandRunner};
    use crate::boundary::client::response::ListResponse;
    use crate::boundary::{ApiClient, CliClient, ConnectResponse, Error, Scope};
    use chrono::{TimeDelta, Utc};
    use std::net::TcpListener;
    use std::ops::Add;
    use std::sync::Arc;
    use tokio_test::assert_ok;
    use tokio_test::io::Builder;

    #[tokio::test]
    async fn test_get_scopes() {


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

        let std_out = Builder::new().read(response_json.as_bytes()).build();
        let mock_result = MockChild::new(Ok(0), Some(std_out));
        let command_runner = MockCommandRunner::new(vec![mock_result].into());

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
        let expected_response = ConnectResponse {
            credentials: vec![],
            session_id: "session_id".to_string(),
            expiration: Utc::now().add(TimeDelta::seconds(20)),
        };
        let response_json = serde_json::to_string(&expected_response).unwrap();
        let std_out = Builder::new().read(response_json.as_bytes()).build();
        let command_runner = MockCommandRunner::new(vec![
            MockChild::new(Ok(0), Some(Builder::new().read("Version Number: 0.20.0\n".to_string().as_bytes()).build())),
            MockChild::new(Ok(0), Some(std_out))
        ].into());


        let sut = CliClient {
            bin_path: "boundary".to_string(),
            command_runner,
            cached_version: Arc::new(tokio::sync::OnceCell::new()),
        };

        let tcp_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = tcp_listener.local_addr().unwrap().port();
        let response = sut.connect("target_id", port).await;
        assert!(
            matches!(response, Err(Error::PortNotAvailable(p)) if p == port),
            "connect did not return PortNotAvailable error while the port is already in use"
        );
        drop(tcp_listener);
        let result = sut.connect("target_id", port).await;
        assert_ok!(&result, "connect should return Ok");
        let (response, _) = result.unwrap();
        assert_eq!(
            response, expected_response,
            "The response should equal the expected response"
        );
    }

    #[tokio::test]
    async fn test_cancel_session_success() {

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

        let child = MockChild::new(Ok(0), Some(Builder::new().read(response_json.as_bytes()).build()));
        let command_runner = MockCommandRunner::new(vec![child].into());

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

        let expected_response = ConnectResponse {
            credentials: vec![],
            session_id: "session_id".to_string(),
            expiration: Utc::now().add(TimeDelta::seconds(20)),
        };
        let response_json = serde_json::to_vec(&expected_response).unwrap();

        let version_number_child = MockChild::new(Ok(0), Some(Builder::new().read("Version Number: 0.21.0\n".as_bytes()).build()));
        let connect_child = MockChild::new(Ok(0), Some(Builder::new().read(&response_json).build()));
        let command_runner = MockCommandRunner::new(vec![version_number_child, connect_child].into());

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
        use crate::boundary;
        use crate::boundary::client::cli::command_runner::mock::{MockChild, MockCommandRunner};
        use crate::boundary::client::cli::CONNECT_TIMEOUT_MS;
        use crate::boundary::{ApiClient, CliClient};
        use semver::Version;
        use std::net::TcpListener;
        use std::sync::Arc;
        use tokio_test::io::Builder;

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

        #[tokio::test(start_paused = true)]
        async fn test_connect_should_fail_when_boundary_does_not_connect_in_time() {
            let std_out = Builder::new()
                .wait(std::time::Duration::from_millis((CONNECT_TIMEOUT_MS + 1000) as u64)).build();

            let command_runner = MockCommandRunner::new(vec![
                MockChild::new(Ok(0), Some(Builder::new().read("Version Number: 0.20.0\n".to_string().as_bytes()).build())),
                MockChild::new(Ok(0), Some(std_out))
            ].into());


            let sut = CliClient {
                bin_path: "boundary".to_string(),
                command_runner,
                cached_version: Arc::new(tokio::sync::OnceCell::new()),
            };

            let tcp_listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let port = tcp_listener.local_addr().unwrap().port();
            drop(tcp_listener);

            let result = sut.connect("target_id", port).await;
            match result {
                Ok(_) => panic!("connect should have failed due to timeout, but it succeeded"),
                Err(boundary::Error::ConnectTimeoutError { .. }) => {},
                Err(e) => panic!("connect should fail with ConnectTimeoutError but it failed with {}", e),
            }
        }
    }
}
