use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("An error occurred while executing the command: {0}")]
    Io(#[from] std::io::Error),
    #[error("boundary cli returned an error code: {0:?}")]
    CliError(Option<i32>, String),
    #[error("{0}: {1}")]
    ApiError(u16, String),
    #[error("An error occurred while parsing JSON: {0}")]
    JsonError(#[from] serde_json::Error),
}
