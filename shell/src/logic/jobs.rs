use core::{
    cell::UnsafeCell,
    fmt::Display,
    iter::Peekable,
    ptr::null,
    sync::atomic::{AtomicBool, AtomicU64},
};

use alloc::{boxed::Box, string::String, sync::Arc, vec::Vec};
use libtinyos::{
    eprintln, serial_println,
    syscalls::{
        self, FileDescriptor, OpenOptions, STDIN_FILENO, STDOUT_FILENO, SysCallRes,
        TaskWaitOptions, WaitOptions,
    },
};
use spin::Mutex;

use crate::{
    logic::trim_string_in_place,
    parse::{self, RedirectionMode, Token, TokenParseError, TokenStream, Tokenizer_},
};

pub static CURRENT_FG: AtomicU64 = AtomicU64::new(0);
pub static WE_ARE_FG: AtomicBool = AtomicBool::new(false);
pub static CURRENT_CTX: Mutex<Option<ExecutionContext>> = Mutex::new(None);

#[derive(Debug)]
pub struct Command_<'a> {
    bin: &'a str,
    args: Vec<&'a str>,
    redirections: Vec<Redirection_<'a>>,
    chained: Option<Pipe_<'a>>,
}

impl Display for Command_<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Command")
            .field("Bin", &self.bin)
            .field("Args", &self.args)
            .field("Redirs", &self.redirections)
            .field("Piped", &self.chained)
            .finish()
    }
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
            let pipe = Pipe_ {
                to: next.into(),
                connections: alloc::vec![(STDOUT_FILENO, STDIN_FILENO)],
            };
            zelf.chained = Some(pipe);
        }

        Some(zelf)
    }

    pub fn execute_all(&self) -> SysCallRes<ExecutionContext> {
        let ctx = ExecutionContext {
            running: Vec::new(),
        };

        // for each process:
        // 1: open files for redir and pipe and dup
        // 2: execute
        // 3: cleanup fds -> close + restore old
        // 4: if fail to execute: kill all previous + exit
        // 5: setup pipe for next process
        // 6: next ->

        struct ExecutionContextGuard {
            ctx: ExecutionContext,
        }

        impl Drop for ExecutionContextGuard {
            fn drop(&mut self) {
                // on good path this ctx will be ctx::Default. We must ensure to mem::take BEFORE drop
                for tsk in &self.ctx.running {
                    _ = unsafe {syscalls::kill(*tsk, -1)}.inspect_err(|e| serial_println!("\x1b[31m[SHELL ERR]\x1b[0m could not kill task {} during job spawn cleanup due to: {:?}", tsk,e));
                }
            }
        }

        let mut cleanup = FdCleanup::new();
        let mut exec_guard = ExecutionContextGuard { ctx };

        let mut current = self;

        loop {
            exec_guard.ctx.running.push(current.execute_one()?);
            // we could call cleanup.cleanup() now to clean the fd table, but this can also be delayed, as we cleanup in reverse

            if let Some(pipe) = &current.chained {
                for (from, to) in &pipe.connections {
                    cleanup.old_fds.push(FDSave {
                        saved_in: unsafe { syscalls::dup(*to, None) }?,
                        from: *to,
                    });
                    unsafe { syscalls::dup(*from, Some(*to)) }?;
                }
                current = pipe.to.as_ref();
            } else {
                break;
            }
        }

        // drop after mem::take, as we do not want to kill good jobs
        let ctx = core::mem::take(&mut exec_guard.ctx);
        Ok(ctx)
    }

    fn execute_one(&self) -> SysCallRes<u64> {
        // setup redirections
        // spawn task
        // cleanup redirections

        let _redir_guards = self
            .redirections
            .iter()
            .map(|redir| redir.install())
            .rev()
            .collect::<SysCallRes<Vec<_>>>()?;

        let args = self.args.join(" ");

        serial_println!("spawning {}, with {}", self.bin, args);

        unsafe {
            syscalls::execve(
                self.bin.as_ptr(),
                self.bin.len(),
                args.as_ptr(),
                args.len(),
                null(),
                0,
            )
        }
    }
}

#[must_use]
#[derive(Default)]
struct FdCleanup {
    old_fds: Vec<FDSave>,
}

impl FdCleanup {
    fn cleanup(&mut self) {
        for save in self.old_fds.drain(..).rev() {
            save.cleanup();
        }
    }

    fn new() -> Self {
        Self {
            old_fds: Vec::new(),
        }
    }
}

impl Drop for FdCleanup {
    fn drop(&mut self) {
        self.cleanup();
    }
}

#[must_use]
struct CloseGuard {
    fd: FileDescriptor,
}

impl Drop for CloseGuard {
    fn drop(&mut self) {
        _ = unsafe { syscalls::close(self.fd) }.inspect_err(|e| {
                        serial_println!(
                            "\x1b[31m[SHELL ERR]\x1b[0m could not close save fd {} during job spawn cleanup due to: {:?}",
                            self.fd,  e
                        );
                    });
    }
}

#[derive(Default, Clone)]
pub struct ExecutionContext {
    running: Vec<u64>,
}

impl ExecutionContext {
    pub fn kill_all(&self) -> SysCallRes<()> {
        for pid in &self.running {
            unsafe { syscalls::kill(*pid, -1) }?;
        }
        Ok(())
    }

    pub fn wait_all(&self) -> SysCallRes<()> {
        for pid in &self.running {
            unsafe { syscalls::wait_pid(*pid, -1, WaitOptions::empty(), TaskWaitOptions::W_EXIT) }?;
        }
        Ok(())
    }
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
    connections: Vec<(FileDescriptor, FileDescriptor)>,
}

#[derive(Debug)]
struct Redirection_<'a> {
    fds: Vec<FileDescriptor>,
    mode: RedirectionMode,
    file: &'a str,
}

impl<'a> Redirection_<'a> {
    fn install(&self) -> SysCallRes<(CloseGuard, FdCleanup)> {
        match self.mode {
            RedirectionMode::Empty => Err(syscalls::SysErrCode::NoErr),
            _ => {
                let mut cleanup = FdCleanup::new();
                let f = unsafe {
                    syscalls::open(self.file.as_ptr(), self.file.len(), self.mode.into())
                }?;
                let guard = CloseGuard { fd: f };

                for fd in &self.fds {
                    cleanup.old_fds.push(FDSave {
                        saved_in: unsafe { syscalls::dup(*fd, None) }?,
                        from: *fd,
                    });

                    unsafe { syscalls::dup(guard.fd, Some(*fd)) }?;
                }

                Ok((guard, cleanup))
            }
        }
    }
}

struct FDSave {
    saved_in: FileDescriptor,
    from: FileDescriptor,
}

impl FDSave {
    fn cleanup(&self) {
        _ = unsafe { syscalls::dup(self.saved_in, Some(self.from)) }.inspect_err(|e| {
                        serial_println!(
                            "\x1b[31m[SHELL ERR]\x1b[0m could not restore fd {} to {} during job spawn cleanup due to: {:?}",
                            self.saved_in, self.from, e
                        );
                    });
        _ = unsafe { syscalls::close(self.saved_in) }.inspect_err(|e| {
                        serial_println!(
                            "\x1b[31m[SHELL ERR]\x1b[0m could not close save fd {} during job spawn cleanup due to: {:?}",
                            self.saved_in,  e
                        );
                    });
    }
}

// #[derive(PartialEq, Eq, Debug, Clone)]
// pub struct Command {
//     bin_name: String,
//     argc: String,
//     redirections: Vec<Redirection>,
//     piped: Option<Pipe>,
// }

// impl Command {
//     pub fn new_(line: &str) -> Option<Self> {
//         let mut tokenstream = Tokenizer_::new(line).tokenize().ok()?.into_iter();
//         let first = tokenstream.next()?;
//         let bin = if let Token::Literal(l) = first {
//             l
//         } else {
//             let Some(Token::Literal(l)) = tokenstream.next() else {
//                 return None;
//             };
//             l
//         };
//         todo!()
//     }

//     pub fn new(bin: &[char], args: &[char]) -> Self {
//         let mut bin = bin.iter().collect();
//         let mut args = args.iter().collect();
//         trim_string_in_place(&mut bin);
//         trim_string_in_place(&mut args);

//         if let Some(idx) = args.find('>') {
//             // we have a redirection
//             // currently only > x is supported
//             let (pre_redirection, post_redirecttion) = args.split_at(idx);
//             let post_redirecttion = post_redirecttion.strip_prefix(">").unwrap().trim();
//             if let Ok(fd) = unsafe {
//                 syscalls::open(
//                     post_redirecttion.as_ptr(),
//                     post_redirecttion.len(),
//                     OpenOptions::WRITE,
//                 )
//             } {
//                 let redirection = Redirection {
//                     to: post_redirecttion.into(),
//                     from: fd,
//                 };
//                 return Self {
//                     bin_name: bin,
//                     argc: pre_redirection.into(),
//                     redirections: alloc::vec![redirection],
//                     piped: None,
//                 };
//             }
//         }

//         Self {
//             bin_name: bin,
//             argc: args,
//             redirections: Vec::new(),
//             piped: None,
//         }
//     }

//     pub fn bin(bin: &[char]) -> Self {
//         Self::new(bin, [].as_slice())
//     }

//     pub fn execute(&self) -> SysCallRes<u64> {
//         let mut cleanup = false;
//         let next_stdout = unsafe { syscalls::dup(STDOUT_FILENO, None) }.unwrap();
//         if let Some(fd) = self.redirections.first() {
//             cleanup = true;
//             unsafe { syscalls::dup(fd.from, Some(STDOUT_FILENO)) }.unwrap();
//         }
//         let res = unsafe {
//             syscalls::execve(
//                 self.bin_name.as_ptr(),
//                 self.bin_name.len(),
//                 self.argc.as_ptr(),
//                 self.argc.len(),
//                 null(),
//                 0,
//             )
//         };
//         if cleanup {
//             unsafe { syscalls::dup(next_stdout, Some(STDOUT_FILENO)) }.unwrap();
//         }
//         res
//     }
// }

// impl Display for Command {
//     fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
//         f.debug_struct("Command")
//             .field("Name", &self.bin_name)
//             .field("Args", &self.argc)
//             .finish()
//     }
// }

// #[derive(Debug, Clone, PartialEq, Eq)]
// struct Redirection {
//     to: String,
//     from: FileDescriptor,
// }

// #[derive(Debug, Clone, PartialEq, Eq)]
// struct Pipe {
//     to: Box<Command>,
// }

pub fn wait_(ctx: ExecutionContext) -> SysCallRes<()> {
    if let Some(stale) = CURRENT_CTX.lock().replace(ctx.clone()) {
        serial_println!("there was a sstale ctx. killing it...");
        _ = stale.kill_all();
    }
    WE_ARE_FG.store(false, core::sync::atomic::Ordering::Release);
    ctx.wait_all()
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
                    if !WE_ARE_FG.load(core::sync::atomic::Ordering::Acquire)
                        && let Some(ctx) = CURRENT_CTX.lock().take()
                    {
                        _ = ctx.kill_all();
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
