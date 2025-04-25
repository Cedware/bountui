use crate::appframework::{Component, UpdateState};
use crate::boundary;
use crate::boundary::{ConnectResponse, Target};
use crate::bountui::components::input_dialog::{Button, InputDialog, InputDialogMessage, InputField};
use crate::bountui::components::table::action::Action;
use crate::bountui::components::table::{FilterItems, HasActions, SortItems, TableColumn, TableMessage, };
use crate::bountui::components::TablePage;
use crate::bountui::Message;
use crossterm::event::{Event, KeyCode};
use ratatui::prelude::{Constraint, Widget};
use ratatui::Frame;
use std::rc::Rc;
use crate::bountui::widgets::ConnectResponseDialog;

#[derive(Debug, Clone)]
pub enum TargetsMessage {
    OpenConnectDialog,
    Table(TableMessage),
    ConnectDialog(InputDialogMessage<ConnectDialogFields>),
    FinishConnectDialog {
        target_id: String,
        port: u16
    },
    Connected(ConnectResponse),
    CloseConnectResultDialog,
    CancelConnect
}

impl From<TargetsMessage> for Message {
    fn from(value: TargetsMessage) -> Self {
        Message::Targets(value)
    }
}

impl From<InputDialogMessage<ConnectDialogFields>> for TargetsMessage {
    fn from(value: InputDialogMessage<ConnectDialogFields>) -> Self {
        TargetsMessage::ConnectDialog(value)
    }
}


#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ConnectDialogFields {
    ListenPort,
}

pub struct TargetsPage {
    table_page: TablePage<boundary::Target>,
    connect_dialog: Option<InputDialog<ConnectDialogFields, TargetsMessage>>,
    connect_result: Option<ConnectResponse>,
}


impl TargetsPage {
    pub fn new(targets: Vec<Target>) -> Self{
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
        }
    }

}

#[derive(Debug, Clone, Copy)]
pub enum TargetAction {
    Connect,
    ShowConnections,
}

impl Component<Message> for TargetsPage {
    fn view(&self, frame: &mut Frame) {
        self.table_page.view(frame);
        if let Some(connect_dialog) = &self.connect_dialog {
            connect_dialog.view(frame);
        }
        if let Some(connect_result) = &self.connect_result {
            frame.render_widget(ConnectResponseDialog::new(connect_result), frame.area())
        }
    }

    fn handle_event(&self, event: &Event) -> Option<Message> {
        
        if let Some(connect_dialog) = &self.connect_dialog {
            return connect_dialog.handle_event(event).map(Message::from);
        }

        if let Some(_) = &self.connect_result {
            if let Event::Key(key_event) = event {
                match key_event.code {
                    KeyCode::Enter => {
                        return Some(TargetsMessage::CloseConnectResultDialog.into())
                    },
                    _ => { }
                }
            }
            return None
        }
        
        if let Event::Key(key_event) = event {
            match key_event.code { 
                KeyCode::Char('c') => {
                    return Some(TargetsMessage::OpenConnectDialog.into())
                },
                KeyCode::Char('C') => {
                    if let Some(target) = self.table_page.selected_item() {
                        return Some(Message::ShowSessions {
                            scope: target.scope_id.clone(),
                            target_id: target.id.clone()
                        })
                    }
                },
                _ => { }
            }
        }
        self.table_page.handle_event(event).map(TargetsMessage::Table).map(Message::from)
    }
}

impl UpdateState<TargetsMessage, Message> for TargetsPage {
    async fn update(&mut self, message: &TargetsMessage) -> Option<Message> {
        match message {
            TargetsMessage::Table(table_message) => {
                self.table_page.update(table_message).await
            },
            TargetsMessage::ConnectDialog(dialog_message) => {
                if let Some(dialog) = &mut self.connect_dialog {
                    dialog.update(dialog_message).await
                } else {
                    None
                }
            },
            TargetsMessage::CancelConnect => {
                self.connect_dialog = None;
                None
            },
            TargetsMessage::FinishConnectDialog {target_id, port} => {
                self.connect_dialog = None;
                Some(Message::Connect {
                    target_id: target_id.clone(),
                    port: *port
                })
            },
            TargetsMessage::OpenConnectDialog => {
                let target_id = self.table_page.selected_item().unwrap().id.clone();
                self.connect_dialog = Some(InputDialog::new(
                    "Connect",
                    vec![
                        InputField::new(ConnectDialogFields::ListenPort, "Listen Port", ""),
                    ],
                    vec![
                        Button::new("Cancel", |_| TargetsMessage::CancelConnect),
                        Button::new("Ok", move |fields| {
                            let port: u16 = fields.iter().find(|field| field.id == ConnectDialogFields::ListenPort).unwrap().value.value().parse().unwrap();
                            TargetsMessage::FinishConnectDialog {
                                target_id: target_id.clone(),
                                port
                            }
                        }),
                    ]
                ));
                None
            },
            TargetsMessage::Connected(response) => {
                self.connect_result = Some(response.clone());
                None
            },
            TargetsMessage::CloseConnectResultDialog => {
                self.connect_result = None;
                None
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