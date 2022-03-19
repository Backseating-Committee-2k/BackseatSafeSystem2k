use crate::{keyboard::Keyboard, timer::Timer};

pub struct Periphery {
    pub timer: Timer,
    pub keyboard: Keyboard,
}
