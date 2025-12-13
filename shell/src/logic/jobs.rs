use core::{
    fmt::Display,
    iter::Peekable,
    ptr::null,
    sync::atomic::{AtomicBool, AtomicU64},
};

use alloc::{boxed::Box, string::String, vec::Vec};
use libtinyos::{
    serial_println,
    syscalls::{
        self, FileDescriptor, OpenOptions, STDOUT_FILENO, SysCallRes, TaskWaitOptions, WaitOptions,
    },
};

use crate::{
    logic::trim_string_in_place,
    parse::{self, RedirectionMode, Token, TokenParseError, TokenStream, Tokenizer_},
};

pub static CURRENT_FG: AtomicU64 = AtomicU64::new(0);
pub static WE_ARE_FG: AtomicBool = AtomicBool::new(false);

#[derive(Debug)]
pub struct Command_<'a> {
    bin: &'a str,
    args: Vec<&'a str>,
    redirections: Vec<Redirection_<'a>>,
    chained: Option<Pipe_<'a>>,
}

impl<'a> Command_<'a> {
    pub fn build(tokenstream: &mut Peekable<impl Iterator<Item = Token<'a>>>) -> Option<Self> {
        // prompt for a single job looks like
        // <bin> <args> <redirs> <pipe>
        // we ignore all whitespace
        let Token::Literal(bin) = consume_next_with_whitespace(tokenstream)? else {
            return None;
        };

        let mut args = Vec::new();
        collect_all(tokenstream, &mut |token| {
            match token {
                Token::Literal(lit) => args.push(*lit),
                Token::WhiteSpace(_) => {}
                _ => return false,
            }
            true
        });

        let mut redirections = Vec::new();
        let mut current_redir: Option<parse::Redirection> = None;

        while let Some(next) = tokenstream.peek() {
            match next {
                Token::Literal(lit) => {
                    if let Some(current) = current_redir.take() {
                        redirections.push(Redirection_ {
                            fds: current.from,
                            mode: current.mode,
                            file: lit,
                        });
                    } else {
                        break;
                    }
                }
                Token::Redirection(redir) => {
                    if let Some(_) = current_redir.replace(redir.clone()) {
                        return None;
                    }
                }
                Token::WhiteSpace(_) => {}
                _ => break,
            }
            tokenstream.next();
        }

        let mut zelf = Command_ {
            bin,
            args,
            redirections,
            chained: None,
        };

        if let Some(Token::Pipe) = consume_next_with_whitespace(tokenstream)
            && let Some(next) = Command_::build(tokenstream)
        {
            let pipe = Pipe_ { to: next.into() };
            zelf.chained = Some(pipe);
        }

        Some(zelf)
    }

    pub fn execute_all(&self) -> SysCallRes<ExecutionContext> {
        let mut ctx = ExecutionContext {
            running: Vec::new(),
        };

        // for each process:
        // 1: open files for redir and pipe and dup
        // 2: execute
        // 3: cleanup fds -> close + restore old
        // 4: if fail to execute: kill all previous + exit
        // 5: setup pipe for next process
        // 6: next ->

        struct ExecCleanup {
            ctx: ExecutionContext,
            should_restore: Vec<(FileDescriptor, FileDescriptor)>,
            should_close: Vec<FileDescriptor>,
        }

        impl Drop for ExecCleanup {
            fn drop(&mut self) {}
        }

        let mut exec_cleanup = ExecCleanup {
            ctx,
            should_restore: Vec::new(),
            should_close: Vec::new(),
        };

        let mut current = self;

        loop {
            // let mut redir_cleanup = Vec::with_capacity(current.redirections.len());
            for redir in &current.redirections {
                let new_open = unsafe {
                    syscalls::open(redir.file.as_ptr(), redir.file.len(), redir.mode.into())
                };
            }

            break;
        }

        Ok(ctx)
    }

    fn execute_one(&self) -> SysCallRes<u64> {
        let res = unsafe { 0 };

        todo!()
    }
}

pub struct ExecutionContext {
    running: Vec<u64>,
}

impl ExecutionContext {
    pub fn exit_all(&self) {}

    pub fn wait_all(&self) {}
}

fn collect_all<'a>(
    stream: &mut Peekable<impl Iterator<Item = Token<'a>>>,
    eval: &mut impl FnMut(&Token<'a>) -> bool,
) {
    while let Some(t) = stream.peek()
        && eval(t)
    {
        stream.next();
    }
}

