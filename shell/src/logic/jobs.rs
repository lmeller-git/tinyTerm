use core::{fmt::Display, iter::Peekable, marker::PhantomData};

use alloc::{
    boxed::Box,
    string::{String, ToString},
    vec::Vec,
};
use hashbrown::HashMap;
use libtinyos::{
    eprintln, serial_println,
    syscalls::{
        self, FDAction, FileDescriptor, OpenOptions, SysCallRes, TaskWaitOptions, WaitOptions,
    },
};

use crate::{
    builtins,
    env::{EnvVarStack, get_env},
    parse::{self, RedirectionMode, Token},
};

#[derive(Debug)]
pub struct Command_<'a> {
    bin: &'a str,
    args: Vec<&'a str>,
    redirections: Vec<Redirection<'a>>,
    chained: Option<Pipe<'a>>,
    active_env_var_stack: EnvVarStack,
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
        // <env vars> <bin> <args> <redirs> <pipe>
        // we ignore all whitespace
        let mut env = EnvVarStack::new();
        collect_all(tokenstream, &mut |token| {
            match token {
                // TODO
                // there might be vars containing an eq in their value, ie VAR="x=y". Should check if we are in some quoted region and split only non quoted regions i guess
                Token::Literal(lit)
                    if let Some(mut split) = Some(lit.split('='))
                        && let (Some(key), Some(val), None) =
                            (split.next(), split.next(), split.next()) =>
                {
                    _ = env.add(key.into(), val.into());
                }
                Token::WhiteSpace(_) => {}
                _ => return false,
            }
            true
        });

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
                        redirections.push(Redirection {
                            fds: current.from,
                            mode: current.mode,
                            file: lit,
                        });
                    } else {
                        break;
                    }
                }
                Token::Redirection(redir) => {
                    if current_redir.replace(redir.clone()).is_some() {
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
            active_env_var_stack: env,
        };

        let mut connections = HashMap::new();

        collect_all(tokenstream, &mut |token| {
            match token {
                Token::Pipe(p) => {
                    for from in &p.from {
                        connections
                            .entry(p.to)
                            .and_modify(|entry: &mut Vec<FileDescriptor>| entry.push(*from))
                            .or_insert(alloc::vec![*from]);
                    }
                }
                Token::WhiteSpace(_) => {}
                _ => return false,
            }
            true
        });

        if !connections.is_empty()
            && let Some(next) = Command_::build(tokenstream)
        {
            let pipe = Pipe {
                to: next.into(),
                connections,
            };
            zelf.chained = Some(pipe);
        }

        Some(zelf)
    }

    pub fn execute_all(&self) -> SysCallRes<ExecutionContext> {
        let mut ctx = ExecutionContext::default();
        let mut current = self;
        let mut current_builder = FDActionBuilder::new()
            .add_clear()
            .add_inherit(0, 0)
            .add_inherit(1, 1)
            .add_inherit(2, 2);
        let mut next_builder = FDActionBuilder::new()
            .add_clear()
            .add_inherit(0, 0)
            .add_inherit(1, 1)
            .add_inherit(2, 2);

        let mut should_close = Vec::new();

        let env = EnvVarStack::joined_as_env(&get_env().vars(), &self.active_env_var_stack);

        // for each pipe:
        // open pipe in self
        // close reader in current task (and dup writer to correct fd)
        // close writer in next task (and dup reader to correct fd)
        // close both in self after spawning next
        while let Some(pipe) = &current.chained {
            serial_println!("pipe: {:?}", pipe);
            for (to, from) in &pipe.connections {
                let mut pipe_fds = [0_u32, 0_u32];
                unsafe { syscalls::pipe(&mut pipe_fds as *mut [u32; 2], -1) }?;

                serial_println!("pipe fds: {:?}", pipe_fds);

                should_close.extend(pipe_fds.iter().map(|fd| OpenFd(*fd)));
                next_builder = next_builder.add_inherit(pipe_fds[0], *to);

                for from in from {
                    current_builder = current_builder.add_inherit(pipe_fds[1], *from);
                }
            }

            if let Some(res) = current.execute_one(
                core::mem::take(&mut current_builder),
                &mut should_close,
                &env,
            ) {
                ctx.running.push(res?);
            }

            (current_builder, next_builder, current) = (
                next_builder,
                FDActionBuilder::default()
                    .add_clear()
                    .add_inherit(0, 0)
                    .add_inherit(1, 1)
                    .add_inherit(2, 2),
                pipe.to.as_ref(),
            );
        }

        if let Some(res) = current.execute_one(
            core::mem::take(&mut current_builder),
            &mut should_close,
            &env,
        ) {
            ctx.running.push(res?);
        }

        serial_println!("closing: {:?}", should_close);
        drop(should_close);
        Ok(ctx)
    }

    pub fn execute_one(
        &self,
        mut action_builder: FDActionBuilder<'a>,
        temp_fds: &mut Vec<OpenFd>,
        env: &str,
    ) -> Option<SysCallRes<u64>> {
        serial_println!("redirs: {:?}", self.redirections);
        for redir in &self.redirections {
            let (builder, temp_fd) = match redir.add_to(action_builder) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            action_builder = builder;
            temp_fds.push(temp_fd);
        }
        serial_println!("builder: {:?}", action_builder);

        let args = self.args.join(" ");
        let args = args.as_bytes();

        if builtins::dispatch(
            self.bin,
            str::from_utf8(args).ok().unwrap_or(""),
            env,
            &action_builder,
        )
        .is_ok()
        {
            return None;
        }

        let vars = get_env().vars();

        let cwd = vars.get("CWD").unwrap_or("/");

        let bin = if let Some(bin) = resolve_relative(cwd, self.bin) {
            bin
        } else {
            self.bin.to_string()
        };

        serial_println!("bin is {}", bin);

        Some(unsafe {
            syscalls::spawn_process(
                bin.as_ptr(),
                bin.len(),
                args.len(),
                args.as_ptr(),
                env.len(),
                env.as_ptr(),
                action_builder.ptr().as_ptr(),
                action_builder.actions.len(),
            )
        })
    }
}

