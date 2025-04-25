use crossterm::event::Event;
use ratatui::Frame;

pub struct HandleEventResult<M> {
    pub processed: bool,
    pub message: Option<M>
}

pub trait Component<M> {

    fn view(&self, frame: &mut Frame);
    fn handle_event(&self, event: &Event) -> Option<M>;

}