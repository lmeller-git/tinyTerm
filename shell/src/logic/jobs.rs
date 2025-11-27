use core::{
    fmt::Display,
    ptr::null,
    sync::atomic::{AtomicBool, AtomicU64},
};

use alloc::string::String;
use libtinyos::{
    serial_println,
    syscalls::{self, FileDescriptor, SysCallRes, TaskWaitOptions, WaitOptions},
};

pub static CURRENT_FG: AtomicU64 = AtomicU64::new(0);
pub static WE_ARE_FG: AtomicBool = AtomicBool::new(false);

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Command {
    bin_name: String,
    argc: String,
}

impl Command {
    pub fn new(bin: &[char], args: &[char]) -> Self {
        let bin = bin.iter().collect();
        let args = args.iter().collect();
        Self {
            bin_name: bin,
            argc: args,
        }
    }

    pub fn bin(bin: &[char]) -> Self {
        Self::new(bin, [].as_slice())
    }

    pub fn execute(&self) -> SysCallRes<u64> {
        unsafe {
            syscalls::execve(
                self.bin_name.as_ptr(),
                self.bin_name.len(),
                self.argc.as_ptr(),
                self.argc.len(),
                null(),
                0,
            )
        }
    }
}

impl Display for Command {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Command")
            .field("Name", &self.bin_name)
            .field("Args", &self.argc)
            .finish()
    }
}

pub fn wait(pid: u64) {
    CURRENT_FG.store(pid, core::sync::atomic::Ordering::Release);
    WE_ARE_FG.store(false, core::sync::atomic::Ordering::Release);
    _ = unsafe { syscalls::wait_pid(pid, -1, WaitOptions::empty(), TaskWaitOptions::W_EXIT) };
}

pub fn signal_handler(signal_pipe: FileDescriptor) {
    let mut buffer = [0_u8; 10];
    loop {
        let n_read = unsafe {
            syscalls::read(
                signal_pipe,
                buffer.as_mut_ptr(),
                buffer.len(),
                -1_isize as usize,
            )
        }
        .unwrap();
        let signals = &buffer[..n_read as usize];
        for signal in signals {
            match signal {
                0 => {
                    let current_fg = CURRENT_FG.load(core::sync::atomic::Ordering::Acquire);
                    if current_fg != 0 && !WE_ARE_FG.load(core::sync::atomic::Ordering::Acquire) {
                        unsafe { syscalls::kill(current_fg, 0) }.unwrap();
                        WE_ARE_FG.store(true, core::sync::atomic::Ordering::Release);
                    }
                    serial_println!("[signal abort]")
                }
                1 => serial_println!("[signal bg]"),
                _ => serial_println!("[signal] unknown [{}]", signal),
            };
        }
    }
}
