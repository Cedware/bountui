use crate::boundary;
use crate::boundary::{ApiClient, ApiClientExt, CredentialEntry, Error, SessionWithTarget};
use crate::bountui::components::credential_dialog::CredentialDialog;
use crate::bountui::components::table::action::Action;
use crate::bountui::components::table::util::format_title_with_parent;
use crate::bountui::components::table::{FilterItems, SortItems, TableColumn};
use crate::bountui::components::TablePage;
use crate::bountui::Message;
use crossterm::event::{Event, KeyCode, KeyModifiers};
use futures::FutureExt;
use ratatui::layout::{Constraint, Rect};
use ratatui::Frame;
use std::collections::HashMap;
use std::future::Future;
use std::rc::Rc;
use std::time::Duration;
use tokio::select;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use tokio_util::sync::CancellationToken;

pub struct SessionsPage<R: LoadSessions + Send + 'static> {
    table_page: TablePage<boundary::SessionWithTarget>,
    message_tx: mpsc::Sender<Message>,
    reload_now_tx: mpsc::Sender<()>,
    marker: std::marker::PhantomData<R>,
    cancellation_token: CancellationToken,
    credentials: Rc<HashMap<String, Vec<CredentialEntry>>>,
    credential_dialog: Option<CredentialDialog>,
}

impl<L: LoadSessions + Send + Sync + 'static> SessionsPage<L> {
    pub async fn new(
        parent_name: Option<&str>,
        load_sessions: L,
        message_tx: mpsc::Sender<Message>,
        credentials: HashMap<String, Vec<CredentialEntry>>,
    ) -> Self {
        let credentials = Rc::new(credentials);

        let columns = vec![
            TableColumn::new(
                "Id".to_string(),
                Constraint::Ratio(1, 6),
                Box::new(|s: &boundary::SessionWithTarget| s.session.id.clone()),
            ),
            TableColumn::new(
                "Target name".to_string(),
                Constraint::Ratio(1, 6),
                Box::new(|s| s.target.name.clone()),
            ),
            TableColumn::new(
                "Target".to_string(),
                Constraint::Ratio(1, 6),
                Box::new(|s| s.target.id.clone()),
            ),
            TableColumn::new(
                "Type".to_string(),
                Constraint::Ratio(1, 6),
                Box::new(|s| s.session.session_type.clone()),
            ),
            TableColumn::new(
                "Status".to_string(),
                Constraint::Ratio(1, 6),
                Box::new(|s| s.session.status.clone()),
            ),
            TableColumn::new(
                "Created Time".to_string(),
                Constraint::Ratio(1, 6),
                Box::new(|s| s.session.created_time.to_string()),
            ),
        ];

        let credentials_for_action = credentials.clone();
        let actions = vec![
            Action::new(
                "Quit".to_string(),
                "Ctrl + C".to_string(),
                Box::new(|_: Option<&SessionWithTarget>| true),
            ),
            Action::new(
                "Back".to_string(),
                "ESC".to_string(),
                Box::new(|_: Option<&SessionWithTarget>| true),
            ),
            Action::new(
                "Stop Session".to_string(),
                "Ctrl + d".to_string(),
                Box::new(|item: Option<&SessionWithTarget>| {
                    item.map_or(false, |s| s.session.can_cancel())
                }),
            ),
            Action::new(
                "Show Credentials".to_string(),
                "v".to_string(),
                Box::new(move |item: Option<&SessionWithTarget>| {
                    item.map_or(false, |s| {
                        credentials_for_action.contains_key(&s.session.id)
                    })
                }),
            ),
        ];

        let table_page = TablePage::new(
            format_title_with_parent("Sessions", parent_name),
            columns,
            Vec::new(),
            actions,
            message_tx.clone(),
            true,
        );

        let (reload_now_tx, mut reload_now_rx) = mpsc::channel(1);

        let cancellation_token = CancellationToken::new();
        {
            let cancellation_token = cancellation_token.clone();
            let refresh_future = async move {
                loop {
                    load_sessions.update_sessions().await;
                    select! {
                        _ = reload_now_rx.recv() => {}
                        _ = tokio::time::sleep(Duration::from_secs(5)) => {}
                        _ = cancellation_token.cancelled() => {
                                break;
                            }
                    }
                }
            }
            .boxed();

            let _ = message_tx.send(Message::RunFuture(refresh_future)).await;
        }

        SessionsPage {
            table_page,
            message_tx,
            reload_now_tx,
            cancellation_token,
            marker: std::marker::PhantomData,
            credentials,
            credential_dialog: None,
        }
    }

    async fn stop_session(&self) {
        if let Some(session) = self.table_page.selected_item() {
            self.message_tx
                .send(Message::StopSession {
                    session_id: session.session.id.clone(),
                    notify_stopped_tx: self.reload_now_tx.clone(),
                })
                .await
                .unwrap();
        }
    }

    fn show_credentials(&mut self) {
        if let Some(session) = self.table_page.selected_item() {
            if let Some(creds) = self.credentials.get(&session.session.id) {
                self.credential_dialog = Some(CredentialDialog::new(
                    creds.clone(),
                    self.message_tx.clone(),
                ));
            }
        }
    }

    pub fn view(&self, frame: &mut Frame, area: Rect) {
        self.table_page.view(frame, area);
        if let Some(dialog) = &self.credential_dialog {
            dialog.view(frame);
        }
    }

    pub async fn handle_event(&mut self, event: &Event) {
        if let Some(dialog) = &mut self.credential_dialog {
            if let Event::Key(key_event) = event {
                if key_event.code == KeyCode::Esc {
                    self.credential_dialog = None;
                    return;
                }
            }
            dialog.handle_event(event).await;
            return;
        }

        if self.table_page.handle_event(event).await {
            return;
        }
        if let Event::Key(key_event) = event {
            if key_event.code == KeyCode::Char('d')
                && key_event.modifiers == KeyModifiers::CONTROL
            {
                self.stop_session().await;
            }
            if key_event.code == KeyCode::Char('v')
                && key_event.modifiers == KeyModifiers::NONE
            {
                self.show_credentials();
            }
        }
    }

    pub fn handle_message(&mut self, message: SessionsPageMessage) {
        match message {
            SessionsPageMessage::SessionsLoaded(sessions) => {
                self.table_page.set_items(sessions);
                self.table_page.loading = false;
            }
        }
    }
}

