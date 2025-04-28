
use crate::boundary;
use crate::boundary::{ConnectResponse, Target};
use crate::bountui::components::input_dialog::{Button, InputDialog, InputDialogMessage, InputField};
use crate::bountui::components::table::action::Action;
use crate::bountui::components::table::{FilterItems, HasActions, SortItems, TableColumn};
use crate::bountui::components::TablePage;
use crate::bountui::widgets::ConnectResponseDialog;
use crate::bountui::Message;
use crossterm::event::{Event, KeyCode};
use ratatui::prelude::Constraint;
use ratatui::Frame;
use std::rc::Rc;


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
    connect_result: Option<ConnectResponse>,
    message_sender: tokio::sync::mpsc::Sender<Message>
}


impl TargetsPage {
    pub fn new(targets: Vec<Target>, message_sender: tokio::sync::mpsc::Sender<Message>) -> Self{
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

        let table_page = TablePage::new("Targets".to_string(), columns, targets);
        TargetsPage {
            table_page,
            connect_dialog: None,
            connect_result: None,
            message_sender
        }
    }

    pub fn view(&self, frame: &mut Frame) {
        self.table_page.view(frame);
        if let Some(connect_dialog) = &self.connect_dialog {
            connect_dialog.view(frame);
        }
        if let Some(connect_result) = &self.connect_result {
            frame.render_widget(ConnectResponseDialog::new(connect_result), frame.area())
        }
    }

    fn close_connect_result_dialog(&mut self) {
        self.connect_result = None;
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
        self.connect_result = Some(response);
    }
    
    async fn connect_to_target(&mut self) {
        if let Some(target) = self.table_page.selected_item() {
            let port: u16 = self.connect_dialog.as_ref().unwrap().fields.iter().find(|field| field.id == ConnectDialogFields::ListenPort).unwrap().value.value().parse().unwrap();
            let (send_response, receive_response) = tokio::sync::oneshot::channel();
            let _ = self.message_sender.send(Message::Connect {
                target_id: target.id.clone(),
                port,
                respond_to: send_response,
            }).await.unwrap();
            self.connect_dialog = None;
        }
    }

    async fn show_sessions(&mut self) {
        if let Some(target) = self.table_page.selected_item() {
            self.message_sender.send(Message::ShowSessions {
                scope: target.scope_id.clone(),
                target_id: target.id.clone()
            }).await.unwrap();
        }
    }

    pub async fn handle_event(&mut self, event: &Event) {

        if let Some(connect_dialog) = &mut self.connect_dialog {
            if let Some(button_clicked) = connect_dialog.handle_event(event) {
                match button_clicked {
                    ConnectDialogButtons::Cancel => {
                        self.close_connect_dialog();
                    }
                    ConnectDialogButtons::Ok => {
                        self.connect_to_target().await;
                    }
                }
            }
        }

        if let Some(_) = &self.connect_result {
            if let Event::Key(key_event) = event {
                match key_event.code {
                    KeyCode::Enter => {
                        self.close_connect_result_dialog();
                    },
                    _ => { }
                }
            }
        }

        if let Event::Key(key_event) = event {
            match key_event.code {
                KeyCode::Char('c') => {
                    self.open_connect_dialog();
                },
                KeyCode::Char('C') => {
                    self.show_sessions().await;
                },
                _ => { }
            }
        }
        self.table_page.handle_event(event);
    }

    pub fn handle_message(&mut self, message: TargetsPageMessage) {
        match message {
            TargetsPageMessage::ConnectedToTarget(response) => {
                self.connection_establised(response);
            }
        }
    }

}

#[derive(Debug, Clone, Copy)]
pub enum TargetAction {
    Connect,
    ShowConnections,
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

impl HasActions<boundary::Target> for TablePage<boundary::Target> {
    type Id = TargetAction;

    fn actions(&self) -> Vec<Action<Self::Id>> {
        vec![
            Action::new(TargetAction::Connect, "Connect".to_string(), "c".to_string()),
            Action::new(
                TargetAction::ShowConnections,
                "Show connections".to_string(),
                "Shift+C".to_string(),
            ),
        ]
    }

    fn is_action_enabled(&self, id: Self::Id, item: &boundary::Target) -> bool {
        match id {
            TargetAction::Connect => item.can_connect(),
            TargetAction::ShowConnections => true,
        }
    }
}