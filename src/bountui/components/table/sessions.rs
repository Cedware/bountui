use std::future::Future;
use crate::boundary;
use crate::boundary::{ApiClient, Error, Session};
use crate::bountui::components::table::action::Action;
use crate::bountui::components::table::{FilterItems, SortItems, TableColumn};
use crate::bountui::components::TablePage;
use crate::bountui::Message;
use crossterm::event::Event;
use ratatui::layout::Constraint;
use ratatui::Frame;
use std::rc::Rc;
use std::time::Duration;
use tokio::select;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;

pub struct SessionsPage<R: ReloadSessions + Send + 'static> {
    table_page: TablePage<boundary::Session>,
    message_tx: mpsc::Sender<Message>,
    reload_join_handle: tokio::task::JoinHandle<()>,
    reload_now_tx: mpsc::Sender<()>,
    marker: std::marker::PhantomData<R>,
}

impl<R: ReloadSessions + Send + Sync + 'static> SessionsPage<R> {
    pub fn new(
        sessions: Vec<Session>,
        reload_sessions: R,
        message_tx: mpsc::Sender<Message>,
    ) -> Self {
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

        let table_page = TablePage::new(
            "Sessions".to_string(),
            columns,
            sessions,
            actions,
            message_tx.clone(),
        );

        let (reload_now_tx, reload_now_rx) = mpsc::channel(1);

        SessionsPage {
            table_page,
            message_tx,
            reload_join_handle: Self::reload_task(reload_sessions, reload_now_rx),
            reload_now_tx,
            marker: std::marker::PhantomData,
        }
    }

    fn reload_task(reload_sessions: R, mut reload_now_rx: mpsc::Receiver<()>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                reload_sessions.update_sessions().await;
                select! {
                    _ = reload_now_rx.recv() => {}
                    _ = tokio::time::sleep(Duration::from_secs(5)) => {}
                }
            }
        })
    }

    async fn stop_session(&self) {
        if let Some(session) = self.table_page.selected_item() {
            self.message_tx
                .send(Message::StopSession {
                    session_id: session.id.clone(),
                    notify_stopped_tx: self.reload_now_tx.clone(),
                })
                .await
                .unwrap();
        
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

    pub fn handle_message(&mut self, message: SessionsPageMessage) {
        match message {
            SessionsPageMessage::SessionsLoaded(sessions) => {
                self.table_page.set_items(sessions);
            }
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


pub trait ReloadSessions: Send + Sync {
    fn fetch_sessions(&self) -> impl Future<Output = Result<Vec<boundary::Session>, boundary::Error>> + Send;

    fn message_tx(&self) -> &mpsc::Sender<Message>;

    fn update_sessions(&self) -> impl Future<Output=()> + Send {
        async move {
            match self.fetch_sessions().await {
                Ok(sessions) => {
                    self.message_tx()
                        .send(SessionsPageMessage::SessionsLoaded(sessions).into())
                        .await
                        .unwrap();
                }
                Err(e) => {
                    let _ = self
                        .message_tx()
                        .send(Message::show_error("Error loading sessions", e));
                }
            }
        }
    }
}


pub struct ReloadScopeSessions<B: boundary::ApiClient> {
    scope_id: String,
    boundary_client: B,
    message_tx: mpsc::Sender<Message>,
}


impl <B: boundary::ApiClient + Send + Sync> ReloadScopeSessions<B> {
    pub fn new(scope_id: String, boundary_client: B, message_tx: mpsc::Sender<Message>) -> Self {
        ReloadScopeSessions {
            scope_id,
            boundary_client,
            message_tx,
        }
    }
}

impl<B: ApiClient + Send + Sync + 'static> ReloadSessions for ReloadScopeSessions<B> {
    async fn fetch_sessions(&self) -> Result<Vec<Session>, Error> {
        self.boundary_client.get_sessions(&self.scope_id).await
    }

    fn message_tx(&self) -> &Sender<Message> {
        &self.message_tx
    }
}

struct ReloadUserUserSessions<B: boundary::ApiClient> {
    user_id: String,
    boundary_client: B,
    message_tx: mpsc::Sender<Message>,
}

impl<B: boundary::ApiClient> ReloadUserUserSessions<B> {
    pub fn new(user_id: String, boundary_client: B, message_tx: mpsc::Sender<Message>) -> Self {
        ReloadUserUserSessions {
            user_id,
            boundary_client,
            message_tx,
        }
    }
}

impl<B: boundary::ApiClient + Send + Sync + 'static> ReloadSessions for ReloadUserUserSessions<B> {
    async fn fetch_sessions(&self) -> Result<Vec<Session>, Error> {
        self.boundary_client.get_user_sessions(&self.user_id).await
    }

    fn message_tx(&self) -> &Sender<Message> {
        &self.message_tx
    }
}


#[derive(Clone, Debug)]
pub enum SessionsPageMessage {
    SessionsLoaded(Vec<Session>),
}

impl From<SessionsPageMessage> for Message {
    fn from(msg: SessionsPageMessage) -> Self {
        Message::SessionsPage(msg)
    }
}