impl FilterItems<SessionWithTarget> for TablePage<SessionWithTarget> {
    fn matches(item: &SessionWithTarget, search: &str) -> bool {
        Self::match_str(&item.session.id, search)
            || Self::match_str(&item.target.id, search)
            || Self::match_str(&item.target.name, search)
            || Self::match_str(&item.session.session_type, search)
            || Self::match_str(&item.session.status, search)
            || Self::match_str(&item.session.created_time.to_string(), search)
    }
}

impl SortItems<SessionWithTarget> for TablePage<SessionWithTarget> {
    fn sort(items: &mut Vec<Rc<SessionWithTarget>>) {
        items.sort_by(|a, b| a.session.created_time.cmp(&b.session.created_time));
    }
}

impl<R: LoadSessions> Drop for SessionsPage<R> {
    fn drop(&mut self) {
        self.cancellation_token.cancel();
    }
}

pub trait LoadSessions: Send + Sync + Clone {
    fn fetch_sessions(
        &self,
    ) -> impl Future<Output = Result<Vec<boundary::SessionWithTarget>, boundary::Error>> + Send;

    fn message_tx(&self) -> &Sender<Message>;

    fn fetch_sessions_or_show_error(
        &self,
    ) -> impl Future<Output = Option<Vec<SessionWithTarget>>> + Send {
        async {
            match self.fetch_sessions().await {
                Ok(sessions) => Some(sessions),
                Err(e) => {
                    let _ = self
                        .message_tx()
                        .send(Message::show_error("Error loading sessions", e))
                        .await;
                    None
                }
            }
        }
    }

    fn update_sessions(&self) -> impl Future<Output = ()> + Send {
        async move {
            if let Some(sessions) = self.fetch_sessions_or_show_error().await {
                self.message_tx()
                    .send(SessionsPageMessage::SessionsLoaded(sessions).into())
                    .await
                    .unwrap();
            }
        }
    }
}

#[derive(Clone)]
pub struct LoadTargetSessionsSessions<B: boundary::ApiClient> {
    scope_id: String,
    target_id: String,
    boundary_client: B,
    message_tx: mpsc::Sender<Message>,
}

impl<B: boundary::ApiClient + Send + Sync> LoadTargetSessionsSessions<B> {
    pub fn new(
        scope_id: String,
        target_id: String,
        boundary_client: B,
        message_tx: mpsc::Sender<Message>,
    ) -> Self {
        LoadTargetSessionsSessions {
            scope_id,
            target_id,
            boundary_client,
            message_tx,
        }
    }
}

impl<B: ApiClient + Clone + Send + Sync + 'static> LoadSessions for LoadTargetSessionsSessions<B> {
    async fn fetch_sessions(&self) -> Result<Vec<SessionWithTarget>, Error> {
        self.boundary_client
            .get_sessions_with_target(&self.scope_id)
            .await
            .map(|sessions| {
                sessions
                    .into_iter()
                    .filter(|s| s.target.id == self.target_id)
                    .collect()
            })
    }

    fn message_tx(&self) -> &Sender<Message> {
        &self.message_tx
    }
}

#[derive(Clone)]
pub struct LoadUserSessions<B: boundary::ApiClient> {
    user_id: String,
    boundary_client: B,
    message_tx: mpsc::Sender<Message>,
}

impl<B: boundary::ApiClient> LoadUserSessions<B> {
    pub fn new(user_id: String, boundary_client: B, message_tx: mpsc::Sender<Message>) -> Self {
        LoadUserSessions {
            user_id,
            boundary_client,
            message_tx,
        }
    }
}

impl<B: boundary::ApiClient + Clone + Send + Sync + 'static> LoadSessions for LoadUserSessions<B> {
    async fn fetch_sessions(&self) -> Result<Vec<SessionWithTarget>, Error> {
        self.boundary_client
            .get_user_sessions_with_target(&self.user_id)
            .await
    }

    fn message_tx(&self) -> &Sender<Message> {
        &self.message_tx
    }
}

#[derive(Clone, Debug)]
pub enum SessionsPageMessage {
    SessionsLoaded(Vec<SessionWithTarget>),
}

impl From<SessionsPageMessage> for Message {
    fn from(msg: SessionsPageMessage) -> Self {
        Message::SessionsPage(msg)
    }
}
