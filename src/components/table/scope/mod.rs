use crate::boundary;
use crate::boundary::Scope;
use crate::components::table::commands::{Command, HasCommands};
use crate::components::table::{FilterItems, SortItems, TableColumn};
use crate::components::{Alerts, TablePage};
use crate::router::Router;
use crate::routes::Routes;
use crossterm::event::Event;
use ratatui::layout::Constraint;
use ratatui::Frame;
use std::cell::RefCell;
use std::rc::Rc;

pub struct ScopesPage<'a, C>
where
    C: boundary::ApiClient,
{
    parent_scope_id: Option<String>,
    boundary_client: &'a C,
    table_page: TablePage<'a, boundary::Scope>,
    handle: tokio::runtime::Handle,
    router: &'a RefCell<Router<Routes>>,
    alerts: &'a Alerts,
}

#[derive(Debug, Clone, Copy)]
pub enum Commands {
    ListScopes,
    ListTargets,
}

impl HasCommands for boundary::Scope {
    type Id = Commands;

    fn commands() -> Vec<Command<Self::Id>> {
        vec![
            Command::new(
                Commands::ListScopes,
                "List scopes".to_string(),
                "s".to_string(),
            ),
            Command::new(
                Commands::ListTargets,
                "List targets".to_string(),
                "t".to_string(),
            ),
        ]
    }

    fn is_enabled(&self, id: Self::Id) -> bool {
        match id {
            Commands::ListScopes => self.can_list_child_scopes(),
            Commands::ListTargets => self.can_list_targets(),
        }
    }
}

impl<'a, C> ScopesPage<'a, C>
where
    C: boundary::ApiClient,
{
    pub fn new(
        parent_scope_id: Option<String>,
        boundary_client: &'a C,
        router: &'a RefCell<Router<Routes>>,
        alerts: &'a Alerts,
    ) -> Self
    where
        C: boundary::ApiClient,
    {
        let columns = vec![
            TableColumn::new(
                "Name".to_string(),
                Constraint::Ratio(3, 8),
                Box::new(|s: &boundary::Scope| s.name.clone()),
            ),
            TableColumn::new(
                "Description".to_string(),
                Constraint::Ratio(3, 8),
                Box::new(|s| s.description.clone()),
            ),
            TableColumn::new(
                "Type".to_string(),
                Constraint::Ratio(1, 8),
                Box::new(|s| s.type_name.clone()),
            ),
            TableColumn::new(
                "ID".to_string(),
                Constraint::Ratio(1, 8),
                Box::new(|s| s.id.clone()),
            ),
        ];
        let table_page = TablePage::new("Scopes".to_string(), vec![], columns, router);
        let mut scopes_page = ScopesPage {
            parent_scope_id,
            boundary_client,
            table_page,
            router,
            handle: tokio::runtime::Handle::current(),
            alerts,
        };
        scopes_page.load();
        scopes_page
    }

    fn load(&mut self) {
        let items = self
            .handle
            .block_on(self.boundary_client.get_scopes(&self.parent_scope_id))
            .map(|scopes| scopes.into_iter().map(Rc::new).collect());
        match items {
            Ok(items) => self.table_page.update_items(items),
            Err(_e) => {
                self.alerts
                    .alert("Error".to_string(), "Failed to load scopes".to_string());
            }
        }
    }

    fn list_scopes(&mut self, scope: &boundary::Scope) {
        if !scope.can_list_child_scopes() {
            return;
        }
        self.router.borrow_mut().push(Routes::Scopes {
            parent: Some(scope.id.clone()),
        });
    }

    fn list_targets(&mut self, scope: &boundary::Scope) {
        if !scope.can_list_targets() {
            return;
        }
        self.router.borrow_mut().push(Routes::Targets {
            scope: scope.id.clone(),
        });
    }

    pub fn handle_event(&mut self, event: &Event) -> bool {
        if self.table_page.handle_event(event) {
            return true;
        }
        if let Event::Key(key_event) = event {
            match key_event.code {
                crossterm::event::KeyCode::Char('s') => {
                    self.list_scopes(self.table_page.selected_item().unwrap().as_ref());
                    return true;
                }
                crossterm::event::KeyCode::Char('t') => {
                    self.list_targets(self.table_page.selected_item().unwrap().as_ref());
                    return true;
                }
                _ => {}
            }
        }
        false
    }

    pub fn render(&self, frame: &mut Frame) {
        self.table_page.render(frame);
    }
}

impl SortItems<boundary::Scope> for TablePage<'_, boundary::Scope> {
    fn sort(items: &mut Vec<Rc<Scope>>) {
        items.sort_by(|a, b| a.name.cmp(&b.name));
    }
}

impl FilterItems<boundary::Scope> for TablePage<'_, boundary::Scope> {
    fn matches(item: &Scope, search: &str) -> bool {
        Self::match_str(&item.name, search)
            || Self::match_str(&item.description, search)
            || Self::match_str(&item.id, search)
    }
}
