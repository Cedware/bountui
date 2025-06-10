use std::future::Future;
use std::process::{ExitStatus, Output};
use mockall::automock;
use tokio::io;
use tokio::process::{Command};


#[automock(type Stdout = tokio_test::io::Mock;)]
pub trait Child{

    type Stdout: io::AsyncRead;
    fn stdout(&mut self) -> Option<Self::Stdout>;
    fn wait(&mut self) -> impl Future<Output=io::Result<ExitStatus>> + Send;
    fn kill(&mut self) -> impl Future<Output=io::Result<()>> + Send;

}

impl Child for tokio::process::Child {

    type Stdout = tokio::process::ChildStdout;

    fn stdout(&mut self) -> Option<Self::Stdout> {
        self.stdout.take()
    }

    fn wait(&mut self) -> impl Future<Output=io::Result<ExitStatus>> {
        self.wait()
    }

    fn kill(&mut self) -> impl Future<Output=io::Result<()>> {
        self.kill()
    }
}

#[automock(type Child = MockChild;)]
pub trait CommandRunner: Send + Sync + 'static {
    type Child: Child;
    fn output(&self, command: &mut Command) -> impl Future<Output=io::Result<Output>> + Send + Sync;
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