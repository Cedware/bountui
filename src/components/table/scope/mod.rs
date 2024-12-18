use crate::boundary;
use crate::boundary::Scope;
use crate::components::table::commands::{Command, HasCommands};
use crate::components::table::{FilterItems, SortItems, TableColumn};
use crate::components::{Alerts, TablePage};
use crate::router::Router;
use crate::routes::Routes;
use crossterm::event::{Event, KeyCode};
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
                "⏎".to_string(),
            ),
            Command::new(
                Commands::ListTargets,
                "List targets".to_string(),
                "⏎".to_string(),
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

    fn show_children(&mut self) {
        if let Some(scope) = self.table_page.selected_item() {
            if scope.can_list_child_scopes() {
                self.list_scopes(scope.as_ref());
            } else if scope.can_list_targets() {
                self.list_targets(scope.as_ref());
            }
        }

    }

    pub fn handle_event(&mut self, event: &Event) -> bool {
        if self.table_page.handle_event(event) {
            return true;
        }
        if let Event::Key(key_event) = event {
            match key_event.code {
                KeyCode::Enter => {
                    self.show_children();
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

#[cfg(test)]
mod test {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use crossterm::event::{Event, KeyCode, KeyEvent};
    use crate::boundary;
    use crate::components::Alerts;
    use crate::components::table::scope::ScopesPage;
    use crate::router::Router;
    use crate::routes::Routes;

    fn scopes() -> Vec<boundary::Scope> {
        vec![
            boundary::Scope {
                id: String::from("scope-id-1"),
                name: String::from("scope-name-1"),
                description: String::from("scope-description-1"),
                type_name: String::from("scope-type-1"),
                authorized_collection_actions: HashMap::from([("scopes".to_string(), vec!["list".to_string()])]),
            },
            boundary::Scope {
                id: String::from("scope-id-2"),
                name: String::from("scope-name-2"),
                description: String::from("scope-description-2"),
                type_name: String::from("scope-type-2"),
                authorized_collection_actions: HashMap::from([("targets".to_string(), vec!["list".to_string()])]),
            },
        ]
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_show_child_scopes() {
        tokio::task::block_in_place(|| {

            let mut boundary_client = boundary::MockApiClient::new();
            boundary_client.expect_get_scopes()
                .with(mockall::predicate::eq(None))
                .return_once(move |_| Box::pin(async { Ok(scopes()) }));

            let router = RefCell::new(Router::new(Routes::Scopes { parent: None }));
            let alerts = Alerts::default();

            let mut page = ScopesPage::new(
                None,
                &boundary_client,
                &router,
                &alerts,
            );
            page.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));
            let route = router.borrow_mut().poll_change();
            assert!(route.is_some(), "Expected route change");
            let route = route.unwrap();
            assert_eq!(*route, Routes::Scopes { parent: Some(String::from("scope-id-1")) });
        })
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_show_targets() {
        tokio::task::block_in_place(|| {

            let mut boundary_client = boundary::MockApiClient::new();
            boundary_client.expect_get_scopes()
                .with(mockall::predicate::eq(None))
                .return_once(move |_| Box::pin(async { Ok(scopes()) }));

            let router = RefCell::new(Router::new(Routes::Scopes { parent: None }));
            let alerts = Alerts::default();

            let mut page = ScopesPage::new(
                None,
                &boundary_client,
                &router,
                &alerts,
            );
            page.table_page.table_state.borrow_mut().select(Some(1));
            page.handle_event(&Event::Key(KeyEvent::from(KeyCode::Enter)));
            let route = router.borrow_mut().poll_change();
            assert!(route.is_some(), "Expected route change");
            let route = route.unwrap();
            assert_eq!(*route, Routes::Targets { scope: String::from("scope-id-2") });
        })
    }

}
