#![no_std]
#![no_main]

extern crate alloc;
use core::fmt::{Display, Write};
use libtinyos::println;

use alloc::vec::Vec;
use libtinyos::{eprintln, syscalls};

#[unsafe(no_mangle)]
extern "C" fn main() {
    println!("Hello, world!");
    let mut buf = [0; 10];
    loop {
        let r = query_keyboard_once(&mut buf);
        println!("received: {:?}", r);
    }
}

pub fn query_keyboard_once(buf: &mut [u8]) -> Vec<KeyCode> {
    let res = unsafe { syscalls::read(syscalls::STDIN_FILENO, buf.as_mut_ptr(), buf.len(), 50) };
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
