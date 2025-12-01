use alloc::{boxed::Box, sync::Arc, vec::Vec};
use crossbeam_queue::SegQueue;
use libtinyos::{
    eprintln,
    syscalls::{self, FileDescriptor, STDIN_FILENO},
};
use vte::ansi::Handler;

use crate::{init::PipePair, state::EventPacket};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    Abort = 0,
    Background = 1,
}
pub fn input_loop(write_fd: FileDescriptor, signal_fds: PipePair) {
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

pub struct InputHandler {
    event_queue_handle: Arc<SegQueue<EventPacket>>,
}

impl Handler for InputHandler {
    fn input(&mut self, c: char) {
        let closure: Box<dyn Fn(&mut dyn Handler) + Send> =
            Box::new(move |handler| handler.input(c));
        self.event_queue_handle.push(closure.into());
    }

    fn move_up(&mut self, n: usize) {
        let closure: Box<dyn Fn(&mut dyn Handler) + Send> =
            Box::new(move |handler| handler.move_up(n));
        self.event_queue_handle.push(closure.into());
    }

    fn move_down(&mut self, n: usize) {
        let closure: Box<dyn Fn(&mut dyn Handler) + Send> =
            Box::new(move |handler| handler.move_down(n));
        self.event_queue_handle.push(closure.into());
    }

    fn move_forward(&mut self, n: usize) {
        let closure: Box<dyn Fn(&mut dyn Handler) + Send> =
            Box::new(move |handler| handler.move_forward(n));
        self.event_queue_handle.push(closure.into());
    }

    fn move_backward(&mut self, n: usize) {
        let closure: Box<dyn Fn(&mut dyn Handler) + Send> =
            Box::new(move |handler| handler.move_backward(n));
        self.event_queue_handle.push(closure.into());
    }

    fn linefeed(&mut self) {
        let closure: Box<dyn Fn(&mut dyn Handler) + Send> =
            Box::new(move |handler| handler.linefeed());
        self.event_queue_handle.push(closure.into());
    }

    fn carriage_return(&mut self) {
        let closure: Box<dyn Fn(&mut dyn Handler) + Send> =
            Box::new(move |handler| handler.carriage_return());
        self.event_queue_handle.push(closure.into());
    }

    fn newline(&mut self) {
        let closure: Box<dyn Fn(&mut dyn Handler) + Send> =
            Box::new(move |handler| handler.newline());
        self.event_queue_handle.push(closure.into());
    }

    fn backspace(&mut self) {
        let closure: Box<dyn Fn(&mut dyn Handler) + Send> =
            Box::new(move |handler| handler.backspace());
        self.event_queue_handle.push(closure.into());
    }
}
