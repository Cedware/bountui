use std::rc::Rc;
use crossterm::event::Event;
use ratatui::Frame;
use ratatui::layout::Constraint;
use tokio::sync::mpsc;
use crate::boundary;
use crate::boundary::Session;
use crate::bountui::components::table::{FilterItems, SortItems, TableColumn};
use crate::bountui::components::table::action::Action;
use crate::bountui::components::TablePage;
use crate::bountui::Message;

pub struct SessionsPage {
    table_page: TablePage<boundary::Session>,
    pub scope_id: String,
    message_tx: mpsc::Sender<Message>,
}

impl SessionsPage {

    pub fn new(scope_id: String, sessions: Vec<Session>, message_tx: mpsc::Sender<Message>) -> Self {
        let columns = vec![
            TableColumn::new(
                "Id".to_string(),
                Constraint::Ratio(1, 5),
                Box::new(|s: &boundary::Session| s.id.clone()),
            ),
            TableColumn::new(
                "Target".to_string(),
                Constraint::Ratio(1, 5),
                Box::new(|s| s.target_id.clone()),
            ),
            TableColumn::new(
                "Type".to_string(),
                Constraint::Ratio(1, 5),
                Box::new(|s| s.session_type.clone()),
            ),
            TableColumn::new(
                "Status".to_string(),
                Constraint::Ratio(1, 5),
                Box::new(|s| s.status.clone()),
            ),
            TableColumn::new(
                "Created Time".to_string(),
                Constraint::Ratio(1, 5),
                Box::new(|s| s.created_time.to_string()),
            ),
        ];

        let actions = vec![
            Action::new(
                "Quit".to_string(),
                "Ctrl + C".to_string(),
                Box::new(|_: Option<&Session>| true),
            ),
            Action::new(
                "Back".to_string(),
                "ESC".to_string(),
                Box::new(|_: Option<&Session>| true),
            ),
            Action::new(
                "Stop Session".to_string(),
                "d".to_string(), // Note: Shortcut display only, actual handling is separate
                Box::new(|item: Option<&Session>| item.map_or(false, |s| s.can_cancel())),
            ),
        ];

        let table_page = TablePage::new("Sessions".to_string(), columns, sessions, actions, message_tx.clone());

        SessionsPage {
            table_page,
            scope_id,
            message_tx
        }
    }

    async fn stop_session(&self) {
        if let Some(session) = self.table_page.selected_item() {
            self.message_tx.send(
                Message::StopSession {
                    session_id: session.id.clone(),
                }
            ).await.unwrap();
        }
    }

    pub fn view(&self, frame: &mut Frame) {
        self.table_page.view(frame, frame.area());
    }

    pub async fn handle_event(&mut self, event: &Event) {
        if self.table_page.handle_event(event).await {
            return;
        }
        if let Event::Key(key_event) = event {
            if key_event.code == crossterm::event::KeyCode::Char('d')
                && key_event.modifiers == crossterm::event::KeyModifiers::CONTROL
            {
                self.stop_session().await;
            }
        }
    }

    pub fn set_sessions(&mut self, sessions: Vec<Session>) {
        self.table_page.set_items(sessions);
    }
}

impl FilterItems<Session> for TablePage<Session> {
    fn matches(item: &Session, search: &str) -> bool {
        Self::match_str(&item.id, search)
            || Self::match_str(&item.target_id, search)
            || Self::match_str(&item.session_type, search)
            || Self::match_str(&item.status, search)
            || Self::match_str(&item.created_time.to_string(), search)
    }
}


impl SortItems<Session> for TablePage<Session> {
    fn sort(items: &mut Vec<Rc<Session>>) {
        items.sort_by(|a, b| a.created_time.cmp(&b.created_time));
    }
}