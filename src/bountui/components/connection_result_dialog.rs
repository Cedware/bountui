use crate::boundary;
use crate::boundary::CredentialEntry;
use crate::bountui::components::table::{Action, FilterItems, SortItems, TableColumn};
use crate::bountui::components::TablePage;
use crate::bountui::Message;
use arboard::Clipboard;
use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::layout::Flex;
use ratatui::prelude::{Alignment, Stylize};
use ratatui::widgets::Clear;
use ratatui::{layout::{Constraint, Layout}, widgets::{Block, BorderType, Borders}, Frame};
use std::rc::Rc;
use tokio::sync::mpsc;

pub struct ConnectionResultDialog {
    table: TablePage<boundary::CredentialEntry>,
    message_tx: mpsc::Sender<Message>
}

impl ConnectionResultDialog {

    pub fn new(connect_response: boundary::ConnectResponse, message_tx: mpsc::Sender<Message>) -> Self {

        let columns = vec![
            TableColumn::new(
                "Credential Source".to_string(),
                Constraint::Ratio(2,4),
                Box::new(|e: &boundary::CredentialEntry| e.credential_source.name.clone())
            ),
            TableColumn::new(
                "Username".to_string(),
                Constraint::Ratio(1,4),
                Box::new(|e: &boundary::CredentialEntry| e.credential.username.clone())
            ),
            TableColumn::new(
                "Password".to_string(),
                Constraint::Ratio(1,4),
                Box::new(|e| e.credential.password.clone())
            )
        ];

        let actions = vec![
            Action::new(
                "Close".to_string(),
                "ESC".to_string(),
                Box::new(|_: Option<&CredentialEntry>| true),
            ),
            Action::new(
                "Copy Username".to_string(),
                "u".to_string(),
                Box::new(|item: Option<&CredentialEntry>| item.is_some()),
            ),
            Action::new(
                "Copy Password".to_string(),
                "p".to_string(),
                Box::new(|item: Option<&CredentialEntry>| item.is_some()),
            ),
        ];

        let table = TablePage::new(
            "Credentials".to_string(),
            columns,
            connect_response.credentials,
            actions,
            message_tx.clone(),
            false
        );

        Self {
            table,
            message_tx
        }
    }



    pub fn view(&self, frame: &mut Frame) {
        let area = frame.area();
        let vertical = Layout::vertical([Constraint::Percentage(70)]).flex(Flex::Center);
        let horizontal = Layout::horizontal([Constraint::Percentage(70)]).flex(Flex::Center);
        let [area] = vertical.areas(area);
        let [area] = horizontal.areas(area);

        // To clear everything behind the dialog
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
        self.table.view(frame, inner_area)

    }

    pub async fn handle_event(&mut self, event: &Event) {
        if let Event::Key(key_event) = event {
            if key_event.modifiers == KeyModifiers::NONE {
                match key_event.code {
                    KeyCode::Char('u') => {
                        self.copy_selected_username_to_clipboard().await;
                    }
                    KeyCode::Char('p') => {
                        self.copy_selected_password_to_clipboard().await;
                    }
                    _ => {}
                }
            }
        }
        self.table.handle_event(event).await;
    }

    pub async fn copy_selected_username_to_clipboard(&self) {
        if let Some(selected_item) = self.table.selected_item() {
            let username = selected_item.credential.username.clone();
            match Clipboard::new().and_then(|mut c| c.set_text(username)) {
                Ok(_) => {}
                Err(e) => {
                    let _ = self.message_tx.send(
                        Message::ShowAlert(
                            "Clipboard Error".to_string(),
                            format!("Failed to copy username: {e}")
                        )
                    ).await;
                }
            }
        }
    }

    pub async fn copy_selected_password_to_clipboard(&self) {
        if let Some(selected_item) = self.table.selected_item() {
            let password = selected_item.credential.password.clone();
            match Clipboard::new().and_then(|mut c| c.set_text(password)) {
                Ok(_) => {}
                Err(e) => {
                    let _ = self.message_tx.send(
                        Message::ShowAlert(
                            "Clipboard Error".to_string(),
                            format!("Failed to copy password: {e}")
                        )
                    ).await;
                }
            }
        }
    }
}

impl SortItems<boundary::CredentialEntry> for TablePage<CredentialEntry>{
    fn sort(items: &mut Vec<Rc<CredentialEntry>>) {
        items.sort_by(|a, b| a.credential.username.cmp(&b.credential.username))
    }
}

impl FilterItems<CredentialEntry> for TablePage<CredentialEntry> {
    fn matches(item: &CredentialEntry, search: &str) -> bool {
        Self::match_str(&item.credential.username, search)
            || Self::match_str(&item.credential_source.name, search)
    }
}
