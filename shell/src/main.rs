#![no_std]
#![no_main]

extern crate alloc;
use core::fmt::{Display, Write};
use libtinyos::{
    print, println, serial_println,
    syscalls::{OpenOptions, TaskWaitOptions, WaitOptions},
};

use alloc::{string::String, vec::Vec};
use libtinyos::{eprintln, syscalls};

#[unsafe(no_mangle)]
extern "C" fn main() {
    println!("Hello, world!");
    let mut buf = [0; 10];
    let mut lines = Vec::new();

    let bin_dir_path = b"/ram/bin/";
    let bin_dir =
        unsafe { syscalls::open(bin_dir_path.as_ptr(), bin_dir_path.len(), OpenOptions::READ) }
            .unwrap();
    let mut ls_buf = [0; 128];
    let n =
        unsafe { syscalls::read(bin_dir, ls_buf.as_mut_ptr(), ls_buf.len(), 0) }.unwrap() as usize;
    let mut bins = str::from_utf8(&ls_buf[..n])
        .unwrap()
        .split("\t")
        .collect::<Vec<&str>>();
    bins.pop();
    serial_println!("ls bins = {:?}", bins);

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
                let split = &lines[..last_ret];
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
                serial_println!("shell received {}", name);
                if bins.contains(&name.as_ref()) {
                    let name_bytes = name.bytes();
                    let mut path = Vec::with_capacity(bin_dir_path.len() + name.len());
                    path.extend_from_slice(bin_dir_path);
                    path.extend(name_bytes);
                    if let Ok(exe_pid) = unsafe { syscalls::execve(path.as_ptr(), path.len()) } {
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
        #[allow(clippy::single_match)]
        match *current {
            0x1B => codes.push(parse_escaped(buf, &mut cursor)),
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
        }
    }
}
