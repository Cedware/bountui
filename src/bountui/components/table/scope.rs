
use crate::boundary;
use crate::boundary::Scope;
use crate::bountui::components::table::action::Action;
use crate::bountui::components::table::{FilterItems, HasActions, SortItems, TableColumn};
use crate::bountui::components::TablePage;
use crate::bountui::Message;
use crossterm::event::{Event, KeyCode};
use ratatui::layout::Constraint;
use ratatui::Frame;
use std::rc::Rc;

pub struct ScopesPage {
    table_page: TablePage<boundary::Scope>,
    send_message: tokio::sync::mpsc::Sender<Message>
}

#[derive(Debug, Clone, Copy)]
pub enum ScopeAction {
    ListScopes,
    ListTargets,
}


impl ScopesPage {
    pub fn new(scopes: Vec<Scope>, message_tx: tokio::sync::mpsc::Sender<Message>) -> Self {
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
            message_tx.clone()
        );

        ScopesPage {
            table_page,
            send_message: message_tx
        }
    }

    pub fn view(&self, frame: &mut Frame) {
        self.table_page.view(frame);
    }

    pub async fn handle_event(&mut self, event: &Event) {
        let filter_was_active = self.table_page.is_filter_input_active();
        self.table_page.handle_event(event).await;
        if filter_was_active {
            return;
        }
        if let Event::Key(key_event) = event {
            if key_event.code == KeyCode::Enter {
                if let Some(selected) = self.table_page.selected_item() {
                    if selected.can_list_child_scopes() {
                        self.send_message.send(Message::ShowScopes {
                            parent: Some(selected.id.clone()),
                        }).await.expect("Message channel closed unexpectedly");
                    } else if selected.can_list_targets() {
                        self.send_message.send(Message::ShowTargets {
                            parent: Some(selected.id.clone()),
                        }).await.expect("Message channel closed unexpectedly");
                    }
                }
            }
        }
    }
}



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
