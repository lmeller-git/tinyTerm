#![no_std]
#![no_main]

extern crate alloc;
use core::{
    fmt::{Display, Write},
    ptr::null,
    sync::atomic::{AtomicBool, AtomicU32, AtomicU64},
    time::Duration,
};
use libtinyos::{
    print, println, serial_println,
    syscalls::{OpenOptions, STDIN_FILENO, TaskWaitOptions, WaitOptions},
    thread,
};

use alloc::{string::String, vec::Vec};
use libtinyos::{eprintln, syscalls};
use vte::{
    Params, Parser, Perform,
    ansi::{Handler, Processor, Timeout},
};

mod io;
mod jobs;
mod parse;

struct Handle;

impl Handler for Handle {
    fn input(&mut self, c: char) {
        serial_println!("[print] {:?}", c);
    }

    fn move_up(&mut self, c: usize) {
        serial_println!("[up] {:?}", c);
    }

    fn move_down(&mut self, c: usize) {
        serial_println!("[down] {:?}", c);
    }

    fn move_forward(&mut self, c: usize) {
        serial_println!("[right] {:?}", c);
    }

    fn move_backward(&mut self, c: usize) {
        serial_println!("[left] {:?}", c);
    }

    fn backspace(&mut self) {
        serial_println!("[backspace]");
    }

    fn put_tab(&mut self, c: u16) {
        serial_println!("[tabs] {:?}", c);
    }

    fn linefeed(&mut self) {
        serial_println!("[linefeed]");
    }

    fn carriage_return(&mut self) {
        serial_println!("[cr]");
    }

    fn newline(&mut self) {
        serial_println!("[newline]");
    }

    fn delete_chars(&mut self, c: usize) {
        serial_println!("[delete] {:?}", c);
    }

    fn insert_blank(&mut self, c: usize) {
        serial_println!("[ins blacnk] {:?}", c);
    }

    fn bell(&mut self) {
        serial_println!("[bell]");
    }
    fn delete_lines(&mut self, c: usize) {
        serial_println!("[delete lines] {:?}", c);
    }

    fn clipboard_store(&mut self, c: u8, c2: &[u8]) {
        serial_println!("[clipboard store] {:?}, {:?}", c, c2);
    }

    fn clipboard_load(&mut self, c: u8, c2: &str) {
        serial_println!("[clipboard load] {:?}, {:?}", c, c2);
    }

    fn set_modify_other_keys(&mut self, mode: vte::ansi::ModifyOtherKeys) {
        serial_println!("[modify other] {:?}", mode);
    }
}

struct Performer;

impl Perform for Performer {
    fn print(&mut self, c: char) {
        serial_println!("[print] {:?}", c);
    }

    fn execute(&mut self, byte: u8) {
        serial_println!("[execute] {}", byte);
    }

    fn hook(&mut self, params: &Params, intermediates: &[u8], ignore: bool, c: char) {
        serial_println!(
            "[hook] params={:?}, intermediates={:?}, ignore={:?}, char={:?}",
            params,
            intermediates,
            ignore,
            c
        );
    }

    fn put(&mut self, byte: u8) {
        serial_println!("[put] {:02x}", byte);
    }

    fn unhook(&mut self) {
        serial_println!("[unhook]");
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], bell_terminated: bool) {
        serial_println!(
            "[osc_dispatch] params={:?} bell_terminated={}",
            params,
            bell_terminated
        );
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], ignore: bool, c: char) {
        serial_println!(
            "[csi_dispatch] params={:#?}, intermediates={:?}, ignore={:?}, char={:?}",
            params,
            intermediates,
            ignore,
            c
        );
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], ignore: bool, byte: u8) {
        serial_println!(
            "[esc_dispatch] intermediates={:?}, ignore={:?}, byte={:02x}",
            intermediates,
            ignore,
            byte
        );
    }
}

