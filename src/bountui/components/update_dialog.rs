use crate::bountui::Message;
use crossterm::event::{Event, KeyCode};
use ratatui::layout::{Alignment, Constraint, Flex, Layout};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};
use ratatui::Frame;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfirmButton {
    Update,
    Later,
}

#[derive(Debug)]
enum UpdateDialogState {
    /// Asking the user whether to update to the contained version.
    Confirm { version: String },
    /// The update to the contained version is being downloaded and installed.
    Updating { version: String },
    /// The update succeeded; bountui still runs the old binary until restart.
    Succeeded { version: String },
    /// The update failed with the contained error message.
    Failed { error: String },
}

/// Modal dialog that offers to update bountui to the latest GitHub release.
pub struct UpdateDialog {
    state: UpdateDialogState,
    selected_button: ConfirmButton,
    message_tx: mpsc::Sender<Message>,
}

impl UpdateDialog {
    pub fn new(version: String, message_tx: mpsc::Sender<Message>) -> Self {
        Self {
            state: UpdateDialogState::Confirm { version },
            selected_button: ConfirmButton::Update,
            message_tx,
        }
    }

    pub fn view(&self, frame: &mut Frame) {
        let area = frame.area();
        let vertical = Layout::vertical([Constraint::Length(9)]).flex(Flex::Center);
        let horizontal = Layout::horizontal([Constraint::Percentage(50)]).flex(Flex::Center);
        let [area] = vertical.areas(area);
        let [area] = horizontal.areas(area);

        let title = match &self.state {
            UpdateDialogState::Confirm { .. } => " Update Available ",
            UpdateDialogState::Updating { .. } => " Updating ",
            UpdateDialogState::Succeeded { .. } => " Update Complete ",
            UpdateDialogState::Failed { .. } => " Update Failed ",
        };

        let block = Block::bordered()
            .light_blue()
            .on_black()
            .title_alignment(Alignment::Center)
            .title(Span::from(title).bold());

        let [_, text_area, _, button_area, _] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(block.inner(area));

        let (lines, buttons) = match &self.state {
            UpdateDialogState::Confirm { version } => (
                vec![
                    Line::raw("A new version of bountui is available:"),
                    Line::raw(""),
                    Line::from(format!(
                        "{}  →  {}",
                        crate::updater::current_version(),
                        version
                    ))
                    .bold(),
                ],
                Some(self.confirm_buttons()),
            ),
            UpdateDialogState::Updating { version } => (
                vec![Line::raw(format!(
                    "Downloading and installing v{version} ..."
                ))],
                None,
            ),
            UpdateDialogState::Succeeded { version } => (
                vec![
                    Line::raw(format!("bountui was updated to v{version}.")),
                    Line::raw(""),
                    Line::raw("Restart bountui to use the new version."),
                ],
                Some(self.ok_button()),
            ),
            UpdateDialogState::Failed { error } => (
                vec![
                    Line::raw("Failed to update bountui:"),
                    Line::raw(""),
                    Line::raw(error.clone()),
                ],
                Some(self.ok_button()),
            ),
        };

        frame.render_widget(Clear, area);
        frame.render_widget(block, area);
        frame.render_widget(
            Paragraph::new(lines)
                .alignment(Alignment::Center)
                .wrap(Wrap::default()),
            text_area,
        );
        if let Some(buttons) = buttons {
            frame.render_widget(buttons, button_area);
        }
    }

