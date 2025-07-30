use crossterm::event::{Event, KeyEventKind};

pub fn receive_cross_term_events() -> tokio::sync::mpsc::Receiver<Event> {

    let (sender, receiver) = tokio::sync::mpsc::channel(10);
    tokio::task::spawn(async move {
        loop {
            if let Ok(event) = crossterm::event::read() {
                
                    if let Event::Key(key_event) = event {
                        if key_event.kind == KeyEventKind::Press {
                            if let Err(_) = sender.send(event).await {
                                break;
                            }
                        }
                    }
                
            }
            else { 
                break;
            }
        }
    });
    receiver
}