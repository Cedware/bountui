use std::rc::Rc;

pub struct Action<T, Id>
where
    Id: Copy,
{
    pub id: Id,
    pub name: String,
    pub shortcut: String,
    pub enabled: Box<dyn Fn(Option<&T>) -> bool>,
}

impl<T, Id> Action<T, Id>
where
    Id: Copy,
{
    pub fn new(
        id: Id,
        name: String,
        shortcut: String,
        enabled: Box<dyn Fn(Option<&T>) -> bool>,
    ) -> Self {
        Self {
            id,
            name,
            shortcut,
            enabled,
        }
    }
}
