use crate::boundary;
use crate::boundary::Session;
use crate::components::table::commands::{Command, HasCommands};
use crate::components::table::{FilterItems, SortItems, TableColumn};
use crate::components::{Alerts, TablePage};
use crate::connection_manager::ConnectionManager;
use crate::router::Router;
use crate::routes::Routes;
use crossterm::event::Event;
use ratatui::layout::Constraint;
use ratatui::Frame;
use std::cell::RefCell;
use std::rc::Rc;

pub struct SessionsPage<'a, C> {
    table_page: TablePage<'a, boundary::Session>,
    boundary_client: &'a C,
    connection_manager: &'a ConnectionManager<'a, C>,
    alerts: &'a Alerts,
    handle: tokio::runtime::Handle,
    user_id: String,
    target_id: String,
    scope_id: String,
}

#[derive(Debug, Clone, Copy)]
pub enum SessionCommands {
    Stop,
}

impl HasCommands for Session {
    type Id = SessionCommands;

    fn commands() -> Vec<Command<Self::Id>> {
        vec![Command::new(
            SessionCommands::Stop,
            "Stop".to_string(),
            "Ctrl+D".to_string(),
        )]
    }

    fn is_enabled(&self, id: Self::Id) -> bool {
        match id {
            SessionCommands::Stop => self.authorized_actions.contains(&"cancel:self".to_string()),
        }
    }
}

impl<'a, C> SessionsPage<'a, C> {
    pub(crate) fn new(
        router: &'a RefCell<Router<Routes>>,
        boundary_client: &'a C,
        connection_manager: &'a ConnectionManager<'a, C>,
        alerts: &'a Alerts,
        user_id: String,
        scope_id: String,
        target_id: String,
    ) -> Self
    where
        C: boundary::ApiClient,
    {
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

        let table_page = TablePage::new("Sessions".to_string(), vec![], columns, router);

        let mut session_page = SessionsPage {
            table_page,
            boundary_client,
            connection_manager,
            alerts,
            user_id,
            target_id,
            scope_id,
            handle: tokio::runtime::Handle::current(),
        };
        session_page.load_sessions();
        session_page
    }

    fn filter_sessions(&self, sessions: Vec<Session>) -> Vec<Session> {
        sessions
            .into_iter()
            .filter(|s| s.target_id == self.target_id && self.user_id == s.user_id)
            .collect()
    }

    fn load_sessions(&mut self)
    where
        C: boundary::ApiClient,
    {
        let sessions = self
            .handle
            .block_on(self.boundary_client.get_sessions(&self.scope_id))
            .map(|sessions| self.filter_sessions(sessions));
        match sessions {
            Ok(sessions) => self.set_sessions(sessions),
            Err(e) => self.alerts.alert(
                "Error".to_string(),
                format!("Failed to load sessions: {}", e),
            ),
        }
    }

    fn set_sessions(&mut self, sessions: Vec<Session>) {
        self.table_page
            .update_items(sessions.into_iter().map(Rc::new).collect());
    }

    fn stop_session(&mut self)
    where
        C: boundary::ApiClient,
    {
        if let Some(session) = self.table_page.selected_item() {
            if !session.can_cancel() {
                return;
            }
            let stop_result = self
                .handle
                .block_on(self.connection_manager.stop(&session.id));
            match stop_result {
                Ok(_) => self.load_sessions(),
                Err(e) => self.alerts.alert("Error".to_string(), format!("{}", e)),
            }
        }
    }

    pub(crate) fn handle_event(&mut self, event: &Event) -> bool
    where
        C: boundary::ApiClient,
    {
        if self.table_page.handle_event(event) {
            return true;
        }
        if let Event::Key(key_event) = event {
            if key_event.code == crossterm::event::KeyCode::Char('d')
                && key_event.modifiers == crossterm::event::KeyModifiers::CONTROL
            {
                self.stop_session();
                return true;
            }
        }
        false
    }

    pub(crate) fn render(&self, frame: &mut Frame) {
        self.table_page.render(frame);
    }
}

impl FilterItems<Session> for TablePage<'_, Session> {
    fn matches(item: &Session, search: &str) -> bool {
        Self::match_str(&item.id, search)
            || Self::match_str(&item.target_id, search)
            || Self::match_str(&item.session_type, search)
            || Self::match_str(&item.status, search)
            || Self::match_str(&item.created_time.to_string(), search)
    }
}

impl SortItems<Session> for TablePage<'_, Session> {
    fn sort(items: &mut Vec<Rc<Session>>) {
        items.sort_by(|a, b| a.created_time.cmp(&b.created_time));
    }
}
