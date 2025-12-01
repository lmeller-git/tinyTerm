use alloc::{boxed::Box, string::String, vec, vec::Vec};
use ratatui::{
    Terminal,
    prelude::Backend,
    style::{Style, Stylize},
    text::Line,
    widgets::{Block, BorderType, Padding, Paragraph, Wrap},
};
use vte::ansi::Handler;

use crate::parse::Config;

const MAX_LINES: usize = 128;
const GRACE: usize = 16;
const MAX_VISIBLE_LINES: usize = 32;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Default, Clone, Copy)]
struct Cursor {
    row: usize,
    col: usize,
}

// may want to use VecDeque, if drain takes too long
pub struct TermState<B: Backend> {
    terminal: Terminal<B>,
    config: Config,
    rows: Vec<String>,
    visible: (usize, usize),
    _cursor: Cursor,
}

impl<B: Backend> TermState<B> {
    pub fn new(backend: B) -> Self {
        Self {
            terminal: Terminal::new(backend).unwrap(),
            config: Config::new(),
            _cursor: Cursor::default(),
            rows: vec![String::new()],
            visible: (0, 1),
        }
    }

    pub fn update_state(&mut self, line: &str) {
        self.rows.last_mut().unwrap().replace_range(.., line);
        self.draw();
    }

    pub fn commit(&mut self) {
        self.rows.push(String::new());
        if self.rows.len() + GRACE >= MAX_LINES {
            self.rows.drain(..GRACE);
        }
        self.visible.1 = self.rows.len();
        self.visible.0 = MAX_VISIBLE_LINES.min(self.rows.len().saturating_sub(1));
    }

    fn draw(&mut self) {
        let lines = self
            .rows
            .iter()
            .skip(self.visible.0)
            .take(self.visible.1)
            .map(|line| Line::raw(line))
            .collect::<Vec<Line>>();

        self.terminal
            .draw(|frame| {
                let block = Block::bordered()
                    .border_style(Style::new().fg(self.config.border()).bg(self.config.bg()))
                    .bg(self.config.bg())
                    .title_top(
                        Line::from("Terminal")
                            .centered()
                            .bold()
                            .fg(self.config.title()),
                    )
                    .border_type(BorderType::Rounded)
                    .padding(Padding::new(5, 5, 5, 5));
                let paragraph = Paragraph::new(lines)
                    .block(block)
                    .wrap(Wrap { trim: true })
                    .fg(self.config.text())
                    .bg(self.config.bg());
                frame.render_widget(paragraph, frame.area())
            })
            .unwrap();
    }
}

pub struct EventPacket {
    events: Vec<Event>,
}

impl EventPacket {
    pub fn new(events: Vec<Event>) -> Self {
        Self { events }
    }
}

impl<E: Into<Event>> From<E> for EventPacket {
    fn from(value: E) -> Self {
        Self::new(vec![value.into()])
    }
}

impl From<Vec<Event>> for EventPacket {
    fn from(value: Vec<Event>) -> Self {
        Self::new(value)
    }
}

impl Default for EventPacket {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

pub enum Event {
    CharStream(Vec<char>),
    Command(Box<dyn Fn(&mut dyn Handler) + Send>),
}

impl Event {}

impl From<Box<dyn Fn(&mut dyn Handler) + Send>> for Event {
    fn from(value: Box<dyn Fn(&mut dyn Handler) + Send>) -> Self {
        Self::Command(value)
    }
}

pub struct EmulatorState<B: Backend> {
    open_terms: TermState<B>, // TODO add mutliple windows, ...
}

impl<B: Backend> EmulatorState<B> {
    pub fn new(term: TermState<B>) -> Self {
        Self { open_terms: term }
    }

    pub fn handle_event(&mut self, event: EventPacket) {}
}
