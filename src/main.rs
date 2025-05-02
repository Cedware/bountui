mod boundary;
mod bountui;
pub mod event_ext;
mod util;

use std::env;
use crossterm::event::Event;
use tokio::select;
use crate::boundary::ApiClient;
use crate::bountui::BountuiApp;


fn receive_cross_term_events() -> tokio::sync::mpsc::Receiver<Event> {

    let (sender, receiver) = tokio::sync::mpsc::channel(100);
    tokio::task::spawn(async move {
        loop {
            if let Ok(event) = crossterm::event::read() {
                if sender.send(event).await.is_err() {
                    break;
                }
            }
        }
    });
    receiver
}

#[tokio::main]
async fn main() {
    let (send_message, mut receive_message) = tokio::sync::mpsc::channel(100);
    let boundary_client = boundary::CliClient::default();
    let connection_manager = bountui::connection_manager::ConnectionManager::new(boundary_client.clone());
    let auth_result = boundary_client.authenticate().await.unwrap();
    env::set_var("BOUNDARY_TOKEN", auth_result.attributes.token);

    let mut app = BountuiApp::new(boundary_client, connection_manager, send_message).await;
    let mut terminal = ratatui::init();
    terminal.clear().unwrap();

    let mut cross_term_event_receiver = receive_cross_term_events();


    while !app.is_finished {
        terminal.draw(|frame| {
            app.view(frame);
        }).unwrap();

        select! {
            message = receive_message.recv() => {
                if let Some(message) = message {
                    app.handle_message(message).await;
                }
            }
            event = cross_term_event_receiver.recv() => {
                if let Some(event) = event {
                    app.handle_event(&event).await;
                }
            }
        }
    }


}