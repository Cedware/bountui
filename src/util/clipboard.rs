use thiserror::Error;
#[derive(Debug, Error, Clone)]
pub enum ClipboardAccessError {

    #[error("The clipboard contents were not available in the requested format or the clipboard is empty.")]
    ContentNotAvailable,
    #[error("The selected clipboard is not supported with the current system configuration.")]
    ClipboardNotSupported,
    #[error("The native clipboard is not accessible due to being held by an other party.")]
    ClipboardOccupied,
    #[error("The image or the text that was about the be transferred to/from the clipboard could not be converted to the appropriate format.")]
    ConversionFailure,
    #[error("Unknown error while interacting with the clipboard: {0}")]
    Unknown(String),
}

impl From<arboard::Error> for ClipboardAccessError {
    fn from(value: arboard::Error) -> Self {
        match value {
            arboard::Error::ContentNotAvailable => ClipboardAccessError::ContentNotAvailable,
            arboard::Error::ClipboardNotSupported => ClipboardAccessError::ClipboardNotSupported,
            arboard::Error::ClipboardOccupied => ClipboardAccessError::ClipboardOccupied,
            arboard::Error::ConversionFailure => ClipboardAccessError::ConversionFailure,
            arboard::Error::Unknown { description } => ClipboardAccessError::Unknown(description),
            _ => ClipboardAccessError::Unknown(value.to_string()),
        }
    }
}

#[cfg_attr(test, mockall::automock)]
pub trait ClipboardAccess {
    fn set_text(&mut self, text: String) -> Result<(), ClipboardAccessError>;
}

pub struct ArboardClipboard {
    inner: arboard::Clipboard,
}

impl ArboardClipboard {
    pub fn new() -> Result<Self, arboard::Error> {
        Ok(Self { inner: arboard::Clipboard::new()? })
    }
}

impl ClipboardAccess for ArboardClipboard {
    fn set_text(&mut self, text: String) -> Result<(), ClipboardAccessError> {
        self.inner
            .set_text(text)
            .map_err(ClipboardAccessError::from)
    }
}

pub struct BrokenClipboard {
    error: ClipboardAccessError,
}

impl BrokenClipboard {
    pub fn new(error: arboard::Error) -> Self {
        Self { error: ClipboardAccessError::from(error) }
    }
}

impl ClipboardAccess for BrokenClipboard {
    fn set_text(&mut self, _text: String) -> Result<(), ClipboardAccessError> {
        Err(self.error.clone())
    }
}