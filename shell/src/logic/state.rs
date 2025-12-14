use alloc::{string::String, vec, vec::Vec};
use libtinyos::{
    eprintln, print, println, serial_println,
    syscalls::{self, STDOUT_FILENO},
};
use vte::ansi::Handler;

use crate::{
    drain_stdin,
    logic::jobs::{Command_, wait, wait_},
    parse::Tokenizer_,
};

const PROMPT: &str = "> ";
const ARROW_LEFT: [u8; 3] = [0x1B, b'[', b'D'];
const ARROW_RIGHT: [u8; 3] = [0x1B, b'[', b'C'];
const ARROW_UP: [u8; 3] = [0x1B, b'[', b'A'];
const ARROW_DOWN: [u8; 3] = [0x1B, b'[', b'B'];

fn stdout_bytes(bytes: &[u8]) {
    _ = unsafe { syscalls::write(STDOUT_FILENO, bytes.as_ptr(), bytes.len()) };
}

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

    // fn extract_command(&self) -> Option<Command_> {
    //     let prompt_len = PROMPT.chars().count();
    //     let Some(should_skip) = self
    //         .input_buf
    //         .iter()
    //         .skip(prompt_len) // prompt
    //         .position(|item| !item.is_whitespace())
    //         .map(|pos| pos + prompt_len)
    //     else {
    //         return None;
    //     };

    //     let input_buf = self.input_buf[should_skip..].iter().collect::<String>();

    //     let stream = Tokenizer_::new(&input_buf).tokenize().ok()?;

    //     Command_::build(&mut stream.into_iter().peekable())
    // }

    fn clear(&mut self) {
        self.input_buf.clear();
        self.cursor = 0
    }

    fn inject_prompt(&mut self) {
        println!("tinyos:/");
        print!("\r{}", PROMPT);
        self.input_buf.extend(PROMPT.chars());
        self.inc_cursor(PROMPT.chars().count());
    }

    fn is_empty(&self) -> bool {
        self.input_buf.len() == PROMPT.chars().count()
    }

    fn inc_cursor(&mut self, by: usize) {
        self.cursor = (self.cursor.saturating_add(by)).min(self.input_buf.len());
    }

    fn _dec_cursor(&mut self, by: usize) {
        self.cursor = self.cursor.saturating_sub(by).max(PROMPT.chars().count());
    }
}

impl Handler for ShellState {
    fn input(&mut self, c: char) {
        if self.cursor == self.input_buf.len() {
            self.input_buf.push(c);
            print!("{c}");
        } else {
            self.input_buf.insert(self.cursor, c);
            print!("\r{}", self.input_buf.iter().collect::<String>());
        }
        self.inc_cursor(1);
    }

    fn move_up(&mut self, by: usize) {
        let bytes = vec![ARROW_UP; by];
        stdout_bytes(bytes.as_flattened());
    }

    fn move_down(&mut self, by: usize) {
        let bytes = vec![ARROW_DOWN; by];
        stdout_bytes(bytes.as_flattened());
    }

    fn move_forward(&mut self, col: usize) {
        self.cursor = (self.cursor.saturating_add(col)).min(self.input_buf.len());
        let bytes = vec![ARROW_RIGHT; col];
        stdout_bytes(bytes.as_flattened());
    }

    fn move_backward(&mut self, col: usize) {
        self.cursor = self.cursor.saturating_sub(col);
        let bytes = vec![ARROW_LEFT; col];
        stdout_bytes(bytes.as_flattened());
    }

    fn linefeed(&mut self) {
        print!("\n");
        let line = self
            .input_buf
            .iter()
            .skip(PROMPT.chars().count())
            .collect::<String>();
        if let Ok(stream) = Tokenizer_::new(&line).tokenize()
            && let Some(cmd) = Command_::build(&mut stream.into_iter().peekable())
        {
            match cmd.execute_all() {
                Ok(pid) => {
                    _ = wait_(pid);
                    drain_stdin();
                }
                Err(e) => {
                    eprintln!("could not spawn process {}:\n{:?}", cmd, e);
                }
            }
        }
        // let command = self.extract_command();
        // match command.execute() {
        //     Ok(pid) => {
        //         wait(pid);
        //         drain_stdin();
        //     }
        //     Err(e) => {
        //         eprintln!("could not spawn process {}:\n{:?}", command, e);
        //     }
        // }
        self.newline();
        self.inject_prompt();
    }

    fn carriage_return(&mut self) {
        self.clear();
        print!("\r");
    }

    fn newline(&mut self) {
        print!("\n");
        self.carriage_return();
    }

    fn put_tab(&mut self, _count: u16) {
        for _ in 0.._count {
            self.input('\t');
        }
    }

    fn backspace(&mut self) {
        if !self.is_empty() && self.cursor > PROMPT.chars().count() {
            self.input_buf.remove(self.cursor - 1);
            self.move_backward(1);
            print!("\r{}", self.input_buf.iter().collect::<String>());
        }
    }
}
