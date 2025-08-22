use crate::boundary;
use crate::boundary::{Scope, Target};
use crate::bountui::components::table::scope::{ScopesPage, ScopesPageMessage};
use crate::bountui::components::table::sessions::{
    LoadTargetSessionsSessions, LoadUserSessions, SessionsPage, SessionsPageMessage,
};
use crate::bountui::components::table::target::{TargetsPage, TargetsPageMessage};
use crate::bountui::components::NavigationInput;
use crate::bountui::connection_manager::ConnectionManager;
use crate::bountui::widgets::Alert;
use crate::event_ext::EventExt;
use crate::util::clipboard::ClipboardAccess;
use crossterm::event::{Event, KeyCode};
use futures::future::BoxFuture;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use log::error;
use ratatui::layout::Constraint;
use ratatui::Frame;
pub use remember_user_input::*;
use std::fmt::Display;
use std::mem;
use tokio::select;

pub mod components;
pub mod connection_manager;
mod remember_user_input;
mod widgets;

pub enum Message {
    ShowScopes {
        parent: Option<Scope>,
    },
    ShowTargets {
        parent: Scope,
    },
    ShowSessions {
        scope: String,
        target: Target,
    },
    Connect {
        target_id: String,
        port: u16,
    },
    StopSession {
        session_id: String,
        notify_stopped_tx: tokio::sync::mpsc::Sender<()>,
    },
    GoBack,
    ShowAlert(String, String),
    SetClipboard(String),
    Targets(TargetsPageMessage),
    Scopes(ScopesPageMessage),
    SessionsPage(SessionsPageMessage),
    // Navigate root pages
    NavigateToScopeTree,
    NavigateToMySessions,
    RunFuture(BoxFuture<'static, ()>),
}

impl Message {
    fn show_error<M: Into<String>, E: Display>(message: M, error: E) -> Message {
        Message::ShowAlert(
            "Error".to_string(),
            format!("{}: {}", message.into(), error),
        )
    }
}

pub enum Page<B: boundary::ApiClient + Clone + Send + Sync + 'static, R: RememberUserInput> {
    Scopes(ScopesPage),
    Targets(TargetsPage<B, R>),
    TargetSessions(SessionsPage<LoadTargetSessionsSessions<B>>),
    UserSessions(SessionsPage<LoadUserSessions<B>>),
}

pub struct BountuiApp<
    C: boundary::ApiClient + Clone + Send + Sync + 'static,
    R: RememberUserInput + Copy,
    M: ConnectionManager,
> {
    page: Page<C, R>,
    boundary_client: C,
    history: Vec<Page<C, R>>,
    connection_manager: M,
    alert: Option<(String, String)>,
    message_tx: tokio::sync::mpsc::Sender<Message>,
    message_rx: tokio::sync::mpsc::Receiver<Message>,
    cross_term_event_rx: tokio::sync::mpsc::Receiver<Event>,
    user_id: String,
    navigation_input: Option<NavigationInput>,
    tasks: FuturesUnordered<BoxFuture<'static, ()>>,
    remember_user_input: R,
    clipboard: Box<dyn ClipboardAccess>,
}

