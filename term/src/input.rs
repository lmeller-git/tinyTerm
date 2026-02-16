use alloc::vec::Vec;
use libtinyos::{
    eprintln,
    syscalls::{self, FileDescriptor, STDIN_FILENO},
};

use crate::init::PipePair;

const SIGNAL_ABORT: u8 = 0;
const SIGNAL_BG: u8 = 1;

pub fn input_loop(write_fd: FileDescriptor, signal_fds: PipePair) {
    let mut buf = [0; 64];
    loop {
        unsafe { syscalls::seek(STDIN_FILENO, 0) }.unwrap();
        let read =
            unsafe { syscalls::read(STDIN_FILENO, buf.as_mut_ptr(), buf.len(), -1_isize as usize) }
                .unwrap() as usize;
        let signals: Vec<u8> = buf[..read]
            .iter()
            .filter_map(|byte| match byte {
                3 => Some(SIGNAL_ABORT),
                26 => Some(SIGNAL_BG),
                _ => None,
            })
            .collect();

        if !signals.is_empty() {
            unsafe { syscalls::write(signal_fds.write, signals.as_ptr(), signals.len()) }.unwrap();
        }
        if let Err(e) = unsafe { syscalls::write(write_fd, buf.as_ptr(), read) } {
            eprintln!("error writing to shel input pipe: {:?}", e);
        }
    }
}