    fn confirm_buttons(&self) -> Paragraph<'static> {
        let button_spans = [
            (ConfirmButton::Update, "Update"),
            (ConfirmButton::Later, "Later"),
        ]
        .iter()
        .map(|(button, title)| {
            let span = Span::from(format!("    {title}    ")).bold();
            if *button == self.selected_button {
                span.reversed()
            } else {
                span
            }
        });
        Paragraph::new(Line::from(
            button_spans
                .flat_map(|span| [span, Span::raw("  ")])
                .collect::<Vec<_>>(),
        ))
        .alignment(Alignment::Center)
    }

    fn ok_button(&self) -> Paragraph<'static> {
        Paragraph::new(Line::from(Span::from("    Ok    ").bold().reversed()))
            .alignment(Alignment::Center)
    }

    pub async fn handle_event(&mut self, event: &Event) {
        let Event::Key(key_event) = event else {
            return;
        };
        match &self.state {
            UpdateDialogState::Confirm { version } => match key_event.code {
                KeyCode::Left | KeyCode::Right | KeyCode::Tab => {
                    self.selected_button = match self.selected_button {
                        ConfirmButton::Update => ConfirmButton::Later,
                        ConfirmButton::Later => ConfirmButton::Update,
                    };
                }
                KeyCode::Enter => match self.selected_button {
                    ConfirmButton::Update => self.start_update(version.clone()).await,
                    ConfirmButton::Later => self.dismiss().await,
                },
                KeyCode::Char('y') => self.start_update(version.clone()).await,
                KeyCode::Esc | KeyCode::Char('n') => self.dismiss().await,
                _ => {}
            },
            // Keys are ignored while the binary is being replaced.
            UpdateDialogState::Updating { .. } => {}
            UpdateDialogState::Succeeded { .. } | UpdateDialogState::Failed { .. } => {
                match key_event.code {
                    KeyCode::Enter | KeyCode::Esc => self.dismiss().await,
                    _ => {}
                }
            }
        }
    }

    async fn start_update(&mut self, version: String) {
        let _ = self
            .message_tx
            .send(Message::StartUpdate(version.clone()))
            .await;
        self.state = UpdateDialogState::Updating { version };
    }

    async fn dismiss(&self) {
        let _ = self.message_tx.send(Message::DismissUpdate).await;
    }

    pub fn update_completed(&mut self, result: Result<String, String>) {
        self.state = match result {
            Ok(version) => UpdateDialogState::Succeeded { version },
            Err(error) => UpdateDialogState::Failed { error },
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn dialog() -> (UpdateDialog, mpsc::Receiver<Message>) {
        let (tx, rx) = mpsc::channel(4);
        (
            UpdateDialog::new("9.9.9".to_string(), tx),
            rx,
        )
    }

    #[tokio::test]
    async fn enter_on_update_button_starts_update() {
        let (mut dialog, mut rx) = dialog();
        dialog.handle_event(&key(KeyCode::Enter)).await;

        match rx.try_recv() {
            Ok(Message::StartUpdate(version)) => assert_eq!(version, "9.9.9"),
            other => panic!("Expected StartUpdate message, got {:?}", other.is_err()),
        }
        assert!(matches!(
            dialog.state,
            UpdateDialogState::Updating { .. }
        ));
    }

    #[tokio::test]
    async fn esc_dismisses_dialog() {
        let (mut dialog, mut rx) = dialog();
        dialog.handle_event(&key(KeyCode::Esc)).await;
        assert!(matches!(rx.try_recv(), Ok(Message::DismissUpdate)));
    }

    #[tokio::test]
    async fn right_then_enter_dismisses_dialog() {
        let (mut dialog, mut rx) = dialog();
        dialog.handle_event(&key(KeyCode::Right)).await;
        dialog.handle_event(&key(KeyCode::Enter)).await;
        assert!(matches!(rx.try_recv(), Ok(Message::DismissUpdate)));
    }

    #[tokio::test]
    async fn keys_are_ignored_while_updating() {
        let (mut dialog, mut rx) = dialog();
        dialog.handle_event(&key(KeyCode::Enter)).await;
        let _ = rx.try_recv();

        dialog.handle_event(&key(KeyCode::Esc)).await;
        dialog.handle_event(&key(KeyCode::Enter)).await;
        assert!(rx.try_recv().is_err(), "No further message expected");
        assert!(matches!(
            dialog.state,
            UpdateDialogState::Updating { .. }
        ));
    }

    #[tokio::test]
    async fn update_completed_shows_result_and_ok_dismisses() {
        let (mut dialog, mut rx) = dialog();
        dialog.update_completed(Ok("9.9.9".to_string()));
        assert!(matches!(
            dialog.state,
            UpdateDialogState::Succeeded { .. }
        ));

        dialog.handle_event(&key(KeyCode::Enter)).await;
        assert!(matches!(rx.try_recv(), Ok(Message::DismissUpdate)));
    }

    #[tokio::test]
    async fn update_completed_with_error_shows_failure() {
        let (mut dialog, _rx) = dialog();
        dialog.update_completed(Err("boom".to_string()));
        assert!(matches!(dialog.state, UpdateDialogState::Failed { .. }));
    }
}
