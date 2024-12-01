pub struct Command<Id>
where
    Id: Copy,
{
    pub id: Id,
    pub name: String,
    pub shortcut: String,
}

impl<Id> Command<Id>
where
    Id: Copy,
{
    pub fn new(id: Id, name: String, shortcut: String) -> Self {
        Self { id, name, shortcut }
    }
}

pub trait HasCommands {
    type Id: Copy;
    fn commands() -> Vec<Command<Self::Id>>;
    fn is_enabled(&self, id: Self::Id) -> bool;
}
