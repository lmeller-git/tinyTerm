// Tokenizer builds a TokenStream from input string, we then parse the TokenStream into Commands
//
// in the end we want to be able to:
// let command = Command::new(input: &str);
// --> this should parse the input into a chain of commands
// command.execute()
// --> this should execute the top level command and walk along the chain of "child" commands
// --> thus a command should have a ref to its bin, argc, argv, filedescriptoe redirections/pipes and "child" commands
// --> sth like
// Command {
//  bin: Bin(&str),
//  argc: Argc(&str),
//  env: Env(&str),
//  chained: Option<Box<Command>>,
//  redirections: Vec<Redirection>
// }
// This could be parsed from a TokenStream loike [Bin(&str), Argc(&str), Redirection(_), Redirection(_), Pipe, ....]

use core::ops::{Deref, DerefMut};

use alloc::vec::{self, Vec};
use libtinyos::syscalls::{FileDescriptor, OpenOptions, STDERR_FILENO, STDOUT_FILENO};

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
    Pipe,
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
            Self::Read => OpenOptions::READ,
            Self::Write => OpenOptions::WRITE,
            Self::WriteAppend => OpenOptions::WRITE | OpenOptions::APPEND,
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
            s if s.starts_with('|') => self.parse_pipe(), // pipe: |
            s if s.starts_with(['>', '<'])
                | s.chars().nth(2).is_some_and(|c| matches!(c, '>' | '<')) =>
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
        if let Some(next_char) = self.src.chars().nth(self.cursor)
            && next_char == '|'
        {
            self.inc('|');
            Ok(Token::Pipe)
        } else {
            Err(TokenParseError::SrcConsumed)
        }
    }

    fn parse_redir(&mut self) -> Result<Token<'a>, TokenParseError> {
        let dual = if let Some(c) = self.src[self.cursor..].chars().next()
            && c == '&'
        {
            self.checked_inc('&', |zelf| {
                Err(TokenParseError::MalformedInput(zelf.cursor))
            })?;
            true
        } else {
            false
        };

        let fd = if !dual
            && let Some(c) = self.src[self.cursor..].chars().next()
            && let Some(digit) = c.to_digit(10)
        {
            self.checked_inc(c, |zelf| Err(TokenParseError::MalformedInput(zelf.cursor)))?;
            Some(digit)
        } else {
            None
        };

        let mode = match &self.src[self.cursor..] {
            s if s.starts_with(">>") => {
                self.cursor += '>'.len_utf8() * 2;
                RedirectionMode::WriteAppend
            }
            s if s.starts_with("<") => {
                self.inc('<');
                RedirectionMode::Read
            }
            s if s.starts_with(">") => {
                self.inc('>');

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
                self.inc(c);
                if c.is_whitespace() || matches!(c, '|' | '>' | '<') {
                    break;
                }
            }
            Ok(Token::Literal(&self.src[start..self.cursor]))
        }
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

//
// we have currently: This is TODO
// redirection: one of
// > | >> | < | num> | num>> | &> | &>> | &num> | &num>>
// pipe: one of
// | | &| | num>| (this could also be interpreted as redirection)
// literal
// whitespace
