use crate::boundary;
use crate::boundary::{AuthenticateResponse, Scope, Target};
use crate::bountui::components::table::scope::{ScopesPage, ScopesPageMessage};
use crate::bountui::components::table::sessions::{
    LoadTargetSessionsSessions, LoadUserSessions, SessionsPage, SessionsPageMessage,
};
use crate::bountui::components::table::target::{TargetsPage, TargetsPageMessage};
use crate::bountui::components::NavigationInput;
use crate::bountui::connection_manager::ConnectionManager;
use crate::bountui::loading_page::LoadingPage;
use crate::bountui::login_page::LoginPage;
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

pub mod auth_cache;
pub mod components;
pub mod connection_manager;
mod loading_page;
mod login_page;
mod remember_user_input;
mod widgets;

pub use auth_cache::AuthCache;

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
    SetClipboard {
        text: String,
        on_success: Option<Box<Message>>,
        on_error: Option<Box<Message>>,
    },
    Targets(TargetsPageMessage),
    Scopes(ScopesPageMessage),
    SessionsPage(SessionsPageMessage),
    // Navigate root pages
    NavigateToScopeTree,
    NavigateToMySessions,
    RunFuture(BoxFuture<'static, ()>),
    Toaster(components::toaster::Message),
    Authenticated(AuthenticateResponse),
    /// Sent during startup after the cached token was successfully validated against the API.
    TokenRestored(AuthenticateResponse),
    /// Sent during startup when the cached token failed validation (expired / revoked).
    TokenInvalid,
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
    Loading(LoadingPage),
    Login(LoginPage<B>),
    Scopes(ScopesPage),
    Targets(TargetsPage<B, R>),
    TargetSessions(SessionsPage<LoadTargetSessionsSessions<B>>),
    UserSessions(SessionsPage<LoadUserSessions<B>>),
}

pub struct BountuiApp<
    C: boundary::ApiClient + Clone + Send + Sync + 'static,
    R: RememberUserInput + Copy,
    M: ConnectionManager
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
    toaster: components::toaster::Toaster,
    auth_cache: Box<dyn AuthCache>,
    frame_count: u64,
}

