use alloc::{string::String, vec::Vec};
use libtinyos::{eprintln, println, serial_println};
use vte::ansi::Handler;

use crate::{
    drain_stdin,
    logic::jobs::{Command, wait},
};

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
        let prompt_len = 1;
        let Some(should_skip) = self
            .input_buf
            .iter()
            .skip(prompt_len) // prompt
            .position(|item| !item.is_whitespace())
            .map(|pos| pos + prompt_len)
        else {
            return Command::bin([].as_slice());
        };

        let mut splits = self.input_buf[should_skip..].splitn(2, |item| item.is_whitespace());

        let bin_name = splits.next().unwrap_or([].as_slice());
        if let Some(args) = splits.next() {
            Command::new(bin_name, args)
        } else {
            Command::bin(bin_name)
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
            Ok(pid) => {
                wait(pid);
                drain_stdin();
            }
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
