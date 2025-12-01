use libtinyos::{
    eprint, eprintln,
    syscalls::{self, FileDescriptor},
};
use ratatui::prelude::Backend;

use crate::state::TermState;

pub fn stderr_handler(input_fd: FileDescriptor, pid: u64) {
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

pub fn stdout_handler<B: Backend>(input_fd: FileDescriptor, mut terminal: TermState<B>) {
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
            terminal.update_state(r);
        }
        cursor += read as usize;
        if cursor >= BUF_SIZE {
            cursor = 0;
            buf.fill(0);
        }
    }
}
