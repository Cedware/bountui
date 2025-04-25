use std::fmt::Display;
use std::mem;
use crossterm::event::Event;
use futures::FutureExt;
use mockall::Any;
use ratatui::Frame;
use crate::appframework::{Application, Component, UpdateState};
use crate::boundary;
use crate::bountui::components::table::scope::{ScopesMessage, ScopesPage};
use crate::bountui::components::table::sessions::{SessionsMessage, SessionsPage};
use crate::bountui::components::table::target::{TargetsMessage, TargetsPage};
use crate::bountui::connection_manager::ConnectionManager;
use crate::bountui::widgets::Alert;
use crate::event_ext::EventExt;

mod app;
pub mod components;
pub mod connection_manager;
pub mod router;
pub mod routes;
mod widgets;
mod state;

pub enum Message {
    ShowScopes{
        parent: Option<String>
    },
    ShowTargets {
        parent: Option<String>
    },
    ShowSessions {
        scope: String,
        target_id: String,
    },
    GoBack,
    Connect {
        target_id: String,
        port: u16
    },
    StopSession {
        session_id: String
    },
    ShowAlert(String, String),
    Targets(TargetsMessage),
    Scopes(ScopesMessage),
    Session(SessionsMessage),
    CloseAlert
}

impl Message {

    fn show_error<M: Into<String>, E: Display>(message: M, error: E) -> Message {
        Message::ShowAlert(
            "Error".to_string(),
            format!("{}: {}", message.into(), error)
        )
    }

}

pub enum Page {
    Scopes(ScopesPage),
    Targets(TargetsPage),
    Sessions(SessionsPage)
}


pub struct BountuiApp<C> {
    page: Page,
    boundary_client: C,
    history: Vec<Page>,
    connection_manager: ConnectionManager<C>,
    alert: Option<(String, String)>
}

impl <C> BountuiApp<C> where C: boundary::ApiClient {
    pub async fn new(boundary_client: C, connection_manager: ConnectionManager<C>) -> Self where C: boundary::ApiClient{
        let scopes = boundary_client.get_scopes(&None).await.unwrap();
        let page = Page::Scopes(ScopesPage::new(scopes));
        BountuiApp {
            boundary_client,
            page,
            history: vec![],
            connection_manager,
            alert: None
        }
    }

    async fn update_sessions(&mut self) -> Option<Message> {
        if let Page::Sessions(sessions_page) = &mut self.page {
            match self.boundary_client.get_sessions(&sessions_page.scope_id).await {
                Ok(sessions) => {
                    sessions_page.set_sessions(sessions);
                    None
                },
                Err(e) => Some(Message::show_error("Failed to load sessions", e))
            }
        }
        else {
            None
        }
    }

    pub fn navigate_to(&mut self, page: Page) {
        self.history.push(mem::replace(&mut self.page, page));
    }

    async fn stop_session(&mut self, session_id: &str) -> Option<Message> {

        match self.connection_manager.stop(session_id).await {
            Ok(_) => {
                self.update_sessions().await
            },
            Err(e) => {
                Some(Message::show_error("Failed to stop session", e))
            }
        }
    }

    async fn show_scope(&mut self, parent: &Option<String>) -> Option<Message> {
        match self.boundary_client.get_scopes(parent).await {
            Ok(scopes) => {
                self.navigate_to(Page::Scopes(ScopesPage::new(scopes)));
                None
            },
            Err(e) => Some(Message::show_error("Faled to load scopes", e))
        }
    }

    async fn show_targets(&mut self, parent: &Option<String>) -> Option<Message> {
        match self.boundary_client.get_targets(parent).await {
            Ok(targets) => {
                self.navigate_to(Page::Targets(TargetsPage::new(targets)));
                None
            },
            Err(e) => Some(Message::show_error("Failed to load targets", e))
        }
    }

    fn go_back(&mut self) -> Option<Message> {
        if let Some(page) = self.history.pop() {
            self.page = page;
        }
        None
    }

    async fn connect(&mut self, target_id: &String, port: u16) -> Option<Message> {
        match self.connection_manager.connect(target_id, port).await {
            Ok(resp) => {
                Some(TargetsMessage::Connected(resp).into())
            }
            Err(e) => {
                Some(Message::show_error(
                    format!("Failed to connect to target {target_id}"),
                    e
                ))
            }
        }
    }


}

impl <C> Component<Message> for BountuiApp<C> where C:boundary::ApiClient {
    fn view(&self, frame: &mut Frame) {
        
        if let Some((title, message)) = &self.alert {
            frame.render_widget(Alert::new(title.to_string(), message.to_string()), frame.area());
        }
        
        match &self.page {
            Page::Scopes(scopes_page) => {
                scopes_page.view(frame);
            },
            Page::Targets(targets_page) => {
                targets_page.view(frame);
            },
            Page::Sessions(sessions_page) => {
                sessions_page.view(frame);
            }
        }
    }

    fn handle_event(&self, event: &Event) -> Option<Message> {

        if self.alert.is_some() && event.is_enter() {
            return Some(Message::CloseAlert);
        }

        let message = match &self.page {
            Page::Scopes(scopes_page) => {
                scopes_page.handle_event(event)
            },
            Page::Targets(targets_page) => {
                targets_page.handle_event(event)
            },
            Page::Sessions(sessions_page) => {
                sessions_page.handle_event(event)
            }
        };
        match message { 
            Some(m) => Some(m),
            None => {
                if let Event::Key(key_event) = event {
                    if key_event.code == crossterm::event::KeyCode::Esc {
                        Some(Message::GoBack)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
        }
    }



}

impl <C> UpdateState<Message, Message> for BountuiApp<C> where C: boundary::ApiClient + Clone {
    async fn update(&mut self, message: &Message) -> Option<Message> {
        match message {
            Message::Scopes(scopes_message) => {
                if let Page::Scopes(scopes_page) = &mut self.page {
                    scopes_page.update(scopes_message).await
                } else { None }
            },
            Message::Targets(targets_message) => {
                if let Page::Targets(targets_page) = &mut self.page {
                    targets_page.update(targets_message).await
                } else { None }
            },
            Message::Session(sessions_message) => {
                if let Page::Sessions(sessions_page) = &mut self.page {
                    sessions_page.update(sessions_message).await
                } else { None }
            },
            Message::ShowScopes { parent } => self.show_scope(parent).await,
            Message::ShowTargets { parent } => self.show_targets(parent).await,
            Message::GoBack => self.go_back(),
            Message::Connect { target_id, port } => self.connect(target_id, *port).await,
            Message::ShowSessions { scope, target_id: target } => {
                let sessions = self.boundary_client.get_sessions(scope).await.unwrap()
                    .iter().filter(|s| s.target_id == *target).cloned().collect();
                self.navigate_to(Page::Sessions(SessionsPage::new(scope.clone(), target.clone(), sessions)));
                None
            },
            Message::StopSession { session_id } => {
                self.stop_session(session_id).await;
                None
            },
            Message::ShowAlert(title, message) => {
                self.alert = Some((title.clone(), message.clone()));
                None
            },
            Message::CloseAlert => {
                self.alert = None;
                None
            }
        }
    }
}

impl <C> Application<Message> for BountuiApp<C> where C: boundary::ApiClient + Clone {

}