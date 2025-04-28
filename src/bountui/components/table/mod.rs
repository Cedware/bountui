mod action;
mod filter;
pub mod scope;
pub mod sessions;
pub mod target;

use crossterm::event::{Event, KeyCode};
use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::style::{Color, Style, Stylize};

use crate::bountui::components::table::action::Action;
use crate::bountui::components::table::filter::Filter;
use ratatui::text::{Line, Span};
use ratatui::widgets::block::{Position, Title};
use ratatui::widgets::{Block, Paragraph, Row, Table};
use ratatui::Frame;
use std::rc::Rc;
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;
use crate::bountui::Message;

pub trait SortItems<T> {
    fn sort(items: &mut Vec<Rc<T>>);
}

pub trait FilterItems<T> {
    fn match_str(value: &str, search: &str) -> bool {
        value.to_lowercase().contains(&search.to_lowercase())
    }

    fn matches(item: &T, search: &str) -> bool;
}

pub trait HasActions<T> {
    type Id: Copy;

    fn actions(&self) -> Vec<Action<Self::Id>>;
    fn is_action_enabled(&self, id: Self::Id, item: &T) -> bool;
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

pub struct TablePage<T> {
    title: String,
    columns: Vec<TableColumn<T>>,
    items: Vec<Rc<T>>,
    visible_items: Vec<Rc<T>>,
    selected: Option<usize>,
    filter: Filter,
    can_go_back: bool,
}
impl<T> TablePage<T> {
    fn new(title: String, columns: Vec<TableColumn<T>>, items: Vec<T>) -> Self {
        let items: Vec<Rc<T>> = items.into_iter().map(Rc::new).collect();
        let visible_items: Vec<Rc<T>> = items.iter().cloned().collect();
        let selected = if visible_items.is_empty() { None } else { Some(0) };
        TablePage {
            title,
            columns,
            items,
            visible_items,
            selected,
            filter: Filter::Disabled,
            can_go_back: false,
        }
    }
    
    pub fn set_items(&mut self, items: Vec<T>) {
        self.items = items.into_iter().map(Rc::new).collect();
        self.visible_items = self.items.iter().cloned().collect();
        self.selected = if self.visible_items.is_empty() { None } else { Some(0) };
    }

    fn selected_item(&self) -> Option<Rc<T>> {
        self.selected
            .map(|i| self.visible_items.get(i).cloned())
            .flatten()
    }

    fn reset_filter(&mut self) {
        self.filter = Filter::Disabled;
        self.visible_items = self.items.iter().cloned().collect();
        self.selected = Some(0);
    }

    fn update_filter(&mut self, event: &Event) where TablePage<T>: FilterItems<T>  {
        if let Filter::Input(filter_input) = &mut self.filter {
            filter_input.handle_event(event);
            let value = filter_input.value().to_string();
            self.visible_items = self
                .items
                .iter()
                .filter(|i| Self::matches(i.as_ref(), &value))
                .map(Rc::clone)
                .collect();
            self.selected = Some(0);
        }
    }

    fn select_next(&mut self) {
        if let Some(selected_index) = self.selected {
            if selected_index < self.visible_items.len() - 1 {
                self.selected = Some(selected_index + 1);
            }
        }
    }

    fn select_previous(&mut self) {
        if let Some(selected_index) = self.selected {
            if selected_index > 0 {
                self.selected = Some(selected_index - 1);
            }
        }
    }

    fn show_filter(&mut self) {
        self.filter = Filter::Input(Input::new("".to_string()));
    }

    fn hide_filter(&mut self) {
        if let Filter::Input(filter_input) = &self.filter {
            self.filter = Filter::Value(filter_input.value().to_string());
        }
    }

    fn instructions(&self) -> Title
    where
        Self: HasActions<T>,
    {
        let mut spans: Vec<Span> = self
            .actions()
            .iter()
            .map(|c| {
                let span = Span::from(format!("  {}<{}>  ", c.name, c.shortcut));
                if self
                    .selected_item()
                    .map(|s| self.is_action_enabled(c.id, s.as_ref()))
                    .unwrap_or(false)
                {
                    span
                } else {
                    span.fg(Color::DarkGray)
                }
            })
            .collect();

        let mut back = Span::from("  Back<ESC>  ");
        if self.can_go_back {
            back = back.fg(Color::DarkGray);
        }
        spans.insert(0, back);
        spans.insert(0, Span::from("  Quit <Ctrl + C>  "));
        Title::from(Line::from(spans))
    }

    fn rows(&self) -> Vec<Row> {
        self
            .visible_items
            .iter()
            .map(|i| {
                self.columns
                    .iter()
                    .map(|c| (c.get_value)(i.as_ref()))
                    .collect()
            })
            .collect()
    }

    fn table(&self) -> Table
    where
        Self: HasActions<T>,
    {
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

    fn handle_event(&mut self, event: &Event) where TablePage<T>: FilterItems<T> {
        if self.filter.is_input() {
            if let Event::Key(key_event) = event {
                if let KeyCode::Enter = key_event.code {
                    self.hide_filter();
                }
            }
            self.update_filter(event);
        }

        if let Event::Key(event) = event {
            match event.code {
                KeyCode::Esc => {
                    if self.filter.is_active() {
                        self.reset_filter();
                    }
                }
                KeyCode::Up => {
                    self.select_previous();
                }
                KeyCode::Down => {
                    self.select_next();
                }
                KeyCode::Char('/') => {
                    self.show_filter();
                }
                _ => {}
            }
        }
    }

    fn view(&self, frame: &mut Frame) where Self: HasActions<T> {
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

        let mut table_state = ratatui::widgets::TableState::new();
        table_state.select(self.selected);
        frame.render_stateful_widget(self.table(), table_area, &mut table_state);
    }

    pub fn is_filter_input_active(&self) -> bool {
        self.filter.is_input()
    }

}
