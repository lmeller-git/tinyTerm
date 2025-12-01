pub mod backend;
pub mod components;

pub const DEFAULT_CONF: &[u8] = b"border: white\ttext: white\tbg: black\ttitle: green";

pub const MAX_CHARS_PER_ROW: usize = 64;
pub const MAX_ROWS_PER_SCREEN: usize = 64;