impl<C, R: RememberUserInput + Copy, M> BountuiApp<C, R, M>
where
    C: boundary::ApiClient + Clone + Send + Sync,
    C::ConnectionHandle: Send,
    M: ConnectionManager
{
    pub fn new(
        boundary_client: C,
        connection_manager: M,
        remember_user_input: R,
        cross_term_event_rx: tokio::sync::mpsc::Receiver<Event>,
        clipboard: Box<dyn ClipboardAccess>,
        auth_cache: Box<dyn AuthCache>,
    ) -> Self {
        let (message_tx, message_rx) = tokio::sync::mpsc::channel(64);

        let (page, user_id) = Self::resolve_initial_page(
            &auth_cache,
            &message_tx,
            &boundary_client,
        );

        BountuiApp {
            boundary_client,
            user_id,
            page,
            history: vec![],
            connection_manager,
            alert: None,
            message_tx: message_tx.clone(),
            message_rx,
            cross_term_event_rx,
            navigation_input: None,
            tasks: FuturesUnordered::new(),
            remember_user_input,
            clipboard,
            toaster: components::toaster::Toaster::new(message_tx),
            auth_cache,
            frame_count: 0,
        }
    }

    fn resolve_initial_page(
        auth_cache: &Box<dyn AuthCache>,
        message_tx: &tokio::sync::mpsc::Sender<Message>,
        boundary_client: &C,
    ) -> (Page<C, R>, String) {
        if let Some(cached) = auth_cache.get_cached_token() {
            unsafe {
                std::env::set_var("BOUNDARY_TOKEN", &cached.token);
            }
            let user_id = cached.user_id.clone();
            let tx = message_tx.clone();
            let auth_response = AuthenticateResponse {
                attributes: boundary::client::response::AuthenticateAttributes {
                    user_id: cached.user_id,
                    token: cached.token,
                },
            };
            let client = boundary_client.clone();
            tokio::spawn(async move {
                match client.get_scopes(None, false).await {
                    Ok(_) => {
                        log::info!("auth_cache: cached token is valid — restoring session");
                        let _ = tx.send(Message::TokenRestored(auth_response)).await;
                    }
                    Err(e) => {
                        log::warn!("auth_cache: cached token validation failed: {e} — falling back to login");
                        let _ = tx.send(Message::TokenInvalid).await;
                    }
                }
            });
            (Page::Loading(LoadingPage), user_id)
        } else {
            (
                Page::Login(LoginPage::new(boundary_client.clone(), message_tx.clone())),
                String::new(),
            )
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

    async fn stop_session(&mut self, session_id: &str) {
        if let Err(e) = self.connection_manager.stop(session_id).await {
            error!("Failed to stop session: {:?}", e);
            self.message_tx
                .send(Message::show_error("Failed to stop session", e))
                .await
                .expect("Failed to send stop session error message");
        }
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
        let credentials = self.connection_manager.get_credentials();
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
                    credentials,
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
                    .send(Message::show_error("Connection Error", e))
                    .await;
            }
        }
    }

    fn handle_layout(&mut self, terminal: &mut ratatui::Terminal<impl ratatui::backend::Backend>) {
        let terminal_size = terminal.size().unwrap();
        let frame_area = ratatui::layout::Rect {
            x: 0,
            y: 0,
            width: terminal_size.width,
            height: terminal_size.height,
        };
        self.toaster.layout(frame_area);
    }

    pub fn view(&mut self, frame: &mut Frame) {
        if let Some((title, message)) = &self.alert {
            frame.render_widget(
                widgets::Alert::new(title.to_string(), message.to_string()),
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
            Page::Loading(_) => {
                self.frame_count = self.frame_count.wrapping_add(1);
                let loading_screen = widgets::LoadingScreen {
                    frame_count: self.frame_count,
                };
                frame.render_widget(loading_screen, content_area);
            }
            Page::Login(_) => {
                frame.render_widget(widgets::LoginScreen, content_area);
            }
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

        // Render toasts overlaying the content at the bottom
        self.toaster.view(frame);
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
            Page::Loading(_) => {}
            Page::Login(_) => {}
            Page::Scopes(scopes_page) => {
                scopes_page.handle_event(event).await;
            }
            Page::Targets(targets_page) => targets_page.handle_event(event).await,
            Page::TargetSessions(sessions_page) => {
                sessions_page.handle_event(event).await;
            }
            Page::UserSessions(sessions_page) => {
                sessions_page.handle_event(event).await;
            }
        }
    }

    pub async fn handle_message(&mut self, message: Message) {
        match message {
            Message::ShowScopes { parent } => self.show_scope(parent).await,
            Message::ShowTargets { parent } => self.show_targets(parent).await,
            Message::Connect { target_id, port } => self.connect(&target_id, port).await,
            Message::ShowSessions { scope, target } => {
                let credentials = self.connection_manager.get_credentials();
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
                            credentials,
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
            Message::SetClipboard {
                text,
                on_success,
                on_error,
            } => match self.clipboard.set_text(text) {
                Ok(_) => {
                    if let Some(success_msg) = on_success {
                        let _ = self.message_tx.send(*success_msg).await;
                    }
                }
                Err(e) => {
                    if let Some(error_msg) = on_error {
                        let _ = self.message_tx.send(*error_msg).await;
                    } else {
                        self.alert = Some((
                            "Clipboard Error".to_string(),
                            format!("Failed to set clipboard text: {e}"),
                        ));
                    }
                }
            },
            Message::Toaster(toaster_message) => {
                self.toaster.handle_message(toaster_message).await;
            }
            Message::Authenticated(auth_response) => {
                unsafe {
                    std::env::set_var("BOUNDARY_TOKEN", &auth_response.attributes.token);
                }
                self.user_id = auth_response.attributes.user_id.clone();

                // Cache the token after a successful login.
                if self.auth_cache.is_available() {
                    if let Err(e) = self.auth_cache.cache_token(
                        &auth_response.attributes.token,
                        &auth_response.attributes.user_id,
                    ) {
                        log::error!("Failed to cache auth token: {e}");
                    }
                }

                self.navigate_to_scope_tree().await;
            }
            Message::TokenRestored(auth_response) => {
                // Token was validated — same setup as a fresh login, but without re-caching.
                unsafe {
                    std::env::set_var("BOUNDARY_TOKEN", &auth_response.attributes.token);
                }
                self.user_id = auth_response.attributes.user_id.clone();
                self.navigate_to_scope_tree().await;
            }
            Message::TokenInvalid => {
                // Cached token is expired or revoked — clear it and start the login flow.
                if let Err(e) = self.auth_cache.clear_cache() {
                    log::error!("auth_cache: failed to clear invalid token from keyring: {e}");
                }
                self.user_id = String::new();
                self.page = Page::Login(LoginPage::new(
                    self.boundary_client.clone(),
                    self.message_tx.clone(),
                ));
            }
        }
    }

    #[cfg(test)]
    async fn process_pending_messages(&mut self) {
        while let Ok(message) = self.message_rx.try_recv() {
            self.handle_message(message).await;
        }
    }

    pub async fn run(&mut self) {
        let mut terminal = ratatui::init();
        terminal.clear().unwrap();

        // Perform initial layout
        self.handle_layout(&mut terminal);

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
                        if event.is_resize() {
                            self.handle_layout(&mut terminal);
                        }
                        else {
                            self.handle_event(&event).await;
                        }

                    }
                },
                _ = self.tasks.next(), if !self.tasks.is_empty() => {}
            }
        }

        ratatui::restore()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bountui::auth_cache::tests::MockAuthCache;
    use crate::bountui::connection_manager::{DefaultConnectionManager, MockConnectionManager};
    use crate::util::clipboard::{ClipboardAccessError, MockClipboardAccess};
    use mockall::predicate::eq;
    use std::collections::HashMap;

    fn make_boundary_client() -> boundary::MockClient {
        boundary::MockClient::builder()
            .user_id("user-1".to_string())
            .scopes(HashMap::new())
            .build()
    }

    fn noop_auth_cache() -> Box<dyn AuthCache> {
        Box::new(MockAuthCache::without_cache())
    }

    async fn make_authenticated_app<M: ConnectionManager>(
        connection_manager: M,
        clipboard: Box<dyn ClipboardAccess>,
    ) -> BountuiApp<boundary::MockClient, Option<UserInputsPath<&'static str>>, M> {
        let (_evt_tx, evt_rx) = tokio::sync::mpsc::channel(1);
        let remember_user_input: Option<UserInputsPath<&'static str>> = None;

        let mut app = BountuiApp::new(
            make_boundary_client(),
            connection_manager,
            remember_user_input,
            evt_rx,
            clipboard,
            noop_auth_cache(),
        );

        for _ in 0..10 {
            app.process_pending_messages().await;
            if matches!(app.page, Page::Scopes(_)) {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        app
    }

    #[tokio::test]
    async fn failed_authentication_keeps_login_page_open_and_shows_alert() {
        let connection_manager = MockConnectionManager::new();
        let (_evt_tx, evt_rx) = tokio::sync::mpsc::channel(1);
        let remember_user_input: Option<UserInputsPath<&'static str>> = None;

        let mut app = BountuiApp::new(
            boundary::MockClient::builder()
                .user_id("user-1".to_string())
                .authenticate_should_fail(true)
                .scopes(HashMap::new())
                .build(),
            connection_manager,
            remember_user_input,
            evt_rx,
            Box::new(MockClipboardAccess::new()),
            noop_auth_cache(),
        );

        for _ in 0..10 {
            app.process_pending_messages().await;
            if app.alert.is_some() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        assert!(matches!(app.page, Page::Login(_)));
        assert!(app.alert.is_some(), "Expected authentication failure alert");
    }

    #[tokio::test]
    async fn set_clipboard_success_clears_alert() {
        let mut mock_clip = MockClipboardAccess::new();
        mock_clip
            .expect_set_text()
            .with(eq("hello".to_string()))
            .returning(|_| Ok(()));

        let connection_manager = MockConnectionManager::new();
        let mut app = make_authenticated_app(connection_manager, Box::new(mock_clip)).await;

        app.handle_message(Message::SetClipboard {
            text: "hello".to_string(),
            on_success: None,
            on_error: None,
        })
        .await;

        assert!(
            app.alert.is_none(),
            "Alert should not be set on clipboard success"
        );
    }

    #[tokio::test]
    async fn set_clipboard_error_sets_alert() {
        let mut mock_clip = MockClipboardAccess::new();
        mock_clip
            .expect_set_text()
            .with(eq("oops".to_string()))
            .returning(|_| Err(ClipboardAccessError::Unknown("boom".to_string())));

        let connection_manager = MockConnectionManager::new();
        let mut app = make_authenticated_app(connection_manager, Box::new(mock_clip)).await;

        app.handle_message(Message::SetClipboard {
            text: "oops".to_string(),
            on_success: None,
            on_error: None,
        })
        .await;

        match &app.alert {
            Some((title, _msg)) => {
                assert_eq!(title, "Clipboard Error");
            }
            None => panic!("Expected clipboard error alert to be set"),
        }
    }

    #[tokio::test]
    async fn connect_shows_error_when_connect_fails() {
        let boundary_client = make_boundary_client();
        let connection_manager = DefaultConnectionManager::new(boundary_client);

        let mut app =
            make_authenticated_app(connection_manager, Box::new(MockClipboardAccess::new())).await;

        app.handle_message(Message::Connect {
            target_id: "TARGET_DOES_NOT_EXIST".to_string(),
            port: 8080,
        })
        .await;
        for _ in 0..10 {
            app.process_pending_messages().await;
            if !matches!(app.page, Page::Login(_)) || app.alert.is_some() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        assert!(
            app.alert.is_some(),
            "Expected error alert on connect failure"
        );
    }
}
