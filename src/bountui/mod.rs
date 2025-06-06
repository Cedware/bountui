use crate::boundary;
use crate::bountui::components::table::scope::ScopesPage;
use crate::bountui::components::table::sessions::{ReloadScopeSessions, SessionsPage};
use crate::bountui::components::table::target::{TargetsPage, TargetsPageMessage};
use crate::bountui::connection_manager::ConnectionManager;
use crate::bountui::widgets::Alert;
use crate::event_ext::EventExt;
use crossterm::event::{Event};
use ratatui::Frame;
use std::fmt::Display;
use std::mem;

pub mod components;
pub mod connection_manager;
mod widgets;

pub enum Message {
    ShowScopes {
        parent: Option<String>,
    },
    ShowTargets {
        parent: Option<String>,
    },
    ShowSessions {
        scope: String,
        target_id: String,
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
    Targets(TargetsPageMessage),
    SessionsChanged(Vec<boundary::Session>),
}

impl Message {
    fn show_error<M: Into<String>, E: Display>(message: M, error: E) -> Message {
        Message::ShowAlert(
            "Error".to_string(),
            format!("{}: {}", message.into(), error),
        )
    }
}

pub enum Page<B: boundary::ApiClient + Send + Sync + 'static> {
    Scopes(ScopesPage),
    Targets(TargetsPage),
    ScopeSessions(SessionsPage<ReloadScopeSessions<B>>),
}

pub struct BountuiApp<C: boundary::ApiClient + Clone + Send + Sync + 'static> {
    page: Page<C>,
    boundary_client: C,
    history: Vec<Page<C>>,
    connection_manager: ConnectionManager<C>,
    alert: Option<(String, String)>,
    message_tx: tokio::sync::mpsc::Sender<Message>,
    pub is_finished: bool,
    user_id: String
}

impl<C> BountuiApp<C>
where
    C: boundary::ApiClient + Clone + Send + Sync,
{
    pub async fn new(
        boundary_client: C,
        user_id: String,
        connection_manager: ConnectionManager<C>,
        send_message: tokio::sync::mpsc::Sender<Message>,
    ) -> Self
    where
        C: boundary::ApiClient,
    {
        let scopes = boundary_client.get_scopes(&None, false).await.unwrap();
        let page = Page::Scopes(ScopesPage::new(scopes, send_message.clone()));
        BountuiApp {
            boundary_client,
            user_id,
            page,
            history: vec![],
            connection_manager,
            alert: None,
            message_tx: send_message,
            is_finished: false,
        }
    }

    pub fn navigate_to(&mut self, page: Page<C>) {
        self.history.push(mem::replace(&mut self.page, page));
    }

    async fn stop_session(&mut self, session_id: &str) -> Option<Message> {
        if let Err(e) = self.connection_manager.stop(session_id).await {
            return Some(Message::show_error("Failed to stop session", e));
        }
        None
    }

    async fn show_scope(&mut self, parent: &Option<String>) {
        match self.boundary_client.get_scopes(parent, false).await {
            Ok(scopes) => {
                self.navigate_to(Page::Scopes(ScopesPage::new(
                    scopes,
                    self.message_tx.clone()
                )));
            }
            Err(e) => {
                self.alert = Some(("Failed to load scopes".to_string(), format!("{:?}", e)));
            }
        }
    }

    async fn show_targets(&mut self, parent: &Option<String>) {
        match self.boundary_client.get_targets(parent).await {
            Ok(targets) => {
                self.navigate_to(Page::Targets(TargetsPage::new(
                    targets,
                    self.message_tx.clone(),
                )));
            }
            Err(e) => {
                let _ = self
                    .message_tx
                    .send(Message::show_error("Failed to load targets", e));
            }
        }
    }

    // async fn show_user_sessions(&mut self) {
    //     match self.boundary_client.get_user_sessions(&self.user_id).await {
    //         Ok(sessions) => {
    //             self.navigate_to(Page::Sessions(SessionsPage::new(
    //                 sessions,
    //                 self.message_tx.clone(),
    //                 ReloadScopeSessions::new()
    //             )))
    //         }
    //         Err(e) => {
    //             let _ = self
    //                 .message_tx
    //                 .send(Message::show_error("Failed to load user sessions", e));
    //         }
    //     }
    // }

    fn go_back(&mut self) {
        if let Some(page) = self.history.pop() {
            self.page = page;
        }
    }

    fn handle_session_changed(&mut self, sessions: Vec<boundary::Session>) {
        if let Page::ScopeSessions(sessions_page) = &mut self.page {
            sessions_page.set_sessions(sessions);
        }
    }

    async fn connect(
        &mut self,
        target_id: &String,
        port: u16
    ) {
        match self.connection_manager.connect(target_id, port).await {
            Ok(resp) => {
                self.message_tx.send(Message::Targets(TargetsPageMessage::ConnectedToTarget(resp))).await.unwrap();
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

        match &self.page {
            Page::Scopes(scopes_page) => {
                scopes_page.view(frame);
            }
            Page::Targets(targets_page) => {
                targets_page.view(frame);
            }
            Page::ScopeSessions(sessions_page) => {
                sessions_page.view(frame);
            }
        }
    }

    pub async fn handle_event(&mut self, event: &Event) {

        if event.is_stop() {
            self.is_finished = true;
            return;
        }
        
        if self.alert.is_some() && event.is_enter() {
            self.alert = None
        }

        // match event {
        //     Event::Key(key_event) if key_event.code == KeyCode::Char('M') => {
        //         self.show_user_sessions().await;
        //         return;
        //     }
        //     _ => {}
        // }

        match &mut self.page {
            Page::Scopes(scopes_page) => {
                scopes_page.handle_event(event).await;
            }
            Page::Targets(targets_page) => targets_page.handle_event(event).await,
            Page::ScopeSessions(sessions_page) => sessions_page.handle_event(event).await,
        }
    }

    pub async fn handle_message(&mut self, message: Message) {
        match message {
            Message::ShowScopes { parent } => self.show_scope(&parent).await,
            Message::ShowTargets { parent } => self.show_targets(&parent).await,
            Message::Connect {
                target_id,
                port,
            } => self.connect(&target_id, port).await,
            Message::ShowSessions {
                scope,
                target_id: target,
            } => {
                let sessions = self
                    .boundary_client
                    .get_sessions(&scope)
                    .await
                    .unwrap()
                    .iter()
                    .filter(|s| s.target_id == *target)
                    .cloned()
                    .collect();
                self.navigate_to(Page::ScopeSessions(SessionsPage::new(
                    sessions,
                    ReloadScopeSessions::new(scope, self.boundary_client.clone(), self.message_tx.clone()),
                    self.message_tx.clone(),
                )));
            }
            Message::StopSession { session_id, notify_stopped_tx } => {
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
            },
            Message::SessionsChanged(sessions_page) => {
                self.handle_session_changed(sessions_page);
            }
        }
    }
}