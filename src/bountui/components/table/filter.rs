use tui_input::Input;

pub enum Filter {
    Disabled,
    Input(Input),
    Value(String),
}

impl Filter {
    pub fn is_input(&self) -> bool {
        matches!(self, Filter::Input(_))
    }

    pub fn is_active(&self) -> bool {
        matches!(self, Filter::Input(_) | Filter::Value(_))
    }
}

impl Default for Filter {
    fn default() -> Self {
        Filter::Disabled
    }
}