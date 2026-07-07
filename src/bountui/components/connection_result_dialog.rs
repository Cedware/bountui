use crate::boundary;
use crate::bountui::components::credential_table::CredentialTable;
use crate::bountui::Message;
use crossterm::event::Event;
use ratatui::layout::Flex;
use ratatui::prelude::{Alignment, Stylize};
use ratatui::widgets::Clear;
use ratatui::{
    layout::{Constraint, Layout},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};
use tokio::sync::mpsc;

pub struct ConnectionEstablishedDialog {
    /// `None` when no credentials were returned — a compact success dialog is shown instead.
    credential_table: Option<CredentialTable>,
}

impl ConnectionEstablishedDialog {
    pub fn new(
        credentials: Vec<boundary::CredentialEntry>,
        message_tx: mpsc::Sender<Message>,
    ) -> Self {
        let credential_table = if credentials.is_empty() {
            None
        } else {
            Some(CredentialTable::new(credentials, message_tx))
        };

        Self { credential_table }
    }

    pub fn view(&self, frame: &mut Frame) {
        match &self.credential_table {
            Some(credential_table) => self.view_with_credentials(frame, credential_table),
            None => self.view_success_only(frame),
        }
    }

    fn view_with_credentials(&self, frame: &mut Frame, credential_table: &CredentialTable) {
        let area = frame.area();
        let vertical = Layout::vertical([Constraint::Percentage(70)]).flex(Flex::Center);
        let horizontal = Layout::horizontal([Constraint::Percentage(70)]).flex(Flex::Center);
        let [area] = vertical.areas(area);
        let [area] = horizontal.areas(area);

        frame.render_widget(Clear, area);

        let block = Block::default()
            .title("Connection Established")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .light_blue()
            .on_black();

        let inner_area = block.inner(area);
        frame.render_widget(block, area);
        credential_table.view(frame, inner_area)
    }

    fn view_success_only(&self, frame: &mut Frame) {
        let area = frame.area();
        let vertical = Layout::vertical([Constraint::Length(5)]).flex(Flex::Center);
        let horizontal = Layout::horizontal([Constraint::Percentage(40)]).flex(Flex::Center);
        let [area] = vertical.areas(area);
        let [area] = horizontal.areas(area);

        frame.render_widget(Clear, area);

        let block = Block::default()
            .title("Connection Established")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .light_blue()
            .on_black();

        let inner_area = block.inner(area);
        frame.render_widget(block, area);

        let [text_area] = Layout::vertical([Constraint::Fill(1)])
            .flex(Flex::Center)
            .areas(inner_area);

        let message = Paragraph::new("Connection established successfully.")
            .alignment(Alignment::Center);
        frame.render_widget(message, text_area);
    }

    pub async fn handle_event(&mut self, event: &Event) {
        if let Some(credential_table) = &mut self.credential_table {
            credential_table.handle_event(event).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boundary::{Credential, CredentialEntry, CredentialSource};
    use chrono::Utc;
    use crossterm::event::KeyCode;

    fn sample_credentials() -> Vec<CredentialEntry> {
        vec![CredentialEntry {
            credential: Credential {
                username: "user1".to_string(),
                password: "pass1".to_string(),
            },
            credential_source: CredentialSource {
                name: "test-source".to_string(),
            },
        }]
    }

    fn empty_connect_response() -> Vec<boundary::CredentialEntry> {
        Vec::new()
    }

    #[tokio::test]
    async fn new_with_credentials_keeps_credential_table() {
        let (tx, _rx) = mpsc::channel::<Message>(1);
        let dialog = ConnectionEstablishedDialog::new(sample_credentials(), tx);
        assert!(dialog.credential_table.is_some());
    }

    #[tokio::test]
    async fn new_without_credentials_drops_credential_table() {
        let (tx, _rx) = mpsc::channel::<Message>(1);
        let dialog = ConnectionEstablishedDialog::new(empty_connect_response(), tx);
        assert!(dialog.credential_table.is_none());
    }

    #[tokio::test]
    async fn handle_event_without_credentials_does_not_panic() {
        let (tx, _rx) = mpsc::channel::<Message>(1);
        let mut dialog = ConnectionEstablishedDialog::new(empty_connect_response(), tx);
        // Any key event should be a no-op when there is no credential table.
        dialog
            .handle_event(&Event::Key(crossterm::event::KeyEvent::new(
                KeyCode::Enter,
                crossterm::event::KeyModifiers::NONE,
            )))
            .await;
    }

    #[test]
    fn unused_connect_response_fields_are_documented() {
        // Sanity check that the ConnectResponse shape we rely on still exists.
        let _ = boundary::ConnectResponse {
            credentials: Vec::new(),
            session_id: String::new(),
            expiration: Utc::now(),
        };
    }
}
