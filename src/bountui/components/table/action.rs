
pub struct Action<T> {
    pub name: String,
    pub shortcut: String,
    pub enabled: Box<dyn Fn(Option<&T>) -> bool>,
}

impl<T> Action<T> {
    pub fn new(
        name: String,
        shortcut: String,
        enabled: Box<dyn Fn(Option<&T>) -> bool>,
    ) -> Self {
        Self {
            name,
            shortcut,
            enabled,
        }
    }
}
