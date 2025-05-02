use crate::boundary;
use crate::boundary::Scope;
use crate::bountui::components::table::action::Action;
use crate::bountui::components::table::{FilterItems, SortItems, TableColumn};
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

        let actions = vec![
            Action::new(
                "Quit".to_string(),
                "Ctrl + C".to_string(),
                Box::new(|_: Option<&Scope>| true),
            ),
            Action::new(
                "Back".to_string(),
                "ESC".to_string(),
                Box::new(|_: Option<&Scope>| true),
            ),
            Action::new(
                "List Scopes".to_string(),
                "⏎".to_string(),
                Box::new(|item: Option<&Scope>| item.map_or(false, |s| s.can_list_child_scopes())),
            ),
            Action::new(
                "List Targets".to_string(),
                "⏎".to_string(),
                Box::new(|item: Option<&Scope>| item.map_or(false, |s| s.can_list_targets())),
            ),
        ];

        let table_page = TablePage::new(
            "Scopes".to_string(),
            columns,
            scopes,
            actions,
            message_tx.clone()
        );

        ScopesPage {
            table_page,
            send_message: message_tx
        }
    }

    pub fn view(&self, frame: &mut Frame) {
        self.table_page.view(frame, frame.area());
    }

    pub async fn handle_event(&mut self, event: &Event) {
        if self.table_page.handle_event(event).await {
            return;
        }
        if let Event::Key(key_event) = event {
            match key_event.code {
                KeyCode::Enter => {
                    if let Some(scope) = self.table_page.selected_item() {
                        if scope.can_list_child_scopes() {
                            self.send_message.send(Message::ShowScopes {
                                parent: Some(scope.id.clone())
                            }).await.unwrap();
                        } else if scope.can_list_targets() {
                            self.send_message.send(Message::ShowTargets {
                                parent: Some(scope.id.clone())
                            }).await.unwrap();
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

impl SortItems<Scope> for TablePage<Scope> {
    fn sort(items: &mut Vec<Rc<Scope>>) {
        items.sort_by(|a, b| a.name.cmp(&b.name));
    }
}

impl FilterItems<Scope> for TablePage<Scope> {
    fn matches(item: &Scope, search: &str) -> bool {
        Self::match_str(&item.name, search)
            || Self::match_str(&item.description, search)
            || Self::match_str(&item.id, search)
    }
}
