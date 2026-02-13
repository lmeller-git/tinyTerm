use core::ptr::null;

use libtinyos::{
    serial_print,
    syscalls::{self, FileDescriptor, OpenOptions, STDERR_FILENO, STDIN_FILENO, STDOUT_FILENO},
};

#[derive(Debug, Clone)]
pub struct OpenPipes {
    pub input: Option<PipePair>,
    pub err: Option<PipePair>,
    pub out: Option<PipePair>,
    pub signal: Option<PipePair>,
}

#[derive(Debug, Clone, Copy)]
pub struct PipePair {
    pub read: FileDescriptor,
    pub write: FileDescriptor,
}

impl From<[u32; 2]> for PipePair {
    fn from(value: [u32; 2]) -> Self {
        Self {
            read: value[0],
            write: value[1],
        }
    }
}

pub fn init(shell: &str, stdout: FileDescriptor) -> (OpenPipes, u64) {
    let mut input_ids = [0_u32, 0_u32];
    unsafe { syscalls::pipe(&mut input_ids as *mut [u32; 2], 32) }.unwrap();
    let mut output_ids = [0_u32, 0_u32];
    unsafe { syscalls::pipe(&mut output_ids as *mut [u32; 2], -1) }.unwrap();
    let mut err_ids = [0_u32, 0_u32];
    unsafe { syscalls::pipe(&mut err_ids as *mut [u32; 2], -1) }.unwrap();
    let mut signal_ids = [0_u32, 0_u32];
    unsafe { syscalls::pipe(&mut signal_ids as *mut [u32; 2], -1) }.unwrap();

    unsafe { syscalls::dup(input_ids[0], Some(STDIN_FILENO)) }.unwrap();
    unsafe { syscalls::dup(output_ids[1], Some(STDOUT_FILENO)) }.unwrap();
    unsafe { syscalls::dup(err_ids[1], Some(STDERR_FILENO)) }.unwrap();

    serial_print!("spawning");
    let shell_id = unsafe {
        syscalls::spawn_process(shell.as_ptr(), shell.len(), 0, null(), 0, null(), null(), 0)
    };
    serial_print!("spawn done");

    unsafe { syscalls::dup(stdout, Some(STDOUT_FILENO)) }.unwrap();
    unsafe { syscalls::dup(stdout, Some(STDERR_FILENO)) }.unwrap();

    let shell_id = shell_id.unwrap();

    let path = b"/proc/kernel/io/stateful_keyboard";
    let stdin = unsafe { syscalls::open(path.as_ptr(), path.len(), OpenOptions::READ) }.unwrap();
    unsafe { syscalls::dup(stdin, Some(STDIN_FILENO)) }.unwrap();
    // first we send the fd, which is coupled to signal pipe read end
    unsafe { syscalls::write(input_ids[1], signal_ids[0].to_be_bytes().as_ptr(), 4) }.unwrap();

    (
        OpenPipes {
            input: Some(input_ids.into()),
            err: Some(err_ids.into()),
            out: Some(output_ids.into()),
            signal: Some(signal_ids.into()),
        },
        shell_id,
    )
}
