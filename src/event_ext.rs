use crossterm::event::Event;

pub trait EventExt {
    
    fn is_enter(&self) -> bool;
    fn is_stop(&self) -> bool;
    
}

impl EventExt for Event {
    
    fn is_enter(&self) -> bool {
        match self {
            Event::Key(key_event) => key_event.code == crossterm::event::KeyCode::Enter,
            _ => false
        }
    }

    fn is_stop(&self) -> bool {
        match self {
            Event::Key(key_event) => key_event.code == crossterm::event::KeyCode::Char('c') && key_event.modifiers == crossterm::event::KeyModifiers::CONTROL,
            _ => false
        }
    }
}