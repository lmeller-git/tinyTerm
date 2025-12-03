use alloc::sync::Arc;
use crossbeam_queue::SegQueue;
use libtinyos::{
    eprint, eprintln,
    syscalls::{self, FileDescriptor},
};

use crate::state::EventPacket;

pub fn stderr_handler(
    input_fd: FileDescriptor,
    pid: u64,
    _event_queue: Arc<SegQueue<EventPacket>>,
) {
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

pub fn stdout_handler(input_fd: FileDescriptor, event_queue: Arc<SegQueue<EventPacket>>) {
    const BUF_SIZE: usize = 1024;
    let mut buf = [0; BUF_SIZE];
    loop {
        let read = unsafe { syscalls::read(input_fd, buf.as_mut_ptr(), buf.len(), -1_i64 as usize) }
            .unwrap() as usize;

        event_queue.push(buf[..read].as_ref().into());
    }
}
