use crate::boundary;
use crate::components::table::scope::ScopesPage;
use crate::components::table::sessions::SessionsPage;
use crate::components::table::target::TargetsPage;
use crate::components::Alerts;
use crate::connection_manager::ConnectionManager;
use crate::router::Router;
use crate::routes::Routes;
use crossterm::event::{Event, KeyCode};
use ratatui::Frame;


pub enum Pages<'a, C>
where
    C: boundary::ApiClient + Send + 'static,
{
    Scopes(ScopesPage<'a, C>),
    Targets(TargetsPage<'a, C>),
    Sessions(SessionsPage<'a, C>),
}

pub struct Bountui<'a, C>
where
    C: boundary::ApiClient + Send + 'static,
{
    boundary_client: &'a C,
    user_id: String,
    pub finished: bool,
    router: &'a Router<Routes>,
    page: Pages<'a, C>,
    connection_manager: &'a ConnectionManager<'a, C>,
    alerts: &'a Alerts,
}

impl<'a, T> Bountui<'a, T>
where
    T: boundary::ApiClient + Send + 'static,
{
    pub fn new(
        boundary_client: &'a T,
        user_id: String,
        router: &'a Router<Routes>,
        connection_manager: &'a ConnectionManager<T>,
        alerts: &'a Alerts,
    ) -> Self {
        let page = Pages::Scopes(ScopesPage::new(None, boundary_client, router, alerts));
        Bountui {
            boundary_client,
            user_id,
            router,
            page,
            finished: false,
            connection_manager,
            alerts,
        }
    }

    pub fn check_ctrl_c(&mut self, event: &Event) -> bool {
        if let Event::Key(key_event) = event {
            if key_event.code == KeyCode::Char('c')
                && key_event
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL)
            {
                self.finished = true;
                return true;
            }
        }
        false
    }

    fn show_scopes(&mut self, parent: Option<String>) {
        self.page = Pages::Scopes(ScopesPage::new(
            parent,
            self.boundary_client,
            self.router,
            self.alerts,
        ));
    }

    fn show_targets(&mut self, scope: String) {
        self.page = Pages::Targets(TargetsPage::new(
            Some(scope),
            self.boundary_client,
            self.router,
            self.connection_manager,
            self.alerts,
        ));
    }

    fn show_sessions(&mut self, scope_id: String, target_id: String) {
        self.page = Pages::Sessions(SessionsPage::new(
            self.router,
            self.boundary_client,
            self.connection_manager,
            self.alerts,
            self.user_id.clone(),
            scope_id,
            target_id,
        ));
    }

    fn poll_router_change(&mut self) {
        if let Some(new_route) = self.router.poll_change() {
            match new_route.as_ref() {
                Routes::Scopes { parent } => self.show_scopes(parent.clone()),
                Routes::Targets { scope } => self.show_targets(scope.clone()),
                Routes::Sessions {
                    scope_id,
                    target_id,
                } => self.show_sessions(scope_id.clone(), target_id.clone()),
            }
        }
    }

    pub fn handle_event(&mut self, event: &Event) {
        if self.check_ctrl_c(event) {
            return;
        }
        if self.alerts.handle_event(event).handled() {
            return;
        }
        let page_update_result = match &mut self.page {
            Pages::Scopes(page) => page.handle_event(event),
            Pages::Targets(page) => page.handle_event(event),
            Pages::Sessions(page) => page.handle_event(event),
        };
        if !page_update_result {
            if let Event::Key(key_event) = event {
                if key_event.code == KeyCode::Esc {
                    self.router.pop();
                }
            }
        }
        self.poll_router_change();
    }

    pub fn render(&self, frame: &mut Frame) {
        match &self.page {
            Pages::Scopes(page) => {
                page.render(frame);
            }
            Pages::Targets(page) => {
                page.render(frame);
            }
            Pages::Sessions(page) => {
                page.render(frame);
            }
        }
        self.alerts.render(frame);
    }
}
