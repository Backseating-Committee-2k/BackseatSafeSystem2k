use crate::{display, keyboard::Keyboard, timer::Timer};

pub struct Periphery<Display: display::Display> {
    pub timer: Timer,
    pub keyboard: Keyboard,
    pub display: Display,
}
