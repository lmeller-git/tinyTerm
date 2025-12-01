#![no_std]
#![no_main]

extern crate alloc;

use alloc::{boxed::Box, sync::Arc};
use crossbeam_queue::SegQueue;
use libtinyos::{
    eprintln, println,
    syscalls::{self},
    thread,
};

use crate::{
    graphics::backend::{init_backend, init_drawer},
    init::init,
    input::input_loop,
    output::{stderr_handler, stdout_handler},
    state::{EventPacket, TermState},
};

pub mod graphics;
mod init;
mod input;
mod output;
pub mod parse;
pub mod state;

// terminal receives ANSI sequence from kernel stdin and parses it.
// We interpret all commmands as commands on the current virtual line, which may be mutated by these. It gets rerendered after every change.
// Once the line gets commited (vie linefeed), we send the finished line to the shell, which may act on it. We also commit it to history.
// Terminal now sends back stdout + stderr to us and we also add this to history immediately, while keeping the current virtual line.
// We continue listening for bytes, in order to act on signals such as ctrl-c, ..., which we relay to the shell (TODO: this needs to be done in some generic way (might need kernel signals)).
// Output of the shell again is parsed as ANSI sequence, in order to retrieve color, ...
// stin, stdout, stder, ... communicate with the state thread via commands, which get executed async

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

    let event_queue: Arc<SegQueue<EventPacket>> = SegQueue::new().into();

    thread::spawn(move || input_loop(pipes.input.unwrap().write, pipes.signal.unwrap())).unwrap();
    thread::spawn(move || stderr_handler(pipes.err.unwrap().read, shell_id)).unwrap();
    println!("background threads started up, we will now handle the shells in and output");

    stdout_handler(pipes.out.unwrap().read, term);

    eprintln!("Stdout handler exited. Shutting down terminal...");
    unsafe { syscalls::exit(0) }
}
