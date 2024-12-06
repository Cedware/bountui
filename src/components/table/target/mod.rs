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
use crate::widgets::ConnectResponseDialog;
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
    connect_response: Option<boundary::ConnectResponse>,
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
            connect_response: None,
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

                let connect_future = self.connection_manager.connect(
                    selected_item.as_ref(),
                    connect_dialog
                        .value(ConnectDialogFields::ListenPort)
                        .unwrap()
                        .parse()
                        .unwrap(),
                );
                self.connect_dialog = None;
                match self.handle.block_on(connect_future) {
                    Ok(resp) => self.connect_response = Some(resp),
                    Err(e) => self.alerts.alert("Error", format!("Failed to connect:\n {e}"))
                }
            }
        }
    }

    pub fn handle_event(&mut self, event: &Event) -> bool
    where
        C: boundary::ApiClient,
    {
        if self.connect_response.is_some() {
            if let Event::Key(key_event) = event {
                if key_event.code == crossterm::event::KeyCode::Enter {
                    self.connect_response = None;
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

        if self.table_page.handle_event(event) {
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

        false
    }

    pub fn render(&self, frame: &mut Frame) {
        self.table_page.render(frame);
        if let Some(connect_dialog) = &self.connect_dialog {
            connect_dialog.render(frame);
        }


        if let Some(connect_result) = &self.connect_response {
            frame.render_widget(ConnectResponseDialog::new(connect_result), frame.area());
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


#[cfg(test)]
mod test {
    use crate::boundary;
    use crate::boundary::{ConnectResponse, Target};
    use crate::components::input_dialog::{Button, InputDialog, InputField};
    use crate::components::table::target::{ConnectDialogButtons, ConnectDialogFields, TargetsPage};
    use crate::components::Alerts;
    use crate::connection_manager::ConnectionManager;
    use crate::router::Router;
    use crate::routes::Routes;
    use std::cell::RefCell;

    fn connect_dialog() -> InputDialog<ConnectDialogButtons, ConnectDialogFields> {
        InputDialog::new(
            "Create Target",
            vec![InputField::new(
                ConnectDialogFields::ListenPort,
                "Listen Port",
                "8080",
            )],
            vec![
                Button::new(ConnectDialogButtons::Ok, "Ok"),
                Button::new(ConnectDialogButtons::Cancel, "Cancel"),
            ],
        )
    }

    fn targets() -> Vec<Target> {
        vec![
            Target {
                id: "1".to_string(),
                name: "Target 1".to_string(),
                description: "Description 1".to_string(),
                type_name: "Type 1".to_string(),
                scope_id: "1".to_string(),
                authorized_actions: vec!["authorize-session".to_string()],
                authorized_collection_actions: Default::default(),
            },
            Target {
                id: "2".to_string(),
                name: "Target 2".to_string(),
                description: "Description 2".to_string(),
                type_name: "Type 2".to_string(),
                scope_id: "2".to_string(),
                authorized_actions: vec!["authorize-session".to_string()],
                authorized_collection_actions: Default::default(),
            },
        ]
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_connect_failure() {


        let mut boundary_client = boundary::MockApiClient::new();
        boundary_client
            .expect_get_targets()
            .return_once(|_| Box::pin(async { Ok(targets()) }));
        boundary_client
            .expect_connect()
            .return_once(|_, _, _| {
                Box::pin(async { Err(boundary::Error::ApiError(1, "Some error".to_string())) })
            });

        let router = RefCell::new(Router::new(Routes::Targets {scope: "".to_string()}));
        let connection_manager = ConnectionManager::new(&boundary_client);
        let alerts = Alerts::default();
        

        tokio::task::block_in_place(||{
            let mut sut = TargetsPage::new(
                None,
                &boundary_client,
                &router,
                &connection_manager,
                &alerts
            );

            sut.connect_dialog = Some(connect_dialog());
            sut.connect();

            assert_eq!(alerts.alerts.borrow().len(), 1, "Alert should be displayed");
            assert!(sut.connect_dialog.is_none(), "Connect dialog should be closed");
            assert!(sut.connect_response.is_none(), "Connect response should be none")
        });
    }


    #[tokio::test(flavor = "multi_thread")]
    async fn test_connect_success() {


        let mut boundary_client = boundary::MockApiClient::new();
        boundary_client
            .expect_get_targets()
            .return_once(|_| Box::pin(async { Ok(targets()) }));
        boundary_client
            .expect_connect()
            .return_once(|_, _, _| {
                Box::pin(async { Ok(ConnectResponse{
                    session_id: "session_id".to_string(),
                    credentials: Default::default() })
                })
            });

        let router = RefCell::new(Router::new(Routes::Targets {scope: "".to_string()}));
        let connection_manager = ConnectionManager::new(&boundary_client);
        let alerts = Alerts::default();


        tokio::task::block_in_place(||{
            let mut sut = TargetsPage::new(
                None,
                &boundary_client,
                &router,
                &connection_manager,
                &alerts
            );

            sut.connect_dialog = Some(connect_dialog());
            sut.connect();

            assert_eq!(alerts.alerts.borrow().len(), 0, "Alert should not be displayed");
            assert!(sut.connect_dialog.is_none(), "Connect dialog should be closed");
            assert!(sut.connect_response.is_some(), "Connect response should be none")
        });


    }
    
}