use alloc::{string::String, vec::Vec};
use libtinyos::{eprintln, println, serial_println};
use vte::ansi::Handler;

use crate::logic::jobs::{Command, wait};

pub struct ShellState {
    input_buf: Vec<char>,
    cursor: usize,
}

impl ShellState {
    pub fn new() -> Self {
        let mut shell = Self {
            input_buf: Vec::new(),
            cursor: 0,
        };
        shell.inject_prompt();
        shell
    }

    fn extract_command(&self) -> Command {
        if let Some(arg_split) = self.input_buf.iter().position(|item| item.is_whitespace()) {
            Command::new(&self.input_buf[..arg_split], &self.input_buf[arg_split..])
        } else {
            Command::bin(&self.input_buf)
        }
    }

    fn clear(&mut self) {
        self.input_buf.clear();
        self.cursor = 0
    }

    fn inject_prompt(&mut self) {
        println!("tinyos:/\n");
        self.input('>');
        self.input(' ');
    }
}

impl Handler for ShellState {
    fn input(&mut self, c: char) {
        serial_println!("[input] {c}");
        if self.cursor == self.input_buf.len() {
            self.input_buf.push(c);
        } else {
            self.input_buf.insert(self.cursor, c);
        }
        self.move_forward(1);
        println!("{}", self.input_buf.iter().collect::<String>());
    }

    fn move_up(&mut self, _: usize) {}

    fn move_down(&mut self, _: usize) {}

    fn move_forward(&mut self, col: usize) {
        self.cursor = (self.cursor.saturating_add(col)).min(self.input_buf.len())
    }

    fn move_backward(&mut self, col: usize) {
        self.cursor = self.cursor.saturating_sub(col)
    }

    fn linefeed(&mut self) {
        let command = self.extract_command();
        match command.execute() {
            Ok(pid) => wait(pid),
            Err(e) => {
                eprintln!("could not spawn process {}:\n{:?}", command, e);
            }
        }
        self.newline();
    }

    fn carriage_return(&mut self) {
        self.clear();
    }

    fn newline(&mut self) {
        self.input('\n');
        self.carriage_return();
        self.inject_prompt();
    }
}
