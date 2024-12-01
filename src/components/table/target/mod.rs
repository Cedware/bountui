use crate::boundary;
use crate::components::table::commands::{Command, HasCommands};
use crate::components::table::{FilterItems, SortItems, TableColumn};
use crate::components::{Alerts, TablePage};
use crate::router::Router;
use crate::routes::Routes;
use crossterm::event::{Event, KeyCode};

use crate::boundary::Target;
use crate::components::input_dialog::{Button, InputDialog, InputField};
use crate::connection_manager::ConnectionManager;
use crate::widgets::ConnectionResultDialog;
use ratatui::layout::Constraint;
use ratatui::Frame;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone, Copy)]
pub enum Commands {
    Connect,
    ShowConnections,
}

impl HasCommands for boundary::Target {
    type Id = Commands;

    fn commands() -> Vec<Command<Self::Id>> {
        vec![
            Command::new(Commands::Connect, "Connect".to_string(), "c".to_string()),
            Command::new(
                Commands::ShowConnections,
                "Show connections".to_string(),
                "Shift+C".to_string(),
            ),
        ]
    }

    fn is_enabled(&self, id: Self::Id) -> bool {
        match id {
            Commands::Connect => self
                .authorized_actions
                .contains(&"authorize-session".to_string()),
            Commands::ShowConnections => true,
        }
    }
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
enum ConnectDialogFields {
    ListenPort,
}

#[derive(Clone, Copy)]
enum ConnectDialogButtons {
    Cancel,
    Ok,
}

pub struct TargetsPage<'a, C> {
    parent_scope_id: Option<String>,
    boundary_client: &'a C,
    table_page: TablePage<'a, boundary::Target>,
    handle: tokio::runtime::Handle,
    connection_manager: &'a ConnectionManager<'a, C>,
    connect_dialog: Option<InputDialog<ConnectDialogButtons, ConnectDialogFields>>,
    connect_result: Option<Result<boundary::ConnectResponse, boundary::Error>>,
    router: &'a RefCell<Router<Routes>>,
    alerts: &'a Alerts,
}

impl<'a, C> TargetsPage<'a, C> {
    pub fn new(
        parent_scope_id: Option<String>,
        boundary_client: &'a C,
        router: &'a RefCell<Router<Routes>>,
        connection_manager: &'a ConnectionManager<'a, C>,
        alerts: &'a Alerts,
    ) -> Self
    where
        C: boundary::ApiClient,
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

        let table_page = TablePage::new("Targets".to_string(), vec![], columns, router);
        let mut page = TargetsPage {
            parent_scope_id,
            boundary_client,
            table_page,
            connect_dialog: None,
            connect_result: None,
            handle: tokio::runtime::Handle::current(),
            connection_manager,
            router,
            alerts,
        };
        page.load();
        page
    }

    fn load(&mut self)
    where
        C: boundary::ApiClient,
    {
        let targets = self
            .handle
            .block_on(self.boundary_client.get_targets(&self.parent_scope_id))
            .map(|targets| targets.into_iter().map(Rc::new).collect());
        match targets {
            Ok(targets) => self.table_page.update_items(targets),
            Err(_) => {
                self.alerts
                    .alert("Error".to_string(), "Failed to load targets".to_string());
            }
        }
    }

    fn show_connect_dialog(&mut self) {
        self.connect_dialog = Some(InputDialog::new(
            "Create Target",
            vec![InputField::new(
                ConnectDialogFields::ListenPort,
                "Listen Port",
                "",
            )],
            vec![
                Button::new(ConnectDialogButtons::Cancel, "Cancel"),
                Button::new(ConnectDialogButtons::Ok, "Ok"),
            ],
        ));
    }

    fn connect(&mut self)
    where
        C: boundary::ApiClient,
    {
        if let Some(connect_dialog) = &self.connect_dialog {
            if let Some(selected_item) = self.table_page.selected_item() {
                //todo: handle error
                let connect_future = self.connection_manager.connect(
                    selected_item.as_ref(),
                    connect_dialog
                        .value(ConnectDialogFields::ListenPort)
                        .unwrap()
                        .parse()
                        .unwrap(),
                );
                self.connect_dialog = None;
                self.connect_result = Some(self.handle.block_on(connect_future));
            }
        }
    }

    pub fn handle_event(&mut self, event: &Event) -> bool
    where
        C: boundary::ApiClient,
    {
        if self.connect_result.is_some() {
            if let Event::Key(key_event) = event {
                if key_event.code == crossterm::event::KeyCode::Enter {
                    self.connect_result = None;
                    return true;
                }
            }
        }

        if let Some(connect_dialog) = self.connect_dialog.as_mut() {
            if let Some(button_id) = connect_dialog.handle_event(event) {
                match button_id {
                    ConnectDialogButtons::Cancel => self.connect_dialog = None,
                    ConnectDialogButtons::Ok => self.connect(),
                }
            }
            return true;
        }

        if let Event::Key(key_event) = event {
            match key_event.code {
                KeyCode::Char('C') => {
                    if let Some(selected_item) = self.table_page.selected_item() {
                        self.router.borrow_mut().push(Routes::Sessions {
                            scope_id: selected_item.scope_id.clone(),
                            target_id: selected_item.id.clone(),
                        });
                        return true;
                    }
                }
                KeyCode::Char('c') => {
                    self.show_connect_dialog();
                    return true;
                }
                _ => {}
            }
        }

        if self.table_page.handle_event(event) {
            return true;
        }
        false
    }

    pub fn render(&self, frame: &mut Frame) {
        self.table_page.render(frame);
        if let Some(connect_dialog) = &self.connect_dialog {
            connect_dialog.render(frame);
        }
        if let Some(connect_result) = &self.connect_result {
            frame.render_widget(ConnectionResultDialog::new(connect_result), frame.area());
        }
    }
}

impl SortItems<boundary::Target> for TablePage<'_, boundary::Target> {
    fn sort(items: &mut Vec<Rc<boundary::Target>>) {
        items.sort_by(|a, b| a.name.cmp(&b.name));
    }
}

impl FilterItems<boundary::Target> for TablePage<'_, boundary::Target> {
    fn matches(item: &Target, search: &str) -> bool {
        Self::match_str(&item.name, search)
            || Self::match_str(&item.description, search)
            || Self::match_str(&item.id, search)
    }
}
