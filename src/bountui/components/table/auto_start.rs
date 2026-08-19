use crate::bountui::components::table::{Action, FilterItems, SortItems, TableColumn};
use crate::bountui::components::TablePage;
use crate::bountui::remember_user_input::{AutoStartConnection, RememberUserInput};
use crate::bountui::Message;
use crossterm::event::{Event, KeyCode};
use ratatui::layout::{Constraint, Rect};
use ratatui::Frame;
use std::rc::Rc;

pub struct AutoStartConnectionsPage<R: RememberUserInput> {
    table_page: TablePage<AutoStartConnection>,
    remember_user_input: R,
    message_tx: tokio::sync::mpsc::Sender<Message>,
}

impl<R: RememberUserInput> AutoStartConnectionsPage<R> {
    pub fn new(
        remember_user_input: R,
        message_tx: tokio::sync::mpsc::Sender<Message>,
    ) -> anyhow::Result<Self> {
        let connections = remember_user_input.get_auto_start_connections()?;
        let columns = vec![
            TableColumn::new(
                "Target".to_string(),
                Constraint::Ratio(3, 8),
                Box::new(|connection: &AutoStartConnection| connection.target_name.clone()),
            ),
            TableColumn::new(
                "Target ID".to_string(),
                Constraint::Ratio(3, 8),
                Box::new(|connection| connection.target_id.clone()),
            ),
            TableColumn::new(
                "Listen Port".to_string(),
                Constraint::Ratio(2, 8),
                Box::new(|connection| connection.local_port.to_string()),
            ),
        ];
        let actions = vec![
            Action::new(
                "Quit".to_string(),
                "Ctrl + C".to_string(),
                Box::new(|_| true),
            ),
            Action::new("Back".to_string(), "ESC".to_string(), Box::new(|_| true)),
            Action::new(
                "Delete".to_string(),
                "d".to_string(),
                Box::new(|connection| connection.is_some()),
            ),
        ];

        Ok(Self {
            table_page: TablePage::new(
                "Auto-Start Connections".to_string(),
                columns,
                connections,
                actions,
                message_tx.clone(),
                false,
            ),
            remember_user_input,
            message_tx,
        })
    }

    pub fn view(&self, frame: &mut Frame, area: Rect) {
        self.table_page.view(frame, area);
    }

    pub async fn handle_event(&mut self, event: &Event) {
        if self.table_page.handle_event(event).await {
            return;
        }
        if !matches!(event, Event::Key(key_event) if key_event.code == KeyCode::Char('d')) {
            return;
        }
        let Some(connection) = self.table_page.selected_item() else {
            return;
        };

        if let Err(error) = self
            .remember_user_input
            .remove_auto_start_connection(&connection.target_id)
        {
            self.message_tx
                .send(Message::ShowAlert(
                    "Error".to_string(),
                    format!("Failed to delete auto-start connection: {error}"),
                ))
                .await
                .unwrap();
            return;
        }

        match self.remember_user_input.get_auto_start_connections() {
            Ok(connections) => self.table_page.set_items(connections),
            Err(error) => {
                self.message_tx
                    .send(Message::ShowAlert(
                        "Error".to_string(),
                        format!("Failed to reload auto-start connections: {error}"),
                    ))
                    .await
                    .unwrap();
            }
        }
    }
}

impl SortItems<AutoStartConnection> for TablePage<AutoStartConnection> {
    fn sort(items: &mut Vec<Rc<AutoStartConnection>>) {
        items.sort_by(|a, b| a.target_name.cmp(&b.target_name));
    }
}

impl FilterItems<AutoStartConnection> for TablePage<AutoStartConnection> {
    fn matches(item: &AutoStartConnection, search: &str) -> bool {
        Self::match_str(&item.target_name, search)
            || Self::match_str(&item.target_id, search)
            || Self::match_str(&item.local_port.to_string(), search)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bountui::UserInputsPath;
    use crossterm::event::KeyEvent;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn delete_removes_selected_connection_from_persistence_and_table() {
        let file = NamedTempFile::new().unwrap();
        let mut store = UserInputsPath(file.path());
        store
            .store_auto_start_connection(AutoStartConnection {
                target_id: "target-1".to_string(),
                target_name: "Target One".to_string(),
                local_port: 4242,
            })
            .unwrap();
        let (message_tx, _message_rx) = tokio::sync::mpsc::channel(4);
        let mut page = AutoStartConnectionsPage::new(store, message_tx).unwrap();

        page.handle_event(&Event::Key(KeyEvent::from(KeyCode::Char('d'))))
            .await;

        assert!(store.get_auto_start_connections().unwrap().is_empty());
        assert!(page.table_page.selected_item().is_none());
    }

    #[test]
    fn renders_persisted_connections_and_delete_action() {
        let file = NamedTempFile::new().unwrap();
        let mut store = UserInputsPath(file.path());
        store
            .store_auto_start_connection(AutoStartConnection {
                target_id: "target-1".to_string(),
                target_name: "Target One".to_string(),
                local_port: 4242,
            })
            .unwrap();
        let (message_tx, _message_rx) = tokio::sync::mpsc::channel(4);
        let page = AutoStartConnectionsPage::new(store, message_tx).unwrap();
        let backend = ratatui::backend::TestBackend::new(100, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| page.view(frame, frame.area()))
            .unwrap();

        let content: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(content.contains("Auto-Start Connections"));
        assert!(content.contains("Target One"));
        assert!(content.contains("target-1"));
        assert!(content.contains("4242"));
        assert!(content.contains("Delete<d>"));
    }
}
