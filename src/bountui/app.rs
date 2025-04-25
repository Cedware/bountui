pub enum UpdateResult<R> {
    Handled(R),
    NotHandled,
}

impl<R> UpdateResult<R> {
    pub fn handled(&self) -> bool {
        match self {
            UpdateResult::Handled(_) => true,
            UpdateResult::NotHandled => false,
        }
    }
}
