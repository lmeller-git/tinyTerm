use core::time::Duration;

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
