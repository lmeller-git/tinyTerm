use core::time::Duration;

use alloc::string::String;
use libtinyos::syscalls;
use vte::ansi::Timeout;

pub mod jobs;
pub mod state;

#[derive(Default)]
pub struct SimpleTimeout {
    timeout: Option<Duration>,
}

impl Timeout for SimpleTimeout {
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

fn trim_string_in_place(s: &mut String) {
    if let Some(start) = s.find(|c: char| !c.is_whitespace())
        && let Some(end) = s.rfind(|c: char| !c.is_whitespace())
    {
        s.truncate(end + 1);
        _ = s.drain(..start);
    }
}
