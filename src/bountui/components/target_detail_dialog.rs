use crate::boundary;
use crate::bountui::components::table::{Action, FilterItems, SortItems, TableColumn};
use crate::bountui::components::TablePage;
use crate::bountui::Message;
use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::layout::{Constraint, Flex};
use ratatui::prelude::{Alignment, Stylize};
use ratatui::widgets::{Block, BorderType, Borders, Clear};
use ratatui::Frame;
use std::rc::Rc;
use tokio::sync::mpsc;

#[derive(Clone)]
struct TargetDetailRow {
    label: String,
    value: String,
}

impl TargetDetailRow {
    fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
        }
    }
}

pub struct TargetDetailDialog {
    table: TablePage<TargetDetailRow>,
    message_tx: mpsc::Sender<Message>,
}

impl TargetDetailDialog {
    pub fn new(target: &boundary::Target, message_tx: mpsc::Sender<Message>) -> Self {
        let rows = vec![
            TargetDetailRow::new("Name", &target.name),
            TargetDetailRow::new("Description", &target.description),
            TargetDetailRow::new("Type", &target.type_name),
            TargetDetailRow::new("ID", &target.id),
            TargetDetailRow::new("Scope ID", &target.scope_id),
            TargetDetailRow::new(
                "Default Port",
                target
                    .default_client_port()
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "None".to_string()),
            ),
            TargetDetailRow::new(
                "Authorized Actions",
                if target.authorized_actions.is_empty() {
                    "None".to_string()
                } else {
                    target.authorized_actions.join(", ")
                },
            ),
        ];

        let columns = vec![
            TableColumn::new(
                "Field".to_string(),
                Constraint::Ratio(1, 3),
                Box::new(|r: &TargetDetailRow| r.label.clone()),
            ),
            TableColumn::new(
                "Value".to_string(),
                Constraint::Ratio(2, 3),
                Box::new(|r: &TargetDetailRow| r.value.clone()),
            ),
        ];

        let actions = vec![
            Action::new(
                "Close".to_string(),
                "ESC".to_string(),
                Box::new(|_: Option<&TargetDetailRow>| true),
            ),
            Action::new(
                "Copy".to_string(),
                "c".to_string(),
                Box::new(|item: Option<&TargetDetailRow>| item.is_some()),
            ),
        ];

        let table = TablePage::new(
            format!("Target Details: {}", target.name),
            columns,
            rows,
            actions,
            message_tx.clone(),
            false,
        );

        Self { table, message_tx }
    }

    pub fn view(&self, frame: &mut Frame) {
        let area = frame.area();
        let vertical =
            ratatui::layout::Layout::vertical([Constraint::Percentage(60)]).flex(Flex::Center);
        let horizontal =
            ratatui::layout::Layout::horizontal([Constraint::Percentage(70)]).flex(Flex::Center);
        let [area] = vertical.areas(area);
        let [area] = horizontal.areas(area);

        frame.render_widget(Clear, area);

        let block = Block::default()
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .light_blue()
            .on_black();

        let inner_area = block.inner(area);
        frame.render_widget(block, area);
        self.table.view(frame, inner_area);
    }

    pub async fn handle_event(&mut self, event: &Event) {
        if let Event::Key(key_event) = event {
            if key_event.modifiers == KeyModifiers::NONE {
                match key_event.code {
                    KeyCode::Char('c') => {
                        self.copy_selected_to_clipboard().await;
                        return;
                    }
                    _ => {}
                }
            }
        }
        self.table.handle_event(event).await;
    }

    async fn copy_selected_to_clipboard(&self) {
        if let Some(row) = self.table.selected_item() {
            let value = row.value.clone();
            let label = row.label.clone();
            let _ = self
                .message_tx
                .send(Message::SetClipboard {
                    text: value,
                    on_success: Some(Box::new(Message::Toaster(
                        crate::bountui::components::toaster::Message::ShowToast {
                            text: format!("{label} copied"),
                            duration: std::time::Duration::from_secs(3),
                        },
                    ))),
                    on_error: Some(Box::new(Message::Toaster(
                        crate::bountui::components::toaster::Message::ShowToast {
                            text: "Failed to copy".to_string(),
                            duration: std::time::Duration::from_secs(3),
                        },
                    ))),
                })
                .await;
        }
    }
}

impl SortItems<TargetDetailRow> for TablePage<TargetDetailRow> {
    fn sort(items: &mut Vec<Rc<TargetDetailRow>>) {
        // Keep original order — no sorting
        let _ = items;
    }
}

impl FilterItems<TargetDetailRow> for TablePage<TargetDetailRow> {
    fn matches(item: &TargetDetailRow, search: &str) -> bool {
        Self::match_str(&item.label, search) || Self::match_str(&item.value, search)
    }
}
