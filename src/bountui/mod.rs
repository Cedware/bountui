use crate::boundary;
use crate::bountui::components::table::scope::ScopesPage;
use crate::bountui::components::table::sessions::{
    LoadTargetSessionsSessions, LoadUserSessions, SessionsPage, SessionsPageMessage,
};
use crate::bountui::components::table::target::{TargetsPage, TargetsPageMessage};
use crate::bountui::components::NavigationInput;
use crate::bountui::connection_manager::ConnectionManager;
use crate::bountui::widgets::Alert;
use crate::event_ext::EventExt;
use crossterm::event::{Event, KeyCode};
use ratatui::layout::Constraint;
use ratatui::Frame;
use std::fmt::Display;
use std::mem;
use crate::boundary::{Scope, Target};

pub mod components;
pub mod connection_manager;
mod widgets;



pub enum Message {
    ShowScopes {
        parent: Option<Scope>,
    },
    ShowTargets {
        parent: Option<Scope>,
    },
    ShowSessions {
        scope: String,
        target: Target
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
    SessionsPage(SessionsPageMessage),
    // Navigate root pages
    NavigateToScopeTree,
    NavigateToMySessions,
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
    TargetSessions(SessionsPage<LoadTargetSessionsSessions<B>>),
    UserSessions(SessionsPage<LoadUserSessions<B>>)
}

pub struct BountuiApp<C: boundary::ApiClient + Clone + Send + Sync + 'static> {
    page: Page<C>,
    boundary_client: C,
    history: Vec<Page<C>>,
    connection_manager: ConnectionManager<C>,
    alert: Option<(String, String)>,
    message_tx: tokio::sync::mpsc::Sender<Message>,
    pub is_finished: bool,
    user_id: String,
    navigation_input: Option<NavigationInput>,
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
        let scopes = boundary_client.get_scopes(None, false).await.unwrap();
        let page = Page::Scopes(ScopesPage::new(None, scopes, send_message.clone()));
        BountuiApp {
            boundary_client,
            user_id,
            page,
            history: vec![],
            connection_manager,
            alert: None,
            message_tx: send_message,
            is_finished: false,
            navigation_input: None,
        }
    }

    pub fn navigate_to(&mut self, page: Page<C>, replace_history: bool) {
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
        let parent_name = parent.as_ref().map(|p| p.name.as_str());
        let parent_id = parent.as_ref().map(|p| p.id.as_str());
        match self.boundary_client.get_scopes(parent_id, false).await {
            Ok(scopes) => {
                self.navigate_to(
                    Page::Scopes(ScopesPage::new(parent_name, scopes, self.message_tx.clone())),
                    false,
                );
            }
            Err(e) => {
                self.alert = Some(("Failed to load scopes".to_string(), format!("{:?}", e)));
            }
        }
    }

    async fn show_targets(&mut self, parent: Option<Scope>) {
        match self.boundary_client.get_targets(parent.as_ref().map(|s|s.id.as_str())).await {
            Ok(targets) => {
                self.navigate_to(
                    Page::Targets(TargetsPage::new(parent.as_ref().map(|s|s.name.as_str()), targets, self.message_tx.clone())),
                    false,
                );
            }
            Err(e) => {
                let _ = self
                    .message_tx
                    .send(Message::show_error("Failed to load targets", e));
            }
        }
    }

    async fn navigate_to_scope_tree(&mut self) {
        self.navigation_input = None;
        let scopes = self.boundary_client.get_scopes(None, false).await;
        match scopes {
            Ok(scopes) => {
                self.navigate_to(
                    Page::Scopes(ScopesPage::new(None, scopes, self.message_tx.clone())),
                    true,
                );
            }
            Err(e) => {
                self.alert = Some(("Failed to load scopes".to_string(), format!("{:?}", e)));
            }
        }
    }

    async fn navigate_to_my_sessions(&mut self) {
        self.navigation_input = None;
        self.navigate_to(
            Page::UserSessions(SessionsPage::new(
                Some("User"),
                LoadUserSessions::new(
                    self.user_id.clone(),
                    self.boundary_client.clone(),
                    self.message_tx.clone(),
                ),
                self.message_tx.clone(),
            ).await),
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
        if event.is_stop() {
            self.is_finished = true;
            return;
        }

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
            Message::ShowSessions {
                scope,
                target,
            } => {
                self.navigate_to(
                    Page::TargetSessions(SessionsPage::new(
                        Some(target.name.as_str()),
                        LoadTargetSessionsSessions::new(
                            scope,
                            target.id,
                            self.boundary_client.clone(),
                            self.message_tx.clone(),
                        ),
                        self.message_tx.clone(),
                    ).await),
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
            Message::SessionsPage(msg) => {
                match &mut self.page {
                    Page::TargetSessions(sessions_page) => {
                        sessions_page.handle_message(msg);
                    },
                    Page::UserSessions(sessions_page) => {
                        sessions_page.handle_message(msg);
                    },
                    _ => {}
                }

            },
            Message::NavigateToScopeTree => {
                self.navigate_to_scope_tree().await;
            },
            Message::NavigateToMySessions => {
                self.navigate_to_my_sessions().await;
            }
        }
    }
}
