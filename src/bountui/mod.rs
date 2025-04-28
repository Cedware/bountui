use crate::boundary;
use crate::boundary::ConnectResponse;
use crate::bountui::components::table::scope::ScopesPage;
use crate::bountui::components::table::sessions::SessionsPage;
use crate::bountui::components::table::target::{TargetsPage, TargetsPageMessage};
use crate::bountui::connection_manager::ConnectionManager;
use crate::bountui::widgets::Alert;
use crate::event_ext::EventExt;
use crossterm::event::Event;
use ratatui::Frame;
use std::fmt::{format, Display};
use std::mem;

mod app;
pub mod components;
pub mod connection_manager;
pub mod router;
pub mod routes;
mod state;
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
    GoBack,
    Connect {
        target_id: String,
        port: u16,
        respond_to: tokio::sync::oneshot::Sender<ConnectResponse>,
    },
    StopSession {
        session_id: String,
    },
    ShowAlert(String, String),
    CloseAlert,
    Targets(TargetsPageMessage),
}

impl Message {
    fn show_error<M: Into<String>, E: Display>(message: M, error: E) -> Message {
        Message::ShowAlert(
            "Error".to_string(),
            format!("{}: {}", message.into(), error),
        )
    }
}

pub enum Page {
    Scopes(ScopesPage),
    Targets(TargetsPage),
    Sessions(SessionsPage),
}

pub struct BountuiApp<C> {
    page: Page,
    boundary_client: C,
    history: Vec<Page>,
    connection_manager: ConnectionManager<C>,
    alert: Option<(String, String)>,
    send_message: tokio::sync::mpsc::Sender<Message>,
}

impl<C> BountuiApp<C>
where
    C: boundary::ApiClient,
{
    pub async fn new(
        boundary_client: C,
        connection_manager: ConnectionManager<C>,
        send_message: tokio::sync::mpsc::Sender<Message>,
    ) -> Self
    where
        C: boundary::ApiClient,
    {
        let scopes = boundary_client.get_scopes(&None).await.unwrap();
        let page = Page::Scopes(ScopesPage::new(scopes, send_message.clone()));
        BountuiApp {
            boundary_client,
            page,
            history: vec![],
            connection_manager,
            alert: None,
            send_message,
        }
    }

    async fn update_sessions(&mut self) -> Option<Message> {
        if let Page::Sessions(sessions_page) = &mut self.page {
            match self
                .boundary_client
                .get_sessions(&sessions_page.scope_id)
                .await
            {
                Ok(sessions) => {
                    sessions_page.set_sessions(sessions);
                    None
                }
                Err(e) => Some(Message::show_error("Failed to load sessions", e)),
            }
        } else {
            None
        }
    }

    pub fn navigate_to(&mut self, page: Page) {
        self.history.push(mem::replace(&mut self.page, page));
    }

    async fn stop_session(&mut self, session_id: &str) -> Option<Message> {
        match self.connection_manager.stop(session_id).await {
            Ok(_) => self.update_sessions().await,
            Err(e) => Some(Message::show_error("Failed to stop session", e)),
        }
    }

    async fn show_scope(&mut self, parent: &Option<String>) {
        match self.boundary_client.get_scopes(parent).await {
            Ok(scopes) => {
                self.navigate_to(Page::Scopes(ScopesPage::new(
                    scopes,
                    self.send_message.clone(),
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
                    self.send_message.clone(),
                )));
            }
            Err(e) => {
                let _ = self
                    .send_message
                    .send(Message::show_error("Failed to load targets", e));
            }
        }
    }

    fn go_back(&mut self) {
        if let Some(page) = self.history.pop() {
            self.page = page;
        }
    }

    async fn connect(
        &mut self,
        target_id: &String,
        port: u16,
        respond_to: tokio::sync::oneshot::Sender<ConnectResponse>,
    ) {
        match self.connection_manager.connect(target_id, port).await {
            Ok(resp) => {
                self.send_message.send(Message::Targets(TargetsPageMessage::ConnectedToTarget(resp))).await.unwrap();
            }
            Err(e) => {
                let _ = self
                    .send_message
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
            Page::Sessions(sessions_page) => {
                sessions_page.view(frame);
            }
        }
    }

    pub async fn handle_event(&mut self, event: &Event) {

        if let Event::Key(key_event) = event {
            if key_event.code == crossterm::event::KeyCode::Esc {
                self.go_back();
                return;
            }
        }

        if self.alert.is_some() && event.is_enter() {
            self.alert = None
        }

        match &mut self.page {
            Page::Scopes(scopes_page) => {
                scopes_page.handle_event(event).await;
            }
            Page::Targets(targets_page) => targets_page.handle_event(event).await,
            Page::Sessions(sessions_page) => sessions_page.handle_event(event).await,
        }
    }

    pub async fn handle_message(&mut self, message: Message) {
        match message {
            Message::ShowScopes { parent } => self.show_scope(&parent).await,
            Message::ShowTargets { parent } => self.show_targets(&parent).await,
            Message::GoBack => self.go_back(),
            Message::Connect {
                target_id,
                port,
                respond_to,
            } => self.connect(&target_id, port, respond_to).await,
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
                self.navigate_to(Page::Sessions(SessionsPage::new(
                    scope.clone(),
                    target.clone(),
                    sessions,
                    self.send_message.clone(),
                )));
            }
            Message::StopSession { session_id } => {
                self.stop_session(&session_id).await;
            }
            Message::ShowAlert(title, message) => {
                self.alert = Some((title.clone(), message.clone()));
            }
            Message::CloseAlert => {
                self.alert = None;
            },
            Message::Targets(targets_message) => {
                if let Page::Targets(targets_page) = &mut self.page {
                    targets_page.handle_message(targets_message);
                }
            }
        }
    }
}

// impl <C> UpdateState<Message, Message> for BountuiApp<C> where C: boundary::ApiClient + Clone {
//     async fn update(&mut self, message: &Message) -> Option<Message> {
//         match message {
//             Message::Scopes(scopes_message) => {
//                 if let Page::Scopes(scopes_page) = &mut self.page {
//                     scopes_page.update(scopes_message).await
//                 } else { None }
//             },
//             Message::Targets(targets_message) => {
//                 if let Page::Targets(targets_page) = &mut self.page {
//                     targets_page.update(targets_message).await
//                 } else { None }
//             },
//             Message::Session(sessions_message) => {
//                 if let Page::Sessions(sessions_page) = &mut self.page {
//                     sessions_page.update(sessions_message).await
//                 } else { None }
//             },
//             Message::ShowScopes { parent } => self.show_scope(parent).await,
//             Message::ShowTargets { parent } => self.show_targets(parent).await,
//             Message::GoBack => self.go_back(),
//             Message::Connect { target_id, port } => self.connect(target_id, *port).await,
//             Message::ShowSessions { scope, target_id: target } => {
//                 let sessions = self.boundary_client.get_sessions(scope).await.unwrap()
//                     .iter().filter(|s| s.target_id == *target).cloned().collect();
//                 self.navigate_to(Page::Sessions(SessionsPage::new(scope.clone(), target.clone(), sessions)));
//                 None
//             },
//             Message::StopSession { session_id } => {
//                 self.stop_session(session_id).await;
//                 None
//             },
//             Message::ShowAlert(title, message) => {
//                 self.alert = Some((title.clone(), message.clone()));
//                 None
//             },
//             Message::CloseAlert => {
//                 self.alert = None;
//                 None
//             }
//         }
//     }
// }

// impl <C> Application<Message> for BountuiApp<C> where C: boundary::ApiClient + Clone {
//
// }
