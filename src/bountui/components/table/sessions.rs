use std::rc::Rc;
use crossterm::event::Event;
use ratatui::Frame;
use ratatui::layout::Constraint;
use tokio::sync::mpsc;
use crate::boundary;
use crate::boundary::Session;
use crate::bountui::components::table::{FilterItems, HasActions, SortItems, TableColumn};
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

        let table_page = TablePage::new("Sessions".to_string(), columns, sessions, message_tx.clone());

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
        if let Event::Key(key_event) = event {
            if key_event.code == crossterm::event::KeyCode::Char('d')
                && key_event.modifiers == crossterm::event::KeyModifiers::CONTROL
            {
                self.stop_session().await;
            }
        }
        self.table_page.handle_event(event).await;
    }

    pub fn set_sessions(&mut self, sessions: Vec<Session>) {
        self.table_page.set_items(sessions);
    }
}

impl HasActions<Session> for TablePage<Session> {
    type Id = SessionAction;

    fn actions(&self) -> Vec<Action<Self::Id>> {
        let mut actions = vec![
            Action::new(
                SessionAction::Quit,
                "Quit".to_string(),
                "Ctrl + C".to_string(),
            ),
            Action::new(
                SessionAction::Back,
                "Back".to_string(),
                "ESC".to_string(),
            ),
        ];
        if let Some(session) = self.selected_item() {
           if session.can_cancel() {
                actions.push(Action::new(
                    SessionAction::StopSession,
                    "Stop Session".to_string(),
                    "d".to_string(),
                ));
           }
        }
        actions
    }

    fn is_action_enabled(&self, id: Self::Id, item: &Session) -> bool {
        match id {
            SessionAction::StopSession => item.can_cancel(),
            SessionAction::Quit => true,
            SessionAction::Back => true,
        }
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

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SessionAction {
    StopSession,
    Quit,
    Back,
}