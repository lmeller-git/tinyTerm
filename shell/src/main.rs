#![no_std]
#![no_main]

extern crate alloc;
use libtinyos::thread;

use libtinyos::syscalls;
use vte::ansi::Processor;

use crate::{
    init::init,
    logic::{SimpleTimeout, jobs::signal_handler, state::ShellState},
};

mod builtins;
mod env;
mod init;
mod io;
mod logic;
pub mod parse;

#[unsafe(no_mangle)]
extern "C" fn main() {
    let signal_fd = init();

    thread::spawn(move || signal_handler(signal_fd)).unwrap();

    let mut buf = [0; 64];
    let mut handle = ShellState::new();
    let mut parser = Processor::<SimpleTimeout>::new();
    drain_stdin();

    loop {
        let res = unsafe {
            syscalls::read(
                syscalls::STDIN_FILENO,
                buf.as_mut_ptr(),
                buf.len(),
                -1_i64 as usize,
            )
        }
        .unwrap();

        parser.advance(&mut handle, &buf[..res as usize]);
    }
}

fn drain_stdin() {
    let mut buf = [0; 64];
    while let Ok(n) =
        unsafe { syscalls::read(syscalls::STDIN_FILENO, buf.as_mut_ptr(), buf.len(), 0) }
        && n > 0
    {}
}