#[derive(Default)]
struct T {
    timeout: Option<Duration>,
}
impl Timeout for T {
    fn set_timeout(&mut self, duration: core::time::Duration) {
        self.timeout = Some(duration + Duration::from_millis(unsafe { syscalls::time() }.unwrap()))
    }

    fn clear_timeout(&mut self) {
        self.timeout.take();
    }

    fn pending_timeout(&self) -> bool {
        self.timeout.is_some()
    }
}

fn b() {
    let mut buf = [0; 64];
    let mut handle = Handle;
    let mut parser = Processor::<T>::new();
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

fn f() {
    let mut buf = [0; 64];
    let mut parser = Parser::<64>::new_with_size();
    let mut performer = Performer;
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

        parser.advance(&mut performer, &buf[..res as usize]);
    }
}

static CURRENT_FG: AtomicU64 = AtomicU64::new(0);
static WE_ARE_FG: AtomicBool = AtomicBool::new(false);

#[unsafe(no_mangle)]
extern "C" fn main() {
    println!("Hello, world!");
    let mut buf = [0; 10];
    let mut lines = Vec::new();

    let bin_dir_path = b"/ram/bin/";
    let bin_dir =
        unsafe { syscalls::open(bin_dir_path.as_ptr(), bin_dir_path.len(), OpenOptions::READ) }
            .unwrap();
    let mut ls_buf = [0; 256];
    let n =
        unsafe { syscalls::read(bin_dir, ls_buf.as_mut_ptr(), ls_buf.len(), 0) }.unwrap() as usize;
    let mut bins = str::from_utf8(&ls_buf[..n])
        .unwrap()
        .split("\t")
        .collect::<Vec<&str>>();
    bins.pop();
    serial_println!("ls bins = {:?}", bins);

    // the first 4 bytes sent over stdin by term will be the fd holding the signal pipe.
    // TODO
    // if this is not spawned by term, we should bail. Not sure, how to enforce this
    // --> could send some pre-defined exchange signals over some stdout/stdin?
    // Ie. send GetSignalFD in stdout -> term receives this -> sends SendSignalFd Fd over Stdin
    // -> if we get something else we bail -> send back ShellErr or ShellSetUp to start process?
    let mut signal_fd_buf = [0_u8; 4];
    while unsafe { syscalls::read(STDIN_FILENO, signal_fd_buf.as_mut_ptr(), 4, 0) }.unwrap() == 0 {}

    let signal_fd = u32::from_be_bytes(signal_fd_buf);

    thread::spawn(move || {
        let mut buffer = [0_u8; 10];
        loop {
            let n_read = unsafe {
                syscalls::read(
                    signal_fd,
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
                        if current_fg != 0 && !WE_ARE_FG.load(core::sync::atomic::Ordering::Acquire)
                        {
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
    })
    .unwrap();

    // b();
    // f();

    loop {
        println!("/");
        print!("> ");
        loop {
            let r = query_keyboard_once(&mut buf);
            for c in r.iter().filter(|item| {
                if let KeyCode::Char(_) = item {
                    true
                } else {
                    false
                }
            }) {
                print!("{}", c);
            }
            lines.extend(r);
            if let Some(last_ret) = lines.iter().position(|item| *item == KeyCode::Char('\n')) {
                let bin_end = lines
                    .iter()
                    .position(|item| *item == KeyCode::Char(' '))
                    .unwrap_or(last_ret);
                let split = &lines[..bin_end];
                serial_println!("split is {:?}", split);
                let name = split
                    .iter()
                    .filter_map(|item| {
                        if let KeyCode::Char(c) = item {
                            Some(c)
                        } else {
                            None
                        }
                    })
                    .collect::<String>();
                let args = &lines[bin_end..last_ret]
                    .iter()
                    .filter_map(|item| {
                        if let KeyCode::Char(c) = *item {
                            Some(c)
                        } else {
                            None
                        }
                    })
                    .collect::<String>();
                serial_println!("shell received {}, with args {}", name, args);
                if bins.contains(&name.as_ref()) {
                    let name_bytes = name.bytes();
                    let mut path = Vec::with_capacity(bin_dir_path.len() + name.len());
                    path.extend_from_slice(bin_dir_path);
                    path.extend(name_bytes);
                    if let Ok(exe_pid) = unsafe {
                        syscalls::execve(
                            path.as_ptr(),
                            path.len(),
                            args.as_ptr(),
                            args.bytes().count(),
                            null(),
                            0,
                        )
                    } {
                        CURRENT_FG.store(exe_pid, core::sync::atomic::Ordering::Release);
                        WE_ARE_FG.store(false, core::sync::atomic::Ordering::Release);
                        serial_println!(
                            "spawned process with path {}",
                            str::from_utf8(&path).unwrap()
                        );
                        _ = unsafe {
                            syscalls::wait_pid(
                                exe_pid,
                                -1,
                                WaitOptions::empty(),
                                TaskWaitOptions::W_EXIT,
                            )
                        }
                        .inspect_err(|e| {
                            eprintln!("failed to wait for process {}: {:?}", exe_pid, e);
                        });
                    } else {
                        eprintln!(
                            "could not spawn binary with path {}",
                            str::from_utf8(&path).unwrap()
                        );
                    };
                } else {
                    eprintln!("No binary with name {} exists.", name);
                }

                _ = lines.drain(..last_ret + 1);
                break;
            }
        }
    }
}

pub fn query_keyboard_once(buf: &mut [u8]) -> Vec<KeyCode> {
    let res = unsafe {
        syscalls::read(
            syscalls::STDIN_FILENO,
            buf.as_mut_ptr(),
            buf.len(),
            -1_i64 as usize,
        )
    };
    if let Ok(res) = res {
        parse_ansi(&buf[..res as usize])
    } else {
        eprintln!("Syscall read failed.");
        return Vec::new();
    }
}

fn parse_ansi(buf: &[u8]) -> Vec<KeyCode> {
    let mut codes = Vec::new();
    let mut cursor = 0;
    while let Some(current) = buf.get(cursor) {
        match *current {
            0x1B => codes.push(parse_escaped(buf, &mut cursor)),
            0x08 => {
                codes.push(KeyCode::BackSpace);
                cursor += 1;
            }
            _ => {
                codes.push(
                    str::from_utf8(&buf[cursor..=cursor])
                        .map(|s| KeyCode::Char(s.chars().next().unwrap_or('?')))
                        .unwrap_or(KeyCode::Unknown),
                );
                cursor += 1;
            }
        }
    }
    codes
}

fn parse_escaped(buf: &[u8], cursor: &mut usize) -> KeyCode {
    // for now we assume only arrows or a single esc
    match buf.get(*cursor + 1) {
        None => {
            *cursor += 1;
            KeyCode::Esc
        }
        Some(byte) => {
            if *byte == b'[' {
                match buf.get(*cursor + 2) {
                    None => {
                        *cursor += 1;
                        KeyCode::Esc
                    }
                    Some(byte) => {
                        *cursor += 3;

                        match byte {
                            b'A' => KeyCode::ArrowUp,
                            b'D' => KeyCode::ArrowLeft,
                            b'B' => KeyCode::ArrowDown,
                            b'C' => KeyCode::ArrowRight,
                            _ => {
                                *cursor -= 2;
                                KeyCode::Esc
                            }
                        }
                    }
                }
            } else {
                *cursor += 1;
                KeyCode::Esc
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Char(char),
    Esc,
    Unknown,
    BackSpace,
}

impl Display for KeyCode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ArrowUp => f.write_str("Up"),
            Self::ArrowDown => f.write_str("Down"),
            Self::ArrowLeft => f.write_str("Left"),
            Self::ArrowRight => f.write_str("Right"),
            Self::Char(c) => f.write_char(*c),
            Self::Esc => f.write_str("Esc"),
            Self::Unknown => f.write_str("Unknown"),
            Self::BackSpace => f.write_str("backspace"),
        }
    }
}
