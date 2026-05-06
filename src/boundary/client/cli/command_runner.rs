use crate::boundary::BoundaryConnectionHandle;
use std::future::Future;
use std::process::{ExitStatus, Output};
use tokio::io;
use tokio::process::Command;

pub trait Child {
    type Stdout: io::AsyncRead;
    fn stdout(&mut self) -> Option<Self::Stdout>;
    fn wait(&mut self) -> impl Future<Output = io::Result<ExitStatus>> + Send;
    fn kill(&mut self) -> impl Future<Output = io::Result<()>> + Send;
}

impl<T> BoundaryConnectionHandle for T
where
    T: Child + Send,
{
    type Error = io::Error;

    async fn wait(&mut self) -> Result<(), Self::Error> {
        <T as Child>::wait(self).await?;
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), Self::Error> {
        <T as Child>::kill(self).await?;
        Ok(())
    }
}

impl Child for tokio::process::Child {
    type Stdout = tokio::process::ChildStdout;

    fn stdout(&mut self) -> Option<Self::Stdout> {
        self.stdout.take()
    }

    fn wait(&mut self) -> impl Future<Output = io::Result<ExitStatus>> {
        self.wait()
    }

    fn kill(&mut self) -> impl Future<Output = io::Result<()>> {
        self.kill()
    }
}

pub trait CommandRunner: Send + Sync + 'static {
    type Child: Child;
    fn output(
        &self,
        command: &mut Command,
    ) -> impl Future<Output = io::Result<Output>> + Send + Sync;
    fn spawn(&self, command: &mut Command) -> io::Result<Self::Child>;
}

#[derive(Copy, Clone)]
pub struct DefaultCommandRunner;

impl CommandRunner for DefaultCommandRunner {
    type Child = tokio::process::Child;

    async fn output(&self, command: &mut Command) -> io::Result<Output> {
        command.output().await
    }

    fn spawn(&self, command: &mut Command) -> io::Result<tokio::process::Child> {
        command.spawn()
    }
}


#[cfg(test)]
pub mod mock {
    use crate::boundary::client::cli::command_runner::{Child, CommandRunner};
    use std::collections::VecDeque;
    use std::future::Future;
    #[cfg(unix)]
    use std::os::unix::process::ExitStatusExt;
    #[cfg(windows)]
    use std::os::windows::process::ExitStatusExt;
    use std::process::{ExitStatus, Output};
    use std::sync::Mutex;
    use tokio::io::AsyncReadExt;
    use tokio::process::Command;


    pub struct MockChild {
        status: Option<std::io::Result<ExitStatus>>,
        stdout: Option<tokio_test::io::Mock>,
    }

    impl MockChild {
        pub fn new(status: std::io::Result<i32>, stdout: Option<tokio_test::io::Mock>) -> Self {
            Self {
                status: Some(status.map(|code| ExitStatus::from_raw(code))),
                stdout,
            }
        }
    }

    impl Child for MockChild
    where
    {
        type Stdout = tokio_test::io::Mock;

        fn stdout(&mut self) -> Option<Self::Stdout> {
            self.stdout.take()
        }

        async fn wait(&mut self) -> std::io::Result<ExitStatus> {
            self.status.take().expect("wait called more than once")
        }

        fn kill(&mut self) -> impl Future<Output=std::io::Result<()>> + Send {
            async { Ok(()) }
        }
    }


    pub struct MockCommandRunner {
        commands: Mutex<VecDeque<MockChild>>,
    }

    impl MockCommandRunner {
        pub fn new(commds: VecDeque<MockChild>) -> Self {
            Self {
                commands: Mutex::new(commds)
            }
        }
    }

    impl CommandRunner for MockCommandRunner {
        type Child = MockChild;

        async fn output(&self, _command: &mut Command) -> std::io::Result<Output> {
            let mut child = self.commands.lock().expect("Failed to lock commands mutex").remove(0).expect("command not found");
            let stdout = match child.stdout() {
                Some(mut s) => {
                    let mut buf = Vec::new();
                    s.read_to_end(&mut buf).await?;
                    buf
                },
                None => Vec::new(),
            };
            let status = child.wait().await?;

            Ok(Output {
                status,
                stdout,
                stderr: Vec::new(),
            })
        }

        fn spawn(&self, _command: &mut Command) -> std::io::Result<Self::Child> {
            Ok(self.commands.lock().expect("Failed to lock commands mutex").remove(0).expect("command not found"))
        }
    }
}