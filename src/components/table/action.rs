pub struct Action<Id>
where
    Id: Copy,
{
    pub id: Id,
    pub name: String,
    pub shortcut: String,
}

impl<Id> Action<Id>
where
    Id: Copy,
{
    pub fn new(id: Id, name: String, shortcut: String) -> Self {
        Self { id, name, shortcut }
    }
}
