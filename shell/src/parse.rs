use core::ops::{Deref, DerefMut};

use alloc::vec::{self, Vec};
use conquer_once::spin::OnceCell;
use libtinyos::syscalls::{
    FileDescriptor, OpenOptions, STDERR_FILENO, STDIN_FILENO, STDOUT_FILENO,
};
use regex::Regex;

static PIPE_REGEX: OnceCell<Regex> = OnceCell::uninit();

pub struct TokenStream<'a> {
    inner: Vec<Token<'a>>,
}

impl<'a> TokenStream<'a> {
    pub fn new() -> Self {
        Self { inner: Vec::new() }
    }
}

impl<'a> Deref for TokenStream<'a> {
    type Target = Vec<Token<'a>>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'a> DerefMut for TokenStream<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<'a> IntoIterator for TokenStream<'a> {
    type Item = Token<'a>;
    type IntoIter = vec::IntoIter<Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter()
    }
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum Token<'a> {
    Literal(&'a str),
    WhiteSpace(&'a str),
    Pipe(Pipe),
    Redirection(Redirection),
    EOF,
}

impl<'a> Token<'a> {
    pub fn is_whitespace(&self) -> bool {
        if let Self::WhiteSpace(_) = self {
            true
        } else {
            false
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pipe {
    pub from: Vec<FileDescriptor>,
    pub to: FileDescriptor,
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Redirection {
    pub from: Vec<FileDescriptor>,
    pub mode: RedirectionMode,
}

#[derive(PartialEq, Eq, Clone, Debug, Copy)]
pub enum RedirectionMode {
    Empty,
    Read,
    Write,
    WriteAppend,
}

impl Into<OpenOptions> for RedirectionMode {
    fn into(self) -> OpenOptions {
        match self {
            Self::Read => OpenOptions::READ | OpenOptions::CREATE,
            Self::Write => OpenOptions::WRITE | OpenOptions::CREATE | OpenOptions::TRUNCATE,
            Self::WriteAppend => OpenOptions::WRITE | OpenOptions::APPEND | OpenOptions::CREATE,
            _ => OpenOptions::empty(),
        }
    }
}

pub struct Tokenizer_<'a> {
    src: &'a str,
    // byte counter
    cursor: usize,
    is_done: bool,
}

impl<'a> Tokenizer_<'a> {
    pub fn new(src: &'a str) -> Self {
        Self {
            src,
            cursor: 0,
            is_done: false,
        }
    }

    pub fn tokenize(&mut self) -> Result<TokenStream<'a>, TokenParseError> {
        let mut stream = TokenStream::new();

        while !self.is_done {
            let word = self.parse_token()?;
            stream.push(word);
        }

        Ok(stream)
    }

    pub fn reset(&mut self) {
        self.cursor = 0;
        self.is_done = false;
    }

    fn parse_token(&mut self) -> Result<Token<'a>, TokenParseError> {
        if self.cursor >= self.src.len() || self.is_done {
            self.is_done = true;
            return Ok(Token::EOF);
        }
        match &self.src[self.cursor..] {
            s if s.starts_with(['\'', '\"']) => self.parse_literal(),
            s if s.starts_with('|')
                | (s.chars()
                    .next()
                    .is_some_and(|c| c == '&' || c.is_ascii_digit())
                    && s.chars().nth(1).is_some_and(|c| c == '|'))
                | PIPE_REGEX
                    .get_or_init(|| regex::Regex::new(r"^&\d+(?:,\d+)*\|").unwrap())
                    .is_match(s) =>
            {
                self.parse_pipe()
            } // pipe: |, ...
            s if s.starts_with(['>', '<'])
                | (s.chars().next().is_some_and(|c| c.is_ascii_digit())
                    && s.chars().nth(1).is_some_and(|c| c == '>'))
                | s.starts_with("&>") =>
            {
                self.parse_redir() // redir: >*  or <* or n>* or n<*
            }
            s if s.is_empty() => {
                self.is_done = true;
                Ok(Token::EOF)
            }
            _ => self.parse_literal(),
        }
    }

    fn parse_pipe(&mut self) -> Result<Token<'a>, TokenParseError> {
        // | or num| or num1|num2 or &| or &num,*|
        let dual = if self.src[self.cursor..].starts_with("&|") {
            self.checked_inc('&', |zelf| {
                Err(TokenParseError::MalformedInput(zelf.cursor))
            })?;
            self.checked_inc('|', |zelf| {
                Err(TokenParseError::MalformedInput(zelf.cursor))
            })?;
            true
        } else {
            false
        };

        let in_fds = if !dual {
            let digit = self.parse_num().unwrap_or(STDOUT_FILENO);
            alloc::vec![digit]
        } else {
            let mut v = Vec::new();
            while let Ok(num) = self.parse_num() {
                v.push(num);
                if !self.src.bytes().nth(self.cursor).is_some_and(|c| c == b',') {
                    break;
                }
                self.inc(',');
            }
            if v.is_empty() {
                v.extend_from_slice(&[STDOUT_FILENO, STDERR_FILENO]);
            }
            if v.len() < 2 {
                return Err(TokenParseError::MalformedInput(self.cursor));
            }
            v
        };

        if !self.src.bytes().nth(self.cursor).is_some_and(|c| c == b'|') {
            return Err(TokenParseError::MalformedInput(self.cursor));
        }
        self.inc('|');

        let out_fd = if let Ok(num) = self.parse_num() {
            num
        } else {
            STDIN_FILENO
        };

        Ok(Token::Pipe(Pipe {
            from: in_fds,
            to: out_fd,
        }))
    }

    fn parse_redir(&mut self) -> Result<Token<'a>, TokenParseError> {
        let dual = if self.src[self.cursor..].starts_with("&>") {
            self.checked_inc('&', |zelf| {
                Err(TokenParseError::MalformedInput(zelf.cursor))
            })?;
            self.checked_inc('>', |zelf| {
                Err(TokenParseError::MalformedInput(zelf.cursor))
            })?;
            true
        } else {
            false
        };

        let mut fd = if !dual && let Ok(num) = self.parse_num() {
            Some(num)
        } else {
            None
        };

        let mode = match &self.src[self.cursor..] {
            s if s.starts_with(">>") => {
                self.cursor += '>'.len_utf8() * 2;
                if fd.is_none() {
                    fd.replace(STDOUT_FILENO);
                }
                RedirectionMode::WriteAppend
            }
            s if s.starts_with("<") => {
                self.inc('<');
                if fd.is_none() {
                    fd.replace(STDIN_FILENO);
                }
                RedirectionMode::Read
            }
            s if s.starts_with(">") => {
                self.inc('>');
                if fd.is_none() {
                    fd.replace(STDOUT_FILENO);
                }
                RedirectionMode::Write
            }
            _ => return Err(TokenParseError::MalformedInput(self.cursor)),
        };

        let mut srcs = Vec::new();
        if let Some(fd) = fd {
            srcs.push(fd);
        }

        if dual {
            srcs.extend_from_slice(&[STDOUT_FILENO, STDERR_FILENO]);
        }

        Ok(Token::Redirection(Redirection { from: srcs, mode }))
    }

    fn parse_literal(&mut self) -> Result<Token<'a>, TokenParseError> {
        // TODO quotes
        let start = self.cursor;
        if self.src[self.cursor..]
            .chars()
            .next()
            .is_some_and(|c| c.is_whitespace())
        {
            // TODO non ascii whitespace?
            self.cursor += self
                .src
                .bytes()
                .skip(self.cursor)
                .take_while(|item| item.is_ascii_whitespace())
                .count();
            Ok(Token::WhiteSpace(&self.src[start..self.cursor]))
        } else {
            for c in self.src[self.cursor..].chars() {
                if c.is_whitespace() || matches!(c, '|' | '>' | '<') {
                    break;
                }
                self.inc(c);
            }
            Ok(Token::Literal(&self.src[start..self.cursor]))
        }
    }

    fn parse_num<T: From<u32>>(&mut self) -> Result<T, TokenParseError> {
        let mut num = 0;
        let mut chars = self.src[self.cursor..].bytes();
        while let Some(c) = chars.next()
            && c.is_ascii_digit()
        {
            num *= 10;
            num += char::from_u32(c as u32)
                .ok_or(TokenParseError::MalformedInput(self.cursor))?
                .to_digit(10)
                .ok_or(TokenParseError::MalformedInput(self.cursor))?;
            self.cursor += 1;
        }

        Ok(num.into())
    }

    fn inc(&mut self, by: char) {
        self.cursor += by.len_utf8();
    }

    fn checked_inc(
        &mut self,
        by: char,
        err_callback: impl Fn(&mut Tokenizer_) -> Result<(), TokenParseError>,
    ) -> Result<(), TokenParseError> {
        if self.cursor >= self.src.len() - by.len_utf8() {
            return err_callback(self);
        } else {
            self.cursor += by.len_utf8();
            Ok(())
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenParseError {
    Generic(&'static str),
    SrcConsumed,
    MalformedInput(usize),
}
