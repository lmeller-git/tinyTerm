use core::str::FromStr;

use libtinyos::syscalls::{self, FileDescriptor, OpenOptions};
use ratatui::{prelude::Backend, style::Color};
use vte::ansi::Handler;

use crate::{graphics::DEFAULT_CONF, state::TermState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    config_file: FileDescriptor,
}

impl Config {
    pub fn new() -> Self {
        let path = b"/ram/term.conf";
        let config_file = unsafe {
            syscalls::open(
                path.as_ptr(),
                path.len(),
                OpenOptions::READ | OpenOptions::WRITE | OpenOptions::CREATE,
            )
        }
        .unwrap();

        unsafe { syscalls::write(config_file, DEFAULT_CONF.as_ptr(), DEFAULT_CONF.len()) }.unwrap();

        Self { config_file }
    }

    pub fn bg(&self) -> Color {
        self.parse_item("bg").unwrap_or(Color::Black)
    }

    pub fn border(&self) -> Color {
        self.parse_item("border").unwrap_or(Color::White)
    }

    pub fn text(&self) -> Color {
        self.parse_item("text").unwrap_or(Color::White)
    }

    pub fn title(&self) -> Color {
        self.parse_item("title").unwrap_or(Color::Green)
    }

    // TODO this hopusl only parse once for all calls per cycle
    fn parse_item(&self, name: &str) -> Option<Color> {
        let mut buf = [0; DEFAULT_CONF.len() + 10];
        if let Ok(n) = unsafe { syscalls::read(self.config_file, buf.as_mut_ptr(), buf.len(), 0) }
            && n > 0
            && let Ok(values) = str::from_utf8(&buf[..n as usize])
        {
            values
                .split('\t')
                .filter_map(|config_line| {
                    if config_line.starts_with(name) {
                        config_line
                            .split(' ')
                            .last()
                            .map(|color_str| Color::from_str(color_str).ok())
                            .flatten()
                    } else {
                        None
                    }
                })
                .next()
        } else {
            None
        }
    }
}

struct VTEPerformerHandler<B: Backend> {
    terminal: TermState<B>,
}

impl<B: Backend> VTEPerformerHandler<B> {
    pub fn new(terminal: TermState<B>) -> Self {
        Self { terminal }
    }
}

impl<B: Backend> Handler for VTEPerformerHandler<B> {
    fn set_title(&mut self, _: Option<alloc::string::String>) {}

    fn set_cursor_style(&mut self, _: Option<vte::ansi::CursorStyle>) {}

    fn set_cursor_shape(&mut self, _shape: vte::ansi::CursorShape) {}

    fn input(&mut self, _c: char) {}

    fn move_up(&mut self, _: usize) {}

    fn move_down(&mut self, _: usize) {}

    fn move_forward(&mut self, _col: usize) {}

    fn move_backward(&mut self, _col: usize) {}

    fn bell(&mut self) {}

    fn carriage_return(&mut self) {}

    fn linefeed(&mut self) {}

    fn newline(&mut self) {}

    fn scroll_up(&mut self, _: usize) {}

    fn scroll_down(&mut self, _: usize) {}

    fn save_cursor_position(&mut self) {}

    fn reset_state(&mut self) {}

    fn set_scrolling_region(&mut self, _top: usize, _bottom: Option<usize>) {}

    fn clear_screen(&mut self, _mode: vte::ansi::ClearMode) {}

    fn clear_line(&mut self, _mode: vte::ansi::LineClearMode) {}

    fn set_active_charset(&mut self, _: vte::ansi::CharsetIndex) {}

    fn restore_cursor_position(&mut self) {}

    fn configure_charset(&mut self, _: vte::ansi::CharsetIndex, _: vte::ansi::StandardCharset) {}

    fn set_color(&mut self, _: usize, _: vte::ansi::Rgb) {}

    fn dynamic_color_sequence(&mut self, _: alloc::string::String, _: usize, _: &str) {}

    fn reset_color(&mut self, _: usize) {}

    fn clipboard_store(&mut self, _: u8, _: &[u8]) {}

    fn clipboard_load(&mut self, _: u8, _: &str) {}

    fn push_title(&mut self) {}

    fn pop_title(&mut self) {}
}
