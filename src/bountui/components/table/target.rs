use crate::boundary;
use crate::boundary::{ConnectResponse, Target};
use crate::bountui::components::input_dialog::{Button, InputDialog, InputField};
use crate::bountui::components::table::action::Action;
use crate::bountui::components::table::{FilterItems, SortItems, TableColumn};
use crate::bountui::components::{ConnectionResultDialog, TablePage};
use crate::bountui::Message;
use crate::bountui::Message::GoBack;
use crate::util::MpscSenderExt;
use crossterm::event::{Event, KeyCode};
use ratatui::prelude::Constraint;
use ratatui::Frame;
use std::rc::Rc;
use ratatui::layout::Rect;

pub enum TargetsPageMessage {
    ConnectedToTarget(ConnectResponse),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ConnectDialogFields {
    ListenPort,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ConnectDialogButtons {
    Cancel,
    Ok,
}

pub struct TargetsPage {
    table_page: TablePage<boundary::Target>,
    connect_dialog: Option<InputDialog<ConnectDialogFields, ConnectDialogButtons>>,
    connect_result_dialog: Option<ConnectionResultDialog>,
    message_tx: tokio::sync::mpsc::Sender<Message>
}


impl TargetsPage {
    pub fn new(targets: Vec<Target>, message_tx: tokio::sync::mpsc::Sender<Message>) -> Self{
        let columns = vec![
            TableColumn::new(
                "Name".to_string(),
                Constraint::Ratio(3, 8),
                Box::new(|s: &boundary::Target| s.name.clone()),
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
                Box::new(|_: Option<&Target>| true),
            ),
            Action::new(
                "Back".to_string(),
                "ESC".to_string(),
                Box::new(|_: Option<&Target>| true),
            ),
            Action::new(
                "Show Sessions".to_string(),
                "Shift + C".to_string(),
                Box::new(|item: Option<&Target>| item.is_some()), // Enabled if any target is selected
            ),
            Action::new(
                "Connect".to_string(),
                "c".to_string(),
                Box::new(|item: Option<&Target>| item.map_or(false, |t| t.can_connect())),
            ),
        ];

        let table_page = TablePage::new("Targets".to_string(), columns, targets, actions, message_tx.clone());
        TargetsPage {
            table_page,
            connect_dialog: None,
            connect_result_dialog: None,
            message_tx
        }
    }

    pub fn view(&self, frame: &mut Frame, area: Rect) {
        self.table_page.view(frame, area);
        if let Some(connect_dialog) = &self.connect_dialog {
            connect_dialog.view(frame);
        }
        if let Some(connect_result_dialog) = &self.connect_result_dialog {
            connect_result_dialog.view(frame);
        }
    }

    fn close_connect_result_dialog(&mut self) {
        self.connect_result_dialog = None;
    }

    fn open_connect_dialog(&mut self) {
        self.connect_dialog = Some(InputDialog::new(
            "Connect",
            vec![
                InputField::new(ConnectDialogFields::ListenPort, "Listen Port", ""),
            ],
            vec![
                Button::new(ConnectDialogButtons::Cancel, "Cancel"),
                Button::new(ConnectDialogButtons::Ok, "Ok"),
            ]
        ));
    }

    fn close_connect_dialog(&mut self) {
        self.connect_dialog = None;
    }

    pub fn connection_establised(&mut self, response: ConnectResponse) {
        self.connect_result_dialog = Some(ConnectionResultDialog::new(response, self.message_tx.clone()));
    }
    
    async fn connect_to_target(&mut self) {
        if let Some(target) = self.table_page.selected_item() {
            let port: u16 = self.connect_dialog.as_ref().unwrap().fields.iter().find(|field| field.id == ConnectDialogFields::ListenPort).unwrap().value.value().parse().unwrap();
            let _ = self.message_tx.send(Message::Connect {
                target_id: target.id.clone(),
                port,
            }).await.unwrap();
            self.connect_dialog = None;
        }
    }

    async fn show_sessions(&mut self) {
        if let Some(target) = self.table_page.selected_item() {
            self.message_tx.send(Message::ShowSessions {
                scope: target.scope_id.clone(),
                target_id: target.id.clone()
            }).await.unwrap();
        }
    }

    pub async fn handle_event(&mut self, event: &Event) {

        // 1. Handle ConnectionResultDialog FIRST if it's open
        if let Some(dialog) = &mut self.connect_result_dialog {
            if let Event::Key(key_event) = event {
                if key_event.code == KeyCode::Esc {
                    self.close_connect_result_dialog();
                    return; // Consume Esc, don't forward
                }
            }
            // Forward all other events to the dialog
            dialog.handle_event(event).await;
            return; // Consume the event, don't let TargetsPage handle it further
        }

        // 2. Handle ConnectDialog if it's open
        if let Some(connect_dialog) = &mut self.connect_dialog {
            match connect_dialog.handle_event(event) {
                Some(ConnectDialogButtons::Cancel) => {
                    self.close_connect_dialog();
                    return; // Consume event
                }
                Some(ConnectDialogButtons::Ok) => {
                    self.connect_to_target().await;
                    return; // Consume event
                }
                None => {
                    // Event was handled by the input field or ignored by the dialog
                    return; // Consume event
                }
            }
        }

        // 3. Handle TablePage filtering input and basic navigation/actions
        // Note: handle_event might consume events like Up/Down/Enter for selection/filtering
        if self.table_page.handle_event(event).await {
            return;
        }

        // 4. Handle TargetsPage specific keys (only if dialogs are closed and filter is inactive)
        if let Event::Key(key_event) = event {
            match key_event.code {
                KeyCode::Char('c') => {
                    // Only open connect dialog if a target is selected and can be connected to
                    if let Some(target) = self.table_page.selected_item() {
                         if target.can_connect() {
                              self.open_connect_dialog();
                         }
                    }
                },
                KeyCode::Char('C') => {
                    // Show sessions for the selected target if possible
                     if self.table_page.selected_item().is_some() {
                         self.show_sessions().await;
                     }
                },
                KeyCode::Esc => {
                    // Go back only if no dialogs are open
                    self.message_tx.send_or_expect(GoBack).await;
                },
                _ => { }
            }
        }
    }

    pub fn handle_message(&mut self, message: TargetsPageMessage) {
        match message {
            TargetsPageMessage::ConnectedToTarget(response) => {
                self.connection_establised(response);
            }
        }
    }

}

impl SortItems<boundary::Target> for TablePage<boundary::Target> {
    fn sort(items: &mut Vec<Rc<boundary::Target>>) {
        items.sort_by(|a, b| a.name.cmp(&b.name));
    }
}

impl FilterItems<boundary::Target> for TablePage<boundary::Target> {
    fn matches(item: &boundary::Target, search: &str) -> bool {
        Self::match_str(&item.name, search)
            || Self::match_str(&item.description, search)
            || Self::match_str(&item.id, search)
    }
}
