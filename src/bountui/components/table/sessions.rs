use std::future::Future;
use crossterm::event::Event;
use ratatui::Frame;
use ratatui::layout::Constraint;
use crate::appframework::{Component, UpdateState};
use crate::boundary;
use crate::boundary::Session;
use crate::bountui::components::table::{FilterItems, HasActions, TableColumn, TableMessage};
use crate::bountui::components::table::action::Action;
use crate::bountui::components::TablePage;
use crate::bountui::Message;

pub enum SessionsMessage {
    Table(TableMessage),
}

impl From<SessionsMessage> for Message {
    fn from(value: SessionsMessage) -> Self {
        Message::Session(value)
    }
}

impl From<TableMessage> for SessionsMessage {
    fn from(value: TableMessage) -> Self {
        SessionsMessage::Table(value)
    }
}

pub struct SessionsPage {
    table_page: TablePage<boundary::Session>,
    user_id: String,
    pub target_id: String,
    pub scope_id: String,
}

impl SessionsPage {

    pub fn new(scope_id: String, target_id: String, sessions: Vec<Session>) -> Self {
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

        let table_page = TablePage::new("Sessions".to_string(), columns, sessions);

        SessionsPage {
            table_page,
            user_id: String::new(),
            target_id,
            scope_id,
        }
    }

    fn stop_session(&self) -> Option<Message>{
        self.table_page.selected_item()
            .map(|session| Message::StopSession{session_id: session.id.clone()})
    }

    pub fn set_sessions(&mut self, sessions: Vec<Session>) {
        self.table_page.set_items(sessions);
    }
}

impl HasActions<Session> for TablePage<Session> {
    type Id = SessionAction;

    fn actions(&self) -> Vec<Action<Self::Id>> {
        vec![Action::new(
            SessionAction::Stop,
            "Stop".to_string(),
            "Ctrl+D".to_string(),
        )]
    }

    fn is_action_enabled(&self, id: Self::Id, item: &Session) -> bool {
        match id {
            SessionAction::Stop => item.can_cancel(),
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

impl Component<Message> for SessionsPage {
    fn view(&self, frame: &mut Frame) {
        self.table_page.view(frame);
    }

    fn handle_event(&self, event: &Event) -> Option<Message> {
        if let Event::Key(key_event) = event {
            if key_event.code == crossterm::event::KeyCode::Char('d')
                && key_event.modifiers == crossterm::event::KeyModifiers::CONTROL
            {
                return self.stop_session()
            }
        }
        self.table_page.handle_event(event).map(SessionsMessage::from).map(Message::from)
    }
}

impl UpdateState<SessionsMessage, Message> for SessionsPage {
    async fn update(&mut self, message: &SessionsMessage) -> Option<Message> {
        match message {
            SessionsMessage::Table(table_message) => {
                self.table_page.update(table_message).await
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SessionAction {
    Stop,
}



//
// #[derive(Debug, Clone, Copy)]
// pub enum SessionAction {
//     Stop,
// }
// 
// 
// impl<'a, C> SessionsPage<'a, C> {
//     pub(crate) fn new(
//         router: &'a Router<Routes>,
//         boundary_client: &'a C,
//         connection_manager: &'a ConnectionManager<'a, C>,
//         alerts: &'a Alerts,
//         user_id: String,
//         scope_id: String,
//         target_id: String,
//     ) -> Self
//     where
//         C: boundary::ApiClient,
//     {
//         let columns = vec![
//             TableColumn::new(
//                 "Id".to_string(),
//                 Constraint::Ratio(1, 5),
//                 Box::new(|s: &boundary::Session| s.id.clone()),
//             ),
//             TableColumn::new(
//                 "Target".to_string(),
//                 Constraint::Ratio(1, 5),
//                 Box::new(|s| s.target_id.clone()),
//             ),
//             TableColumn::new(
//                 "Type".to_string(),
//                 Constraint::Ratio(1, 5),
//                 Box::new(|s| s.session_type.clone()),
//             ),
//             TableColumn::new(
//                 "Status".to_string(),
//                 Constraint::Ratio(1, 5),
//                 Box::new(|s| s.status.clone()),
//             ),
//             TableColumn::new(
//                 "Created Time".to_string(),
//                 Constraint::Ratio(1, 5),
//                 Box::new(|s| s.created_time.to_string()),
//             ),
//         ];
// 
//         let table_page = TablePage::new("Sessions".to_string(), vec![], columns, router);
// 
//         let mut session_page = SessionsPage {
//             table_page,
//             boundary_client,
//             connection_manager,
//             alerts,
//             user_id,
//             target_id,
//             scope_id,
//             handle: tokio::runtime::Handle::current(),
//         };
//         session_page.load_sessions();
//         session_page
//     }
// 
//     fn filter_sessions(&self, sessions: Vec<Session>) -> Vec<Session> {
//         sessions
//             .into_iter()
//             .filter(|s| s.target_id == self.target_id && self.user_id == s.user_id)
//             .collect()
//     }
// 
//     fn load_sessions(&mut self)
//     where
//         C: boundary::ApiClient,
//     {
//         let sessions = self
//             .handle
//             .block_on(self.boundary_client.get_sessions(&self.scope_id))
//             .map(|sessions| self.filter_sessions(sessions));
//         match sessions {
//             Ok(sessions) => self.set_sessions(sessions),
//             Err(e) => self.alerts.alert(
//                 "Error".to_string(),
//                 format!("Failed to load sessions: {}", e),
//             ),
//         }
//     }
// 
//     fn set_sessions(&mut self, sessions: Vec<Session>) {
//         self.table_page
//             .update_items(sessions.into_iter().map(Rc::new).collect());
//     }
// 
//     fn stop_session(&mut self)
//     where
//         C: boundary::ApiClient,
//     {
//         if let Some(session) = self.table_page.selected_item() {
//             if !session.can_cancel() {
//                 return;
//             }
//             let stop_result = self
//                 .handle
//                 .block_on(self.connection_manager.stop(&session.id));
//             match stop_result {
//                 Ok(_) => self.load_sessions(),
//                 Err(e) => self.alerts.alert("Error".to_string(), format!("{}", e)),
//             }
//         }
//     }
// 
//     pub(crate) fn handle_event(&mut self, event: &Event) -> bool
//     where
//         C: boundary::ApiClient,
//     {
//         if self.table_page.handle_event(event) {
//             return true;
//         }
//         if let Event::Key(key_event) = event {
//             if key_event.code == crossterm::event::KeyCode::Char('d')
//                 && key_event.modifiers == crossterm::event::KeyModifiers::CONTROL
//             {
//                 self.stop_session();
//                 return true;
//             }
//         }
//         false
//     }
// 
//     pub(crate) fn render(&self, frame: &mut Frame) {
//         self.table_page.render(frame);
//     }
// }
// 

// 
// impl SortItems<Session> for TablePage<'_, Session> {
//     fn sort(items: &mut Vec<Rc<Session>>) {
//         items.sort_by(|a, b| a.created_time.cmp(&b.created_time));
//     }
// }
// 
