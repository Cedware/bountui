use ratatui::widgets::TableState;

pub trait TableStateExt  {

    fn selected_coerced(&self) -> usize;

}

impl TableStateExt for TableState {
    fn selected_coerced(&self) -> usize {
        self.selected().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use ratatui::widgets::TableState;
    use crate::ext::table_state::TableStateExt;

    #[test]
    fn test_selected_coerced() {
        let mut table_state = TableState::new();
        table_state.select(None);
        assert_eq!(table_state.selected_coerced(), 0);
        table_state.select(Some(3));
        assert_eq!(table_state.selected_coerced(), 3);
    }

}