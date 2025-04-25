mod component;

pub use component::Component;
use std::future::Future;
use std::io;

pub trait UpdateState<M, MOut> {
    fn update(&mut self, message: &M) -> impl Future<Output = Option<MOut>>;
}


pub trait Application<M>: Component<M> + UpdateState<M, M> {
    
    async fn run(&mut self, message: Option<M>) -> io::Result<()> {
        let mut terminal = ratatui::init();
        terminal.clear()?;

        if let Some(message) = message {
            terminal.draw(|frame| {
                self.view(frame);
            })?;
            self.update(&message).await;
        }

        loop {
            terminal.draw(|frame| {
                self.view(frame);
            })?;
            let event = crossterm::event::read()?;
            if let Some(mut message) = self.handle_event(&event) {
                while let Some(new_message) = self.update(&message).await {
                    message = new_message;
                }
            }
        }
        Ok(())
    }

}