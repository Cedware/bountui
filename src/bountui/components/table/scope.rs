use crate::boundary;
use crate::boundary::{ApiClient, Scope};
use crate::bountui::components::table::action::Action;
use crate::bountui::components::table::{FilterItems, SortItems, TableColumn};
use crate::bountui::components::TablePage;
use crate::bountui::{Message};
use crossterm::event::{Event, KeyCode};
use ratatui::layout::{Constraint, Rect};
use ratatui::Frame;
use std::rc::Rc;
use futures::FutureExt;
use crate::bountui::components::table::util::format_title_with_parent;

pub struct ScopesPage {
    table_page: TablePage<boundary::Scope>,
    send_message: tokio::sync::mpsc::Sender<Message>
}

pub enum ScopesPageMessage {
    ScopesLoaded(Vec<Scope>),
}

impl From<ScopesPageMessage> for Message {
    fn from(value: ScopesPageMessage) -> Self {
        Message::Scopes(value)
    }
}

impl ScopesPage {
    pub async fn new<C: ApiClient + Send + 'static>(parent_scope: Option<&Scope>, message_tx: tokio::sync::mpsc::Sender<Message>, boundary_client: C) -> Self {
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
        
        let parent_id = parent_scope.map(|s| s.id.clone());
        Self::load_scopes(parent_id, &message_tx, boundary_client).await;
        let title = format_title_with_parent("Scopes", parent_scope.map(|s| s.name.as_str()));
        let table_page = TablePage::new(
            title,
            columns,
            Vec::new(),
            actions,
            message_tx.clone(),
            true
        );

        ScopesPage {
            table_page,
            send_message: message_tx
        }
    }

    async fn load_scopes<C: ApiClient + Send + 'static>(parent_id: Option<String>, message_tx: &tokio::sync::mpsc::Sender<Message>, boundary_client: C) {
        let message_tx_clone = message_tx.clone();
        let _ = message_tx.send(Message::RunFuture(async move {
            let result = boundary_client.get_scopes(parent_id.as_ref().map(|i| i.as_str()), false).await;
            let message = match result {
                Ok(scopes) => {
                    ScopesPageMessage::ScopesLoaded(scopes).into()
                },
                Err(e) => {
                    Message::ShowAlert("Error".to_string(), format!("Failed to load scopes: {}", e))
                }
            };
            message_tx_clone.send(message).await.unwrap();
        }.boxed())).await;
    }

    pub fn view(&self, frame: &mut Frame, area: Rect) {
        self.table_page.view(frame, area);
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
                                parent: Some((*scope).clone())
                            }).await.unwrap();
                        } else if scope.can_list_targets() {
                            self.send_message.send(Message::ShowTargets {
                                parent: (*scope).clone()
                            }).await.unwrap();
                        }
                    }
                }
                _ => {}
            }
        }
    }

    pub async fn handle_message(&mut self, message: ScopesPageMessage) {
        match message {
            ScopesPageMessage::ScopesLoaded(scopes) => {
                self.table_page.set_items(scopes);
                self.table_page.loading = false;
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

