#![no_std]
#![no_main]

extern crate alloc;

use alloc::boxed::Box;
use libtinyos::{
    eprintln, println,
    syscalls::{self, FileDescriptor, OpenOptions, STDERR_FILENO, STDIN_FILENO, STDOUT_FILENO},
    thread,
};
use ratatui::{
    Terminal,
    prelude::Backend,
    style::{Color, Style, Stylize},
    text::Line,
    widgets::{Block, BorderType, Padding, Paragraph, Wrap},
};

use crate::graphics::backend::{init_backend, init_drawer, init_term};

mod background;
mod graphics;
mod input;

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
    let term = init_term(backend).unwrap();

    println!("terminal hooked into serial, attached fb");

    let shell = b"/ram/bin/shell";

    let mut input_ids = [0_u32, 0_u32];
    unsafe { syscalls::pipe(&mut input_ids as *mut [u32; 2]) }.unwrap();
    let mut output_ids = [0_u32, 0_u32];
    unsafe { syscalls::pipe(&mut output_ids as *mut [u32; 2]) }.unwrap();
    let mut err_ids = [0_u32, 0_u32];
    unsafe { syscalls::pipe(&mut err_ids as *mut [u32; 2]) }.unwrap();

    unsafe { syscalls::dup(input_ids[0], Some(STDIN_FILENO)) }.unwrap();
    unsafe { syscalls::dup(output_ids[1], Some(STDOUT_FILENO)) }.unwrap();
    unsafe { syscalls::dup(err_ids[1], Some(STDERR_FILENO)) }.unwrap();

    let shell_id = unsafe { syscalls::execve(shell.as_ptr(), shell.len()) }.unwrap();

    unsafe { syscalls::dup(serial, Some(STDOUT_FILENO)) }.unwrap();
    unsafe { syscalls::dup(serial, Some(STDERR_FILENO)) }.unwrap();

    let path = b"/proc/kernel/io/keyboard";
    let stdin = unsafe { syscalls::open(path.as_ptr(), path.len(), OpenOptions::READ) }.unwrap();
    unsafe { syscalls::dup(stdin, Some(STDIN_FILENO)) }.unwrap();

    println!("spawned shell, hooked to terminal and attached back to serial");

    thread::spawn(move || input_loop(input_ids[1])).unwrap();
    thread::spawn(move || stderr_handler(err_ids[0], shell_id)).unwrap();
    println!("background threads started up, we will now handle the shells in and output");

    stdout_handler(output_ids[0], term);

    eprintln!("Stdout handler exited. Shutting down terminal...");
    unsafe { syscalls::exit(0) }
}

fn input_loop(write_fd: FileDescriptor) {
    let mut buf = [0; 64];
    loop {
        let read =
            unsafe { syscalls::read(STDIN_FILENO, buf.as_mut_ptr(), buf.len(), -1_i64 as usize) }
                .unwrap();
        if unsafe { syscalls::write(write_fd, buf.as_ptr(), read as usize) }.is_err() {
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
        eprintln!("error in shell: {}", output);
    }
}

fn stdout_handler<B: Backend>(input_fd: FileDescriptor, mut terminal: Terminal<B>) {
    const BUF_SIZE: usize = 1024;
    let mut buf = [0; BUF_SIZE];
    let mut cursor = 0;
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
            terminal
                .draw(|frame| {
                    let block = Block::bordered()
                        .border_style(Style::new().fg(Color::Red).bg(Color::Blue))
                        .title_top(Line::from("Terminal").centered().bold().fg(Color::Yellow))
                        .border_type(BorderType::Rounded)
                        .padding(Padding::new(5, 5, 5, 5));
                    let paragraph = Paragraph::new(r)
                        .block(block)
                        .wrap(Wrap { trim: true })
                        .fg(Color::White);
                    frame.render_widget(paragraph, frame.area())
                })
                .unwrap();
        }
        cursor += read as usize;
        if cursor >= BUF_SIZE {
            cursor = 0;
            buf.fill(0);
        }
    }
}
