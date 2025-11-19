#![no_std]
#![no_main]

extern crate alloc;
use core::{marker::PhantomData, time::Duration};

use alloc::{
    boxed::Box,
    string::String,
    vec::{self, Vec},
};
use libtinyos::{
    println, serial_println,
    syscalls::{self, TaskStateChange, TaskWaitOptions, WaitOptions},
    thread,
};
use ratatui::{
    backend::Backend,
    buffer::Cell,
    layout::{self, Position, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders},
};

use crate::graphics::backend::{init_backend, init_drawer, init_term};
use tinygraphics::draw_target::DrawTarget;

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

    serial_println!("Building drawer");
    let drawer = init_drawer();
    let drawer_ref = Box::leak(drawer.into());
    serial_println!("drawer box sitting at {:#x}", drawer_ref as *mut _ as usize);

    serial_println!("building backend");
    let backend = init_backend(drawer_ref);

    serial_println!("building term");
    let mut term = init_term(backend).unwrap();
    serial_println!("done");
    serial_println!("size is: {}", term.size().unwrap());

    serial_println!("trying to draw something");
    term.draw(|frame| {
        serial_println!("frame size is : {}", frame.area());
        frame.render_widget(
            Block::new()
                .title(Line::from("hello world"))
                .borders(Borders::ALL)
                .style(Style::new().fg(Color::Magenta).bg(Color::Green)),
            Rect::new(190, 70, 20, 10),
        );
    })
    .unwrap();

    let path = b"/ram/bin/example-rs.out";

    let time = unsafe { syscalls::time() }.unwrap();
    let time = Duration::from_millis(time);
    serial_println!("time before execve: {:?}", time);
    let id = unsafe { syscalls::execve(path.as_ptr(), path.len()) }.unwrap();
    let time2 = unsafe { syscalls::time() }.unwrap();
    let time2 = Duration::from_millis(time2);
    serial_println!("term is still alive at {:?} and spawned {}", time2, id);
    let r = unsafe { syscalls::wait_pid(id, -1, WaitOptions::empty(), TaskWaitOptions::W_EXIT) }
        .unwrap();
    assert_eq!(TaskStateChange::EXIT, r);
    let time3 = unsafe { syscalls::time() }.unwrap();
    let time3 = Duration::from_millis(time3);
    serial_println!(
        "we waited for example-rs to exit until {:?} for {:?}",
        time3,
        time3 - time2
    );

    let x = Foo {
        _x: 42,
        _p: PhantomData,
    };
    let tid = unsafe { syscalls::get_tid() };
    serial_println!("now spawning thread..., current id is {}", tid);
    let time4 = Duration::from_millis(unsafe { syscalls::time() }.unwrap());

    let handle = thread::spawn(move || {
        let tid = unsafe { syscalls::get_tid() };
        loop {
            serial_println!("hello form thread with id {}. The arg is {:?}", tid, x);
            unsafe {
                syscalls::waittime(5000);
            }
        }
    })
    .unwrap();
    unsafe { syscalls::thread_cancel(*handle.get_id()) };
    handle.join().unwrap();
    let time5 = Duration::from_millis(unsafe { syscalls::time() }.unwrap());
    serial_println!("thread joined, waited for {:?}", time5 - time4);

    unsafe { syscalls::exit(0) }
}

#[derive(Debug)]
struct Foo {
    _x: u64,
    _p: PhantomData<String>,
}