fn consume_next_with_whitespace<'a>(
    stream: &mut impl Iterator<Item = Token<'a>>,
) -> Option<Token<'a>> {
    let next = stream.next()?;
    if next.is_whitespace() {
        stream.next()
    } else {
        Some(next)
    }
}

#[derive(Debug)]
struct Pipe_<'a> {
    to: Box<Command_<'a>>,
}

#[derive(Debug)]
struct Redirection_<'a> {
    fds: Vec<FileDescriptor>,
    mode: RedirectionMode,
    file: &'a str,
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Command {
    bin_name: String,
    argc: String,
    redirections: Vec<Redirection>,
    piped: Option<Pipe>,
}

impl Command {
    pub fn new_(line: &str) -> Option<Self> {
        let mut tokenstream = Tokenizer_::new(line).tokenize().ok()?.into_iter();
        let first = tokenstream.next()?;
        let bin = if let Token::Literal(l) = first {
            l
        } else {
            let Some(Token::Literal(l)) = tokenstream.next() else {
                return None;
            };
            l
        };
        todo!()
    }

    pub fn new(bin: &[char], args: &[char]) -> Self {
        let mut bin = bin.iter().collect();
        let mut args = args.iter().collect();
        trim_string_in_place(&mut bin);
        trim_string_in_place(&mut args);

        if let Some(idx) = args.find('>') {
            // we have a redirection
            // currently only > x is supported
            let (pre_redirection, post_redirecttion) = args.split_at(idx);
            let post_redirecttion = post_redirecttion.strip_prefix(">").unwrap().trim();
            if let Ok(fd) = unsafe {
                syscalls::open(
                    post_redirecttion.as_ptr(),
                    post_redirecttion.len(),
                    OpenOptions::WRITE,
                )
            } {
                let redirection = Redirection {
                    to: post_redirecttion.into(),
                    from: fd,
                };
                return Self {
                    bin_name: bin,
                    argc: pre_redirection.into(),
                    redirections: alloc::vec![redirection],
                    piped: None,
                };
            }
        }

        Self {
            bin_name: bin,
            argc: args,
            redirections: Vec::new(),
            piped: None,
        }
    }

    pub fn bin(bin: &[char]) -> Self {
        Self::new(bin, [].as_slice())
    }

    pub fn execute(&self) -> SysCallRes<u64> {
        let mut cleanup = false;
        let next_stdout = unsafe { syscalls::dup(STDOUT_FILENO, None) }.unwrap();
        if let Some(fd) = self.redirections.first() {
            cleanup = true;
            unsafe { syscalls::dup(fd.from, Some(STDOUT_FILENO)) }.unwrap();
        }
        let res = unsafe {
            syscalls::execve(
                self.bin_name.as_ptr(),
                self.bin_name.len(),
                self.argc.as_ptr(),
                self.argc.len(),
                null(),
                0,
            )
        };
        if cleanup {
            unsafe { syscalls::dup(next_stdout, Some(STDOUT_FILENO)) }.unwrap();
        }
        res
    }
}

impl Display for Command {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Command")
            .field("Name", &self.bin_name)
            .field("Args", &self.argc)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Redirection {
    to: String,
    from: FileDescriptor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Pipe {
    to: Box<Command>,
}

pub fn wait(pid: u64) {
    CURRENT_FG.store(pid, core::sync::atomic::Ordering::Release);
    WE_ARE_FG.store(false, core::sync::atomic::Ordering::Release);
    _ = unsafe { syscalls::wait_pid(pid, -1, WaitOptions::empty(), TaskWaitOptions::W_EXIT) };
}

pub fn signal_handler(signal_pipe: FileDescriptor) {
    let mut buffer = [0_u8; 10];
    loop {
        let n_read = unsafe {
            syscalls::read(
                signal_pipe,
                buffer.as_mut_ptr(),
                buffer.len(),
                -1_isize as usize,
            )
        }
        .unwrap();
        let signals = &buffer[..n_read as usize];
        for signal in signals {
            match signal {
                0 => {
                    let current_fg = CURRENT_FG.load(core::sync::atomic::Ordering::Acquire);
                    if current_fg != 0 && !WE_ARE_FG.load(core::sync::atomic::Ordering::Acquire) {
                        unsafe { syscalls::kill(current_fg, 0) }.unwrap();
                        WE_ARE_FG.store(true, core::sync::atomic::Ordering::Release);
                    }
                    serial_println!("[signal abort]")
                }
                1 => serial_println!("[signal bg]"),
                _ => serial_println!("[signal] unknown [{}]", signal),
            };
        }
    }
}
