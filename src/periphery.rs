use crate::{cursor::Cursor, display, keyboard::Keyboard, timer::Timer};

pub struct Periphery<Display: display::Display> {
    pub timer: Timer,
    pub keyboard: Keyboard,
    pub display: Display,
    pub cursor: Cursor,
}
