#![no_std]
#![no_main]

extern crate alloc;

use alloc::{boxed::Box, vec::Vec};
use conquer_once::spin::OnceCell;
use libtinyos::{
    eprint, eprintln, println,
    syscalls::{self, FileDescriptor, STDIN_FILENO},
    thread,
};
use ratatui::prelude::Backend;

use crate::{
    graphics::backend::{init_backend, init_drawer},
    init::{PipePair, init},
    parse::Config,
    state::TermState,
};

pub mod graphics;
mod init;
mod input;
pub mod parse;
pub mod state;

static CONFIG: OnceCell<Config> = OnceCell::uninit();

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    Abort = 0,
    Background = 1,
}

#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    let path = "/proc/kernel/io/serial";
    let serial = unsafe {
        syscalls::open(
            path.as_ptr(),
            path.bytes().len(),
            syscalls::OpenOptions::WRITE,
        )
    }
    .unwrap();
    unsafe { syscalls::dup(serial, Some(syscalls::STDOUT_FILENO)) }.unwrap();

    let drawer = init_drawer();
    let drawer_ref = Box::leak(drawer.into());
    let backend = init_backend(drawer_ref);
    let term = TermState::new(backend);

    println!("terminal hooked into serial, attached fb");

    let shell = "/ram/bin/tinyShell.out";

    let (pipes, shell_id) = init(shell, serial);

    println!("spawned shell, hooked to terminal and attached back to serial");

    thread::spawn(move || input_loop(pipes.input.unwrap().write, pipes.signal.unwrap())).unwrap();
    thread::spawn(move || stderr_handler(pipes.err.unwrap().read, shell_id)).unwrap();
    println!("background threads started up, we will now handle the shells in and output");

    stdout_handler(pipes.out.unwrap().read, term);

    eprintln!("Stdout handler exited. Shutting down terminal...");
    unsafe { syscalls::exit(0) }
}

fn input_loop(write_fd: FileDescriptor, signal_fds: PipePair) {
    // first we send the fd, which is coupled to signal pipe read end
    unsafe { syscalls::write(write_fd, signal_fds.read.to_be_bytes().as_ptr(), 4) }.unwrap();
    let mut buf = [0; 64];
    loop {
        unsafe { syscalls::seek(STDIN_FILENO, 0) }.unwrap();
        let read =
            unsafe { syscalls::read(STDIN_FILENO, buf.as_mut_ptr(), buf.len(), -1_i64 as usize) }
                .unwrap() as usize;
        let signals: Vec<u8> = buf[..read]
            .iter()
            .filter_map(|byte| match byte {
                3 => Some(Signal::Abort as u8),
                26 => Some(Signal::Background as u8),
                _ => None,
            })
            .collect();

        if !signals.is_empty() {
            unsafe { syscalls::write(signal_fds.write, signals.as_ptr(), signals.len()) }.unwrap();
        }
        if unsafe { syscalls::write(write_fd, buf.as_ptr(), read) }.is_err() {
            eprintln!("error writing to shel input pipe.");
        }
    }
}

fn stderr_handler(input_fd: FileDescriptor, pid: u64) {
    let mut buf = [0; 64];
    loop {
        let read =
            unsafe { syscalls::read(input_fd, buf.as_mut_ptr(), buf.len(), -1_i64 as usize) }
                .unwrap();
        let Ok(output) = core::str::from_utf8(&buf[..read as usize]) else {
            eprintln!("unknwon error in shell {} encountered", pid);
            panic!("unknown error in shell with id {}", pid)
        };
        eprint!("{}", output);
    }
}

fn stdout_handler<B: Backend>(input_fd: FileDescriptor, mut terminal: TermState<B>) {
    const BUF_SIZE: usize = 1024;
    let mut buf = [0; BUF_SIZE];
    let mut cursor = 0;
    let _conf = CONFIG.get_or_init(|| Config::new());
    loop {
        let read = unsafe {
            syscalls::read(
                input_fd,
                buf[cursor..].as_mut_ptr(),
                buf.len() - cursor,
                -1_i64 as usize,
            )
        }
        .unwrap();
        if let Ok(r) = str::from_utf8(&buf[..read as usize + cursor]) {
            terminal.update_state(r);
        }
        cursor += read as usize;
        if cursor >= BUF_SIZE {
            cursor = 0;
            buf.fill(0);
        }
    }
}
