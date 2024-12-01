mod commands;
mod filter;
pub mod scope;
pub mod sessions;
pub mod target;

use crate::ext::TableStateExt;
use crate::router::Router;
use crate::routes::Routes;
use crossterm::event::{Event, KeyCode};
use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::style::{Color, Style, Stylize};

use crate::components::table::commands::HasCommands;
use crate::components::table::filter::Filter;
use ratatui::text::{Line, Span};
use ratatui::widgets::block::{Position, Title};
use ratatui::widgets::{Block, Paragraph, Row, Table, TableState};
use ratatui::Frame;
use std::cell::RefCell;
use std::rc::Rc;
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;

pub trait SortItems<T> {
    fn sort(items: &mut Vec<Rc<T>>);
}

pub trait FilterItems<T> {
    fn match_str(value: &str, search: &str) -> bool {
        value.to_lowercase().contains(&search.to_lowercase())
    }

    fn matches(item: &T, search: &str) -> bool;
}

struct TableColumn<T> {
    header: String,
    width: Constraint,
    get_value: Box<dyn Fn(&T) -> String>,
}

impl<T> TableColumn<T> {
    fn new(header: String, width: Constraint, get_value: Box<dyn Fn(&T) -> String>) -> Self {
        TableColumn {
            header,
            width,
            get_value,
        }
    }
}

pub struct TablePage<'a, T> {
    title: String,
    items: Vec<Rc<T>>,
    columns: Vec<TableColumn<T>>,
    visible_items: Vec<Rc<T>>,
    table_state: RefCell<TableState>,
    filter: Filter,
    _router: &'a RefCell<Router<Routes>>,
}

impl<'a, T> TablePage<'a, T>
where
    T: HasCommands,
{
    fn new(
        title: String,
        items: Vec<Rc<T>>,
        columns: Vec<TableColumn<T>>,
        router: &'a RefCell<Router<Routes>>,
    ) -> Self {
        let table_state = TableState::new();
        let visible_items = items.to_vec();
        TablePage {
            title,
            items,
            columns,
            visible_items,
            table_state: RefCell::new(table_state),
            filter: Filter::Disabled,
            _router: router,
        }
    }

    pub fn update_items(&mut self, mut items: Vec<Rc<T>>)
    where
        Self: SortItems<T>,
    {
        Self::sort(&mut items);
        self.items = items;
        self.visible_items = self.items.to_vec();
        self.table_state.borrow_mut().select(Some(0));
    }

    fn instructions(&self) -> Title<'a> {
        let mut spans: Vec<Span> = T::commands()
            .iter()
            .map(|c| {
                let span = Span::from(format!("  {}<{}>  ", c.name, c.shortcut));
                if self
                    .selected_item()
                    .map(|s| s.is_enabled(c.id))
                    .unwrap_or(false)
                {
                    span
                } else {
                    span.fg(Color::DarkGray)
                }
            })
            .collect();

        let mut back = Span::from("  Back<ESC>  ");
        if !self._router.borrow().can_go_back() {
            back = back.fg(Color::DarkGray);
        }
        spans.insert(0, back);
        spans.insert(0, Span::from("  Quit <Ctrl + C>  "));
        Title::from(Line::from(spans))
    }

    fn rows(&self) -> Vec<Row> {
        self.visible_items
            .iter()
            .map(|i| {
                self.columns
                    .iter()
                    .map(|c| (c.get_value)(i.as_ref()))
                    .collect()
            })
            .collect()
    }

    fn table(&self) -> Table {
        let title = Title::from(self.title.clone().bold());

        let rows: Vec<Row> = self.rows();

        let block = Block::bordered()
            .title(title.alignment(Alignment::Center))
            .title(
                self.instructions()
                    .position(Position::Bottom)
                    .alignment(Alignment::Center),
            )
            .light_blue()
            .bg(Color::Black);
        let header_items: Vec<Span> = self
            .columns
            .iter()
            .map(|c| c.header.clone().bold().fg(Color::White))
            .collect();
        let header = Row::new(header_items);

        let width_constraints: Vec<Constraint> = self.columns.iter().map(|c| c.width).collect();
        Table::new(rows, width_constraints)
            .header(header)
            .highlight_style(Style::new().reversed())
            .block(block)
    }

    pub fn selected_item(&self) -> Option<Rc<T>> {
        let selected_index = self.table_state.borrow().selected_coerced();
        self.visible_items.get(selected_index).cloned()
    }

    fn apply_filter(&mut self, filter: &str)
    where
        Self: FilterItems<T>,
    {
        self.visible_items = self
            .items
            .iter()
            .filter(|i| Self::matches(i.as_ref(), filter))
            .map(Rc::clone)
            .collect();
        self.table_state.borrow_mut().select(Some(0));
    }

    fn reset_filter(&mut self) {
        self.filter = Filter::Disabled;
        self.visible_items = self.items.iter().cloned().collect();
        self.table_state.borrow_mut().select(Some(0));
    }

    fn handle_search_input(&mut self, event: &Event) -> bool
    where
        Self: FilterItems<T>,
    {
        if let Filter::Input(filter_input) = &mut self.filter {
            if let Event::Key(key_event) = event {
                match key_event.code {
                    KeyCode::Enter => {
                        self.filter = Filter::Value(filter_input.value().to_string());
                    }
                    KeyCode::Esc => {
                        self.reset_filter();
                    }
                    _ => {
                        filter_input.handle_event(event);
                        let value = filter_input.value().to_string();
                        self.apply_filter(&value);
                        self.table_state.borrow_mut().select(Some(0));
                    }
                }
            }
            return true;
        }
        false
    }

    fn show_filter_input(&mut self) {
        let filter_string = if let Filter::Value(value) = &self.filter {
            value.clone()
        } else {
            "".to_string()
        };
        self.filter = Filter::Input(Input::new(filter_string));
    }

    fn handle_event(&mut self, event: &Event) -> bool
    where
        Self: FilterItems<T>,
    {
        if self.handle_search_input(event) {
            return true;
        }

        if let Event::Key(event) = event {
            match event.code {
                KeyCode::Esc => {
                    if self.filter.is_active() {
                        self.reset_filter();
                        return true;
                    }
                }
                KeyCode::Up => {
                    let selected_index = self.table_state.borrow().selected_coerced();
                    if selected_index > 0 {
                        self.table_state
                            .borrow_mut()
                            .select(Some(selected_index - 1));
                    }
                    return true;
                }
                KeyCode::Down => {
                    let selected_index = self.table_state.borrow().selected_coerced();
                    if !self.visible_items.is_empty()
                        && selected_index < self.visible_items.len() - 1
                    {
                        self.table_state
                            .borrow_mut()
                            .select(Some(selected_index + 1));
                    }
                    return true;
                }
                KeyCode::Char('/') => {
                    self.show_filter_input();
                    return true;
                }
                _ => {}
            };
        }
        false
    }

    fn render(&self, frame: &mut Frame) {
        let layout_constraints = if self.filter.is_input() {
            [Constraint::Length(3), Constraint::Fill(1)]
        } else {
            [Constraint::Length(0), Constraint::Fill(1)]
        };

        let [search_area, table_area] = Layout::vertical(layout_constraints).areas(frame.area());

        if let Filter::Input(search) = &self.filter {
            let block = Block::bordered().light_blue().on_black();
            let paragraph = Paragraph::new(format!("🔍{}", search.value()))
                .block(block)
                .alignment(Alignment::Left);
            frame.render_widget(paragraph, search_area);
        }

        frame.render_stateful_widget(self.table(), table_area, &mut self.table_state.borrow_mut());
    }
}
