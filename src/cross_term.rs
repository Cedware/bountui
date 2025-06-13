use crossterm::event::Event;

pub fn receive_cross_term_events() -> tokio::sync::mpsc::Receiver<Event> {

    let (sender, receiver) = tokio::sync::mpsc::channel(10);
    tokio::task::spawn(async move {
        loop {
            if let Ok(event) = crossterm::event::read() {
                if let Err(_) = sender.send(event).await {
                    break;
                }
            }
            else { 
                break;
            }
        }
    });
    receiver
}