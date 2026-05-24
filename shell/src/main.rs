#![no_std]
#![no_main]

extern crate alloc;
use libtinyos::serial_println;
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
fn main() {
    serial_println!("prepre starytup");
    let signal_fd = init();
    serial_println!("post init starytup");

    thread::spawn(move || signal_handler(signal_fd)).unwrap();

    let mut buf = [0; 64];
    serial_println!("pre starytup");
    let mut handle = ShellState::new();
    let mut parser = Processor::<SimpleTimeout>::new();

    serial_println!("spawned up shell");
    drain_stdin();

    serial_println!("entering main loop");

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
