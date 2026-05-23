use core::{ops::RangeBounds, time::Duration};

use alloc::{string::String, vec, vec::Vec};
use libtinyos::{serial_println, syscalls};
use ratatui::{
    Terminal,
    prelude::Backend,
    style::{Style, Stylize},
    text::Line,
    widgets::{Block, BorderType, Padding, Paragraph, Wrap},
};
use vte::ansi::{Handler, Processor, Timeout};

use crate::parse::Config;

const MAX_LINES: usize = 128;
const GRACE: usize = 16;
const MAX_VISIBLE_LINES: usize = 64;
const TAB_WIDTH: usize = 3;

pub trait EventHandler {
    fn process_events(&mut self, events: EventPacket);
    fn flush(&mut self);
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Default, Clone, Copy)]
struct Cursor {
    row: usize,
    col: usize,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Default, Clone, Copy)]
struct Visible {
    from: usize,
    count: usize,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
enum Dirtyness {
    Full,
    Partial,
    Clean,
}

impl Dirtyness {
    fn up(&mut self) {
        *self = match self {
            Self::Full => Self::Full,
            Self::Partial => Self::Full,
            Self::Clean => Self::Partial,
        }
    }

    fn partial(&mut self) {
        if *self != Self::Full {
            self.up();
        }
    }

    fn full(&mut self) {
        *self = Self::Full
    }
}

struct BufferLine {
    inner: Vec<char>,
}

impl BufferLine {
    fn new() -> Self {
        Self { inner: Vec::new() }
    }

    fn insert(&mut self, idx: usize, value: char) {
        self.inner.insert(idx, value);
    }

    fn remove(&mut self, idx: usize) {
        self.inner.remove(idx);
    }

    fn push(&mut self, value: char) {
        self.inner.push(value);
    }

    fn pop(&mut self) -> Option<char> {
        self.inner.pop()
    }

    #[allow(clippy::inherent_to_string)]
    fn to_string(&self) -> String {
        let mut s = String::with_capacity(self.len());
        for c in self.inner.iter() {
            match c {
                '\t' => s.extend(&[' '; TAB_WIDTH]),
                c => s.push(*c),
            }
        }
        s
    }

    fn len(&self) -> usize {
        self.inner.len()
    }

    fn _is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    fn clear(&mut self) {
        self.inner.clear();
    }

    fn extend(&mut self, items: impl Iterator<Item = char>) {
        self.inner.extend(items);
    }

    fn splice(&mut self, range: impl RangeBounds<usize>, items: impl Iterator<Item = char>) {
        self.inner.splice(range, items);
    }
}

// may want to use VecDeque, if drain takes too long
pub struct TermState<B: Backend> {
    terminal: Terminal<B>,
    config: Config,
    rows: Vec<BufferLine>,
    visible: Visible,
    _cursor: Cursor,
    dirty: Dirtyness,
}

impl<B: Backend> TermState<B> {
    pub fn new(backend: B) -> Self {
        Self {
            terminal: Terminal::new(backend).unwrap(),
            config: Config::new(),
            _cursor: Cursor::default(),
            rows: vec![BufferLine::new()],
            visible: Visible { from: 0, count: 1 },
            dirty: Dirtyness::Full,
        }
    }

    fn parse_stream(&mut self, parser: &mut Processor<SimpleTimeout>, bytes: &[u8]) {
        parser.advance(self, bytes);
    }

    fn add_line(&mut self) {
        self.rows.push(BufferLine::new());
        self.dirty.partial();
        if self.rows.len() + GRACE >= MAX_LINES {
            self.rows.drain(..GRACE);
            self._cursor.row = self._cursor.row.saturating_sub(GRACE);
        }
        if self.rows.len() > MAX_VISIBLE_LINES {
            self.visible.from =
                (self.visible.from + 1).min(self.rows.len().saturating_sub(self.visible.count));
            self.dirty.up();
        }
        self.visible.count = (self.visible.count + 1)
            .min(MAX_VISIBLE_LINES)
            .min(self.rows.len().saturating_sub(self.visible.from))
            .max(1);
        self._cursor.col = 0;
        self._cursor.row = (self._cursor.row + 1).min(self.rows.len() - 1);
    }