fn resolve_relative(root: &str, append: &str) -> Option<String> {
    if append.starts_with('/') {
        return None;
    }

    let mut root = root.to_string();
    if root.ends_with('/') {
        root.pop();
    }
    let segments = append
        .split('/')
        .filter(|&segment| !segment.is_empty() && segment != ".");
    for segment in segments {
        if segment == ".." {
            if let Some((r, _)) = root.rsplit_once('/') {
                root.truncate(r.len());
            }
        } else {
            root.push('/');
            root.push_str(segment);
        }
    }

    Some(root)
}

#[derive(Debug, Default)]
pub struct FDActionBuilder<'a> {
    actions: Vec<FDAction>,
    _phantom_life: PhantomData<&'a u8>,
}

#[allow(dead_code)]
impl<'a> FDActionBuilder<'a> {
    fn new() -> Self {
        Self {
            actions: Vec::new(),
            _phantom_life: PhantomData,
        }
    }

    fn reset(mut self) -> Self {
        self.actions.clear();
        self
    }

    fn add_clear(mut self) -> Self {
        self.actions.clear();
        self.actions.push(FDAction::Clear);
        self
    }

    fn add_open(mut self, path: &'a str, flags: OpenOptions, to: FileDescriptor) -> Self {
        let arr = path.as_bytes();
        self.actions.push(FDAction::Open(
            syscalls::FDOpen {
                path: syscalls::FatPtr {
                    size: arr.len(),
                    thin: arr.as_ptr(),
                },
                flags,
            },
            to,
        ));
        self
    }

    fn add_close(mut self, fd: FileDescriptor) -> Self {
        self.actions.push(FDAction::Close(fd));
        self
    }

    fn add_dup(mut self, from: FileDescriptor, to: FileDescriptor) -> Self {
        self.actions.push(FDAction::Dup(from, to));
        self
    }

    fn add_move(mut self, from: FileDescriptor, to: FileDescriptor) -> Self {
        self = self.add_dup(from, to);
        self.add_close(from)
    }

    fn add_inherit(mut self, from: FileDescriptor, to: FileDescriptor) -> Self {
        self.actions.push(FDAction::Inherit(from, to));
        self
    }

    fn ptr(&'a self) -> &'a [FDAction] {
        &self.actions
    }
}

#[derive(Default, Clone, Debug)]
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
pub struct OpenFd(FileDescriptor);

impl Drop for OpenFd {
    fn drop(&mut self) {
        unsafe {
            _ = syscalls::close(self.0).inspect_err(|e| {
                eprintln!(
                    "\x1b[31m[SHELL ERR]\x1b[0m could not close fd {}, due to {:?}",
                    self.0, e
                );
            })
        };
    }
}

#[derive(Debug)]
struct Pipe<'a> {
    to: Box<Command_<'a>>,
    connections: HashMap<FileDescriptor, Vec<FileDescriptor>>,
}

#[derive(Debug)]
struct Redirection<'a> {
    fds: Vec<FileDescriptor>,
    mode: RedirectionMode,
    file: &'a str,
}

impl<'a> Redirection<'a> {
    fn add_to(
        &self,
        mut builder: FDActionBuilder<'a>,
    ) -> SysCallRes<(FDActionBuilder<'a>, OpenFd)> {
        match self.mode {
            RedirectionMode::Empty => Err(syscalls::SysErrCode::NoErr),

            _ => {
                let bytes = self.file.as_bytes();
                let fd = unsafe { syscalls::open(bytes.as_ptr(), bytes.len(), self.mode.into()) }?;

                for to in &self.fds {
                    builder = builder.add_inherit(fd, *to);
                }
                Ok((builder, OpenFd(fd)))
            }
        }
    }
}

pub fn wait_(ctx: ExecutionContext) -> SysCallRes<()> {
    let env = get_env();
    env.set_ctx(ctx.clone());
    env.set_bg();
    let r = ctx.wait_all();
    env.clear_ctx();
    r
}

pub fn signal_handler(signal_pipe: FileDescriptor) {
    let mut buffer = [0_u8; 10];
    let env = get_env();
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
                    if !env.is_fg()
                        && let Some(_) = env.get_ctx()
                    {
                        env.clear_ctx();
                        env.set_fg();
                    }
                    serial_println!("[signal abort]")
                }
                1 => serial_println!("[signal bg]"),
                _ => serial_println!("[signal] unknown [{}]", signal),
            };
        }
    }
}
