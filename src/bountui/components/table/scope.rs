use crate::appframework::{Component, UpdateState};
use crate::boundary;
use crate::boundary::Scope;
use crate::bountui::components::table::action::Action;
use crate::bountui::components::table::{FilterItems, HasActions, SortItems, TableColumn, TableMessage};
use crate::bountui::components::TablePage;
use crate::bountui::Message;
use crossterm::event::{Event, KeyCode};
use ratatui::layout::Constraint;
use ratatui::Frame;
use std::rc::Rc;

pub struct ScopesPage {
    table_page: TablePage<boundary::Scope>,
}

pub enum ScopesMessage {
    Table(TableMessage)
}

impl From<ScopesMessage> for Message {
    fn from(message: ScopesMessage) -> Self {
        Message::Scopes(message)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ScopeAction {
    ListScopes,
    ListTargets,
}


impl ScopesPage {
    pub fn new(scopes: Vec<Scope>) -> Self {
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
        let table_page = TablePage::new(
            "Scopes".to_string(),
            columns,
            scopes,
        );

        ScopesPage {
            table_page,
        }
    }
}

impl Component<Message> for ScopesPage {
    fn view(&self, frame: &mut Frame) {
        self.table_page.view(frame);
    }

    fn handle_event(&self, event: &Event) -> Option<Message> {
        let message = self.table_page.handle_event(event);
        if let Some(message) = message {
            return Some(Message::Scopes(ScopesMessage::Table(message)));
        }
        if let Event::Key(key_event) = event {
            if key_event.code == KeyCode::Enter {
                if let Some(selected) = self.table_page.selected_item() {
                    if selected.can_list_child_scopes() {
                        return Some(Message::ShowScopes {
                            parent: Some(selected.id.clone()),
                        });
                    } else if selected.can_list_targets() {
                        return Some(Message::ShowTargets {
                            parent: Some(selected.id.clone())
                        });
                    }
                }
            }
        }
        None
    }
}

impl UpdateState<ScopesMessage, Message> for ScopesPage {
    async fn update(&mut self, message: &ScopesMessage) -> Option<Message> {
        match message {
            ScopesMessage::Table(table_message) => {
                self.table_page.update(table_message).await
            }
        }
    }
}

// impl<'a, C> ScopesPage<'a, C>
// where
//     C: boundary::ApiClient,
// {
//     pub fn new(
//         parent_scope_id: Option<String>,
//         boundary_client: &'a C,
//         router: &'a Router<Routes>,
//         alerts: &'a Alerts,
//     ) -> Self
//     where
//         C: boundary::ApiClient,
//     {
//         let columns = vec![
//             TableColumn::new(
//                 "Name".to_string(),
//                 Constraint::Ratio(3, 8),
//                 Box::new(|s: &boundary::Scope| s.name.clone()),
//             ),
//             TableColumn::new(
//                 "Description".to_string(),
//                 Constraint::Ratio(3, 8),
//                 Box::new(|s| s.description.clone()),
//             ),
//             TableColumn::new(
//                 "Type".to_string(),
//                 Constraint::Ratio(1, 8),
//                 Box::new(|s| s.type_name.clone()),
//             ),
//             TableColumn::new(
//                 "ID".to_string(),
//                 Constraint::Ratio(1, 8),
//                 Box::new(|s| s.id.clone()),
//             ),
//         ];
//         let table_page = TablePage::new("Scopes".to_string(), vec![], columns, router);
//         let mut scopes_page = ScopesPage {
//             parent_scope_id,
//             boundary_client,
//             table_page,
//             router,
//             handle: tokio::runtime::Handle::current(),
//             alerts,
//         };
//         scopes_page.load();
//         scopes_page
//     }
//
//     fn load(&mut self) {
//         let items = self
//             .handle
//             .block_on(self.boundary_client.get_scopes(&self.parent_scope_id))
//             .map(|scopes| scopes.into_iter().map(Rc::new).collect());
//         match items {
//             Ok(items) => self.table_page.update_items(items),
//             Err(_e) => {
//                 self.alerts
//                     .alert("Error".to_string(), "Failed to load scopes".to_string());
//             }
//         }
//     }
//
//     fn list_scopes(&mut self, scope: &boundary::Scope) {
//         if !scope.can_list_child_scopes() {
//             return;
//         }
//         self.router.push(Routes::Scopes {
//             parent: Some(scope.id.clone()),
//         });
//     }
//
//     fn list_targets(&mut self, scope: &boundary::Scope) {
//         if !scope.can_list_targets() {
//             return;
//         }
//         self.router.push(Routes::Targets {
//             scope: scope.id.clone(),
//         });
//     }
//
//     fn show_children(&mut self) {
//         if let Some(scope) = self.table_page.selected_item() {
//             if scope.can_list_child_scopes() {
//                 self.list_scopes(scope.as_ref());
//             } else if scope.can_list_targets() {
//                 self.list_targets(scope.as_ref());
//             }
//         }
//
//     }
//
//     pub fn handle_event(&mut self, event: &Event) -> bool {
//         if self.table_page.handle_event(event) {
//             return true;
//         }
//         if let Event::Key(key_event) = event {
//             match key_event.code {
//                 KeyCode::Enter => {
//                     self.show_children();
//                     return true;
//                 }
//                 _ => {}
//             }
//         }
//         false
//     }
//
//     pub fn render(&self, frame: &mut Frame) {
//         self.table_page.render(frame);
//     }
// }

impl SortItems<boundary::Scope> for TablePage<boundary::Scope> {
    fn sort(items: &mut Vec<Rc<Scope>>) {
        items.sort_by(|a, b| a.name.cmp(&b.name));
    }
}

impl FilterItems<boundary::Scope> for TablePage<boundary::Scope> {
    fn matches(item: &Scope, search: &str) -> bool {
        Self::match_str(&item.name, search)
            || Self::match_str(&item.description, search)
            || Self::match_str(&item.id, search)
    }
}

impl HasActions<Scope> for TablePage<Scope> {
    type Id = ScopeAction;

    fn actions(&self) -> Vec<Action<Self::Id>> {
        vec![
            Action::new(
                ScopeAction::ListScopes,
                "List scopes".to_string(),
                "⏎".to_string(),
            ),
            Action::new(
                ScopeAction::ListTargets,
                "List targets".to_string(),
                "⏎".to_string(),
            ),
        ]
    }

    fn is_action_enabled(&self, id: Self::Id, item: &Scope) -> bool {
        match id {
            ScopeAction::ListScopes => item.can_list_child_scopes(),
            ScopeAction::ListTargets => item.can_list_targets(),
        }
    }
}
