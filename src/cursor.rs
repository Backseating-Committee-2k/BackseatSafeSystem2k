use std::time::{Duration, Instant};

use int_enum::IntEnum;

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, IntEnum)]
pub enum CursorMode {
    Blinking = 0,
    Visible = 1,
    Invisible = 2,
}

pub struct Cursor {
    pub visible: bool,
    pub time_of_next_toggle: Instant,
}

impl Cursor {
    pub const TOGGLE_INTERVAL: Duration = Duration::from_millis(400);
}