    fn draw(&mut self) {
        let lines = self
            .rows
            .iter()
            .skip(self.visible.from)
            .take(self.visible.count)
            .map(|line| Line::raw(line.to_string()))
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
                    .wrap(Wrap { trim: false })
                    .fg(self.config.text())
                    .bg(self.config.bg());
                frame.render_widget(paragraph, frame.area())
            })
            .unwrap();
    }
}

impl<B: Backend> EventHandler for TermState<B> {
    fn process_events(&mut self, events: EventPacket) {
        let mut parser = Processor::<SimpleTimeout>::new();
        for event in &events.events {
            match event {
                Event::CharStream(v) => {
                    let stream = v.iter().collect::<String>();
                    self.parse_stream(&mut parser, stream.as_bytes());
                }
                Event::ByteStream(bytes) => self.parse_stream(&mut parser, bytes),
                Event::String(s) => self.parse_stream(&mut parser, s.as_bytes()),
            }
        }
    }

    fn flush(&mut self) {
        if self.dirty != Dirtyness::Clean {
            self.draw();
            self.dirty = Dirtyness::Clean;
        }
    }
}

impl<B: Backend> Handler for TermState<B> {
    fn input(&mut self, c: char) {
        let Cursor { row, col } = self._cursor;
        let s = self.rows.get_mut(row).unwrap();
        if col == s.len() {
            s.push(c);
        } else {
            s.insert(col, c);
        }
        self.move_forward(1);
        self.dirty.partial();
    }

    fn backspace(&mut self) {
        let Cursor { row, col } = self._cursor;
        let s = self.rows.get_mut(row).unwrap();
        if col == s.len() {
            s.pop();
        } else {
            s.remove(col);
        }
        self.move_backward(1);
        self.dirty.partial();
    }

    fn move_up(&mut self, _: usize) {}

    fn move_backward(&mut self, col: usize) {
        self._cursor.col = self._cursor.col.saturating_sub(col);
        self.dirty.partial();
    }

    fn move_down(&mut self, _: usize) {}

    fn move_forward(&mut self, by: usize) {
        let Cursor { row, col } = self._cursor;
        self._cursor.col = (col + by).min(self.rows.get(row).unwrap().len());
        self.dirty.partial();
    }

    fn set_title(&mut self, _: Option<String>) {}

    fn carriage_return(&mut self) {
        self.rows.last_mut().unwrap().clear();
        self._cursor.col = 0;
        self.dirty.partial();
    }

    fn linefeed(&mut self) {
        serial_println!("[TERM] Line feed");
        self.add_line();
        self.dirty.partial();
    }

    fn newline(&mut self) {
        serial_println!("[TERM] new line");
        self.linefeed();
        self.carriage_return();
        self.dirty.up();
    }

    fn put_tab(&mut self, count: u16) {
        serial_println!("[TERM] put tab");
        let Cursor { row, col } = self._cursor;
        let s = self.rows.get_mut(row).unwrap();
        if col == s.len() {
            s.extend((0..count).map(|_| '\t'));
        } else {
            s.splice(col..=col, (0..count).map(|_| '\t'));
        }
        self.move_forward(count as usize);
        self.dirty.partial();
    }

    fn bell(&mut self) {}

    fn clear_screen(&mut self, _mode: vte::ansi::ClearMode) {
        self.newline();
        self.visible = Visible {
            from: self.rows.len(),
            count: 1,
        };
        self.dirty.full();
    }

    fn dynamic_color_sequence(&mut self, c1: String, c2: usize, c3: &str) {
        serial_println!(
            "[TERM HANDLER]: TODO: dynamic color sequence. Received: {}, {}, {}",
            c1,
            c2,
            c3
        );
    }
}

#[derive(Default)]
pub struct SimpleTimeout {
    timeout: Option<Duration>,
}

impl Timeout for SimpleTimeout {
    fn set_timeout(&mut self, duration: core::time::Duration) {
        self.timeout = Some(duration + Duration::from_millis(unsafe { syscalls::time() }.unwrap()))
    }

    fn clear_timeout(&mut self) {
        self.timeout.take();
    }

    fn pending_timeout(&self) -> bool {
        self.timeout.is_some()
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
    ByteStream(Vec<u8>),
    String(String),
}

impl Event {}

impl From<Vec<char>> for Event {
    fn from(value: Vec<char>) -> Self {
        Self::CharStream(value)
    }
}

impl From<&[u8]> for Event {
    fn from(value: &[u8]) -> Self {
        Self::ByteStream(value.to_vec())
    }
}

impl From<String> for Event {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}
