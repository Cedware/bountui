use std::fmt::{Debug, Display, Formatter};

pub struct StubError;

impl Debug for StubError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("a stub error occurred")
    }
}

impl Display for StubError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("a stub error occurred")
    }
}

impl std::error::Error for StubError {}