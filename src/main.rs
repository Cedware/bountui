mod app;
mod boundary;
mod bountui;
mod components;
pub mod connection_manager;
mod ext;
mod router;
mod routes;
mod widgets;

use crate::boundary::ApiClient;
use crate::bountui::Bountui;
use crate::components::Alerts;
use crate::connection_manager::ConnectionManager;
use crate::router::Router;
use crate::routes::Routes;
use crossterm::event;
use std::cell::RefCell;
use std::io;

fn run_blocking() -> io::Result<()> {
    let client = boundary::CliClient::default();
    let auth_result = tokio::runtime::Handle::current().block_on(client.authenticate());
    let user_id = match auth_result {
        Ok(auth) => auth.attributes.user_id,
        Err(e) => {
            eprintln!("Failed to authenticate: {}", e);
            return Ok(());
        }
    };
    let mut terminal = ratatui::init();
    terminal.clear()?;
    let router = RefCell::new(Router::new(Routes::Scopes { parent: None }));
    let connection_manager = ConnectionManager::new(&client);
    let alerts = Alerts::default();
    let mut app = Bountui::new(&client, user_id, &router, &connection_manager, &alerts);
    while !app.finished {
        terminal.draw(|frame| {
            app.render(frame);
        })?;

        let event = event::read()?;
        app.handle_event(&event);
    }
    terminal.clear()?;
    Ok(())
}

#[tokio::main]
async fn main() -> io::Result<()> {
    tokio::task::block_in_place(run_blocking)
}
