use core::str::FromStr;

use libtinyos::syscalls::{self, FileDescriptor, OpenOptions};
use ratatui::style::Color;

use crate::graphics::DEFAULT_CONF;

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
        unsafe { syscalls::seek(self.config_file, 0) }.ok()?;
        let mut buf = [0; DEFAULT_CONF.len() * 2];
        if let Ok(n) = unsafe { syscalls::read(self.config_file, buf.as_mut_ptr(), buf.len(), 0) }
            && n > 0
            && let Ok(values) = str::from_utf8(&buf[..n as usize])
        {
            values
                .split(' ')
                .filter_map(|config_line| {
                    if config_line.starts_with(name) {
                        config_line
                            .split(':')
                            .next_back()
                            .and_then(|color_str| Color::from_str(color_str).ok())
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

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}
