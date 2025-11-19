#![no_std]
#![no_main]

extern crate alloc;
use libtinyos::println;

#[unsafe(no_mangle)]
extern "C" fn main() {
    println!("Hello, world!");
}