impl<C, R: RememberUserInput + Copy, M> BountuiApp<C, R, M>
where
    C: boundary::ApiClient + Clone + Send + Sync,
    C::ConnectionHandle: Send,
    M: ConnectionManager,
{
    pub async fn new(
        boundary_client: C,
        user_id: String,
        connection_manager: M,
        remember_user_input: R,
        cross_term_event_rx: tokio::sync::mpsc::Receiver<Event>,
        clipboard: Box<dyn ClipboardAccess>,
    ) -> Self
    {
        let (message_tx, message_rx) = tokio::sync::mpsc::channel(1);
        let page =
            Page::Scopes(ScopesPage::new(None, message_tx.clone(), boundary_client.clone()).await);

        BountuiApp {
            boundary_client,
            user_id,
            page,
            history: vec![],
            connection_manager,
            alert: None,
            message_tx,
            message_rx,
            cross_term_event_rx,
            navigation_input: None,
            tasks: FuturesUnordered::new(),
            remember_user_input,
            clipboard,
        }
    }

    pub fn navigate_to(&mut self, page: Page<C, R>, replace_history: bool) {
        if replace_history {
            self.history.clear();
            self.page = page;
        } else {
            self.history.push(mem::replace(&mut self.page, page));
        }
    }

    async fn stop_session(&mut self, session_id: &str) -> Option<Message> {
        if let Err(e) = self.connection_manager.stop(session_id).await {
            return Some(Message::show_error("Failed to stop session", e));
        }
        None
    }

    async fn show_scope(&mut self, parent: Option<Scope>) {
        self.navigate_to(
            Page::Scopes(
                ScopesPage::new(
                    parent.as_ref(),
                    self.message_tx.clone(),
                    self.boundary_client.clone(),
                )
                    .await,
            ),
            false,
        );
    }

    async fn show_targets(&mut self, parent: Scope) {
        self.navigate_to(
            Page::Targets(
                TargetsPage::new(
                    parent,
                    self.message_tx.clone(),
                    self.boundary_client.clone(),
                    self.remember_user_input,
                )
                    .await,
            ),
            false,
        );
    }

    async fn navigate_to_scope_tree(&mut self) {
        self.navigation_input = None;
        self.navigate_to(
            Page::Scopes(
                ScopesPage::new(None, self.message_tx.clone(), self.boundary_client.clone()).await,
            ),
            true,
        );
    }

    async fn navigate_to_my_sessions(&mut self) {
        self.navigation_input = None;
        self.navigate_to(
            Page::UserSessions(
                SessionsPage::new(
                    Some("User"),
                    LoadUserSessions::new(
                        self.user_id.clone(),
                        self.boundary_client.clone(),
                        self.message_tx.clone(),
                    ),
                    self.message_tx.clone(),
                )
                    .await,
            ),
            true,
        );
    }

    fn go_back(&mut self) {
        if let Some(page) = self.history.pop() {
            self.page = page;
        }
    }

    async fn connect(&mut self, target_id: &String, port: u16) {
        match self.connection_manager.connect(target_id, port).await {
            Ok(resp) => {
                self.message_tx
                    .send(Message::Targets(TargetsPageMessage::ConnectedToTarget(
                        resp,
                    )))
                    .await
                    .unwrap();
            }
            Err(e) => {
                let _ = self
                    .message_tx
                    .send(Message::show_error("Connection Error", e));
            }
        }
    }

    pub fn view(&self, frame: &mut Frame) {
        if let Some((title, message)) = &self.alert {
            frame.render_widget(
                Alert::new(title.to_string(), message.to_string()),
                frame.area(),
            );
        }

        let layout_constraints = match self.navigation_input {
            Some(_) => {
                vec![Constraint::Length(3), Constraint::Fill(1)]
            }
            None => vec![Constraint::Length(0), Constraint::Fill(1)],
        };

        let [nav_input_area, content_area] =
            ratatui::layout::Layout::vertical(layout_constraints).areas(frame.area());

        if let Some(nav_input) = &self.navigation_input {
            nav_input.view(frame, nav_input_area);
        }

        match &self.page {
            Page::Scopes(scopes_page) => {
                scopes_page.view(frame, content_area);
            }
            Page::Targets(targets_page) => {
                targets_page.view(frame, content_area);
            }
            Page::TargetSessions(sessions_page) => {
                sessions_page.view(frame, content_area);
            }
            Page::UserSessions(sessions_page) => {
                sessions_page.view(frame, content_area);
            }
        }
    }

    pub async fn handle_event(&mut self, event: &Event) {
        if self.alert.is_some() && event.is_enter() {
            self.alert = None
        }

        match event {
            Event::Key(key_event) => match key_event.code {
                KeyCode::Char(':') => {
                    self.navigation_input = Some(NavigationInput::new(self.message_tx.clone()));
                    return;
                }
                KeyCode::Esc => {
                    if self.navigation_input.is_some() {
                        self.navigation_input = None;
                        return;
                    }
                }
                _ => {}
            },
            _ => {}
        }

        if let Some(nav_input) = &mut self.navigation_input {
            nav_input.handle_event(event).await;
            return;
        }

        match &mut self.page {
            Page::Scopes(scopes_page) => {
                scopes_page.handle_event(event).await;
            }
            Page::Targets(targets_page) => targets_page.handle_event(event).await,
            Page::TargetSessions(sessions_page) => sessions_page.handle_event(event).await,
            Page::UserSessions(sessions_page) => sessions_page.handle_event(event).await,
        }
    }

    pub async fn handle_message(&mut self, message: Message) {
        match message {
            Message::ShowScopes { parent } => self.show_scope(parent).await,
            Message::ShowTargets { parent } => self.show_targets(parent).await,
            Message::Connect { target_id, port } => self.connect(&target_id, port).await,
            Message::ShowSessions { scope, target } => {
                self.navigate_to(
                    Page::TargetSessions(
                        SessionsPage::new(
                            Some(target.name.as_str()),
                            LoadTargetSessionsSessions::new(
                                scope,
                                target.id,
                                self.boundary_client.clone(),
                                self.message_tx.clone(),
                            ),
                            self.message_tx.clone(),
                        )
                            .await,
                    ),
                    false,
                );
            }
            Message::StopSession {
                session_id,
                notify_stopped_tx,
            } => {
                self.stop_session(&session_id).await;
                let _ = notify_stopped_tx.send(()).await;
            }
            Message::ShowAlert(title, message) => {
                self.alert = Some((title.clone(), message.clone()));
            }
            Message::GoBack => self.go_back(),
            Message::Targets(targets_message) => {
                if let Page::Targets(targets_page) = &mut self.page {
                    targets_page.handle_message(targets_message);
                }
            }
            Message::SessionsPage(msg) => match &mut self.page {
                Page::TargetSessions(sessions_page) => {
                    sessions_page.handle_message(msg);
                }
                Page::UserSessions(sessions_page) => {
                    sessions_page.handle_message(msg);
                }
                _ => {}
            },
            Message::NavigateToScopeTree => {
                self.navigate_to_scope_tree().await;
            }
            Message::NavigateToMySessions => {
                self.navigate_to_my_sessions().await;
            }
            Message::RunFuture(future) => {
                self.tasks.push(future);
            }
            Message::Scopes(scopes_message) => {
                if let Page::Scopes(scopes_page) = &mut self.page {
                    scopes_page.handle_message(scopes_message).await;
                }
            }
            Message::SetClipboard(text) => {
                if let Err(e) = self.clipboard.set_text(text) {
                    self.alert = Some((
                        "Clipboard Error".to_string(),
                        format!("Failed to set clipboard text: {e}"),
                    ));
                }
            }
        }
    }

    pub async fn run(&mut self) {
        let mut terminal = ratatui::init();
        terminal.clear().unwrap();


        loop {
            terminal
                .draw(|frame| {
                    self.view(frame);
                })
                .unwrap();
            select! {
                message = self.message_rx.recv() => {
                    if let Some(message) = message {
                        self.handle_message(message).await;
                    }
                }
                event = self.cross_term_event_rx.recv() => {
                    if let Some(event) = event {
                        if event.is_stop() {
                            let _ = self.connection_manager.shutdown().await
                                .map_err(|e| error!("Failed to shutdown connection manager: {:?}", e));
                            break;
                        }
                        self.handle_event(&event).await;
                    }
                },
                _ = self.tasks.next() => {}
            }
        }

        ratatui::restore()
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::boundary::client::MockApiClient;
    use crate::bountui::connection_manager::MockConnectionManager;
    use crate::util::clipboard::{ClipboardAccessError, MockClipboardAccess};
    use mockall::predicate::eq;
    use std::sync::Arc;

    #[tokio::test]
    async fn set_clipboard_success_clears_alert() {
        let mut mock_clip = MockClipboardAccess::new();
        mock_clip
            .expect_set_text()
            .with(eq("hello".to_string()))
            .returning(|_| Ok(()));

        let boundary_client: Arc<MockApiClient> = Arc::new(MockApiClient::new());
        let connection_manager = MockConnectionManager::new();
        let (_evt_tx, evt_rx) = tokio::sync::mpsc::channel(1);
        let remember_user_input: Option<UserInputsPath<&'static str>> = None;

        let mut app = BountuiApp::new(
            boundary_client,
            "user-1".to_string(),
            connection_manager,
            remember_user_input,
            evt_rx,
            Box::new(mock_clip),
        ).await;

        app.handle_message(Message::SetClipboard("hello".to_string())).await;

        assert!(app.alert.is_none(), "Alert should not be set on clipboard success");
    }

    #[tokio::test]
    async fn set_clipboard_error_sets_alert() {
        let mut mock_clip = MockClipboardAccess::new();
        mock_clip
            .expect_set_text()
            .with(eq("oops".to_string()))
            .returning(|_| Err(ClipboardAccessError::Unknown("boom".to_string())));

        let boundary_client: Arc<MockApiClient> = Arc::new(MockApiClient::new());
        let connection_manager = MockConnectionManager::new();
        let (_evt_tx, evt_rx) = tokio::sync::mpsc::channel(1);
        let remember_user_input: Option<UserInputsPath<&'static str>> = None;

        let mut app = BountuiApp::new(
            boundary_client,
            "user-1".to_string(),
            connection_manager,
            remember_user_input,
            evt_rx,
            Box::new(mock_clip),
        ).await;

        app.handle_message(Message::SetClipboard("oops".to_string())).await;

        match &app.alert {
            Some((title, _msg)) => {
                assert_eq!(title, "Clipboard Error");
            }
            None => panic!("Expected clipboard error alert to be set"),
        }
    }
}
