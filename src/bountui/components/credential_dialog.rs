use crate::boundary;
use crate::bountui::components::credential_table::CredentialTable;
use crate::bountui::Message;
use crossterm::event::Event;
use ratatui::layout::Flex;
use ratatui::prelude::{Alignment, Stylize};
use ratatui::widgets::Clear;
use ratatui::{
    layout::{Constraint, Layout},
    widgets::{Block, BorderType, Borders},
    Frame,
};
use tokio::sync::mpsc;

pub struct CredentialDialog {
    credential_table: CredentialTable,
}

impl CredentialDialog {
    pub fn new(
        credentials: Vec<boundary::CredentialEntry>,
        message_tx: mpsc::Sender<Message>,
    ) -> Self {
        Self {
            credential_table: CredentialTable::new(credentials, message_tx),
        }
    }

    pub fn view(&self, frame: &mut Frame) {
        let area = frame.area();
        let vertical = Layout::vertical([Constraint::Percentage(70)]).flex(Flex::Center);
        let horizontal = Layout::horizontal([Constraint::Percentage(70)]).flex(Flex::Center);
        let [area] = vertical.areas(area);
        let [area] = horizontal.areas(area);

        frame.render_widget(Clear, area);

        let block = Block::default()
            .title("Credentials")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .light_blue()
            .on_black();

        let inner_area = block.inner(area);
        frame.render_widget(block, area);
        self.credential_table.view(frame, inner_area)
    }

    pub async fn handle_event(&mut self, event: &Event) {
        self.credential_table.handle_event(event).await;
    }
}
