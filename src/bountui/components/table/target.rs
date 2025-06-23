use crate::boundary;
use crate::boundary::{ApiClient, ConnectResponse, Scope, Target};
use crate::bountui::components::input_dialog::{Button, InputDialog, InputField};
use crate::bountui::components::table::action::Action;
use crate::bountui::components::table::util::format_title_with_parent;
use crate::bountui::components::table::{FilterItems, SortItems, TableColumn};
use crate::bountui::components::{ConnectionResultDialog, TablePage};
use crate::bountui::remember_user_input::RememberUserInput;
use crate::bountui::Message;
use crate::bountui::Message::GoBack;
use crate::util::MpscSenderExt;
use crossterm::event::{Event, KeyCode};
use futures::FutureExt;
use ratatui::layout::Rect;
use ratatui::prelude::Constraint;
use ratatui::Frame;
use std::rc::Rc;

pub enum TargetsPageMessage {
    ConnectedToTarget(ConnectResponse),
    TargetsLoaded(Vec<Target>),
}

impl From<TargetsPageMessage> for Message {
    fn from(value: TargetsPageMessage) -> Self {
        Message::Targets(value)
    }
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

pub struct TargetsPage<C, S: RememberUserInput> {
    table_page: TablePage<boundary::Target>,
    connect_dialog: Option<InputDialog<ConnectDialogFields, ConnectDialogButtons>>,
    connect_result_dialog: Option<ConnectionResultDialog>,
    message_tx: tokio::sync::mpsc::Sender<Message>,
    boundary_client: C,
    parent_scope: Scope,
    remember_user_input: S,
}

impl<C, S: RememberUserInput> TargetsPage<C, S> {
    pub async fn new(
        parent_scope: Scope,
        message_tx: tokio::sync::mpsc::Sender<Message>,
        boundary_client: C,
        remember_user_input: S,
    ) -> Self
    where
        C: ApiClient + Clone + Send + 'static,
    {
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

        let table_page = TablePage::new(
            format_title_with_parent("Targets", Some(parent_scope.name.as_str())),
            columns,
            Vec::new(),
            actions,
            message_tx.clone(),
            true,
        );
        let targets_page = TargetsPage {
            table_page,
            connect_dialog: None,
            connect_result_dialog: None,
            message_tx,
            parent_scope,
            boundary_client,
            remember_user_input,
        };
        targets_page.load_targets().await;
        targets_page
    }

    pub async fn load_targets(&self)
    where
        C: ApiClient + Clone + Send + 'static,
    {
        let boundary_client = self.boundary_client.clone();
        let message_tx = self.message_tx.clone();
        let scope_id = self.parent_scope.id.clone();
        let future = async move {
            match boundary_client.get_targets(Some(scope_id.as_str())).await {
                Ok(targets) => {
                    message_tx
                        .send(TargetsPageMessage::TargetsLoaded(targets).into())
                        .await
                        .unwrap();
                }
                Err(e) => {
                    message_tx
                        .send(Message::ShowAlert(
                            "Error".to_string(),
                            format!("Failed to load targets: {e}"),
                        ))
                        .await
                        .unwrap();
                }
            }
        }
        .boxed();
        self.message_tx
            .send(Message::RunFuture(future))
            .await
            .unwrap();
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
        let selected_item = self.table_page.selected_item().unwrap();
        let remembered_port: Option<u16> = self
            .remember_user_input
            .get_local_port(&selected_item.id)
            .unwrap_or(None);
        let default_port = self
            .table_page
            .selected_item()
            .as_ref()
            .and_then(|t| t.default_client_port());

        let suggested_port = remembered_port
            .or(default_port)
            .map(|p| p.to_string())
            .unwrap_or_else(|| "".to_string());

        self.connect_dialog = Some(InputDialog::new(
            "Connect",
            vec![InputField::new(
                ConnectDialogFields::ListenPort,
                "Listen Port",
                suggested_port,
            )],
            vec![
                Button::new(ConnectDialogButtons::Cancel, "Cancel"),
                Button::new(ConnectDialogButtons::Ok, "Ok"),
            ],
        ));
    }

    fn close_connect_dialog(&mut self) {
        self.connect_dialog = None;
    }

    pub fn connection_establised(&mut self, response: ConnectResponse) {
        self.connect_result_dialog = Some(ConnectionResultDialog::new(
            response,
            self.message_tx.clone(),
        ));
    }

    async fn connect_to_target(&mut self) {
        if let Some(target) = self.table_page.selected_item() {
            let port: u16 = self
                .connect_dialog
                .as_ref()
                .unwrap()
                .get_value(ConnectDialogFields::ListenPort)
                .unwrap()
                .parse()
                .unwrap();
            self.store_selected_port(port);
            let _ = self
                .message_tx
                .send(Message::Connect {
                    target_id: target.id.clone(),
                    port,
                })
                .await
                .unwrap();
            self.connect_dialog = None;
        }
    }

    fn store_selected_port(&mut self, port: u16) {
        if let Some(target) = self.table_page.selected_item() {
            let _ = self
                .remember_user_input
                .store_local_port(target.id.clone(), port);
        }
    }

    async fn show_sessions(&mut self) {
        if let Some(target) = self.table_page.selected_item() {
            self.message_tx
                .send(Message::ShowSessions {
                    scope: target.scope_id.clone(),
                    target: (*target).clone(),
                })
                .await
                .unwrap();
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
                }
                KeyCode::Char('C') => {
                    // Show sessions for the selected target if possible
                    if self.table_page.selected_item().is_some() {
                        self.show_sessions().await;
                    }
                }
                KeyCode::Esc => {
                    // Go back only if no dialogs are open
                    self.message_tx.send_or_expect(GoBack).await;
                }
                _ => {}
            }
        }
    }

    pub fn handle_message(&mut self, message: TargetsPageMessage) {
        match message {
            TargetsPageMessage::ConnectedToTarget(response) => {
                self.connection_establised(response);
            }
            TargetsPageMessage::TargetsLoaded(targets) => {
                self.table_page.loading = false;
                self.table_page.set_items(targets);
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
