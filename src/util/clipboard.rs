use std::cell::RefCell;

/// Trait to abstract clipboard operations used by the UI.
/// Allows swapping in a no-op implementation for tests or when initialization fails.
pub trait ClipboardAccess {
    fn set_text(&mut self, text: String) -> Result<(), String>;
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
    fn set_text(&mut self, text: String) -> Result<(), String> {
        self.inner
            .set_text(text)
            .map_err(|e| format!("{e}"))
    }
}

#[derive(Default)]
pub struct NoopClipboard;

impl ClipboardAccess for NoopClipboard {
    fn set_text(&mut self, _text: String) -> Result<(), String> { Ok(()) }
}