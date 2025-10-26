#![no_std]
#![no_main]

extern crate alloc;
use libtinyos::{println, syscalls};

#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    println!("Hello, world!");
    unsafe { syscalls::exit(0) }
}
