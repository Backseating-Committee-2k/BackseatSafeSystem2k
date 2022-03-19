use crate::Word;

pub enum KeyState {
    Down,
    Up,
}

pub struct Keyboard {
    get_keystate_callback: Box<dyn FnMut(Word) -> KeyState>,
}

impl Keyboard {
    pub fn new(get_keystate_callback: Box<dyn FnMut(Word) -> KeyState>) -> Self {
        Keyboard {
            get_keystate_callback,
        }
    }

    pub fn get_keystate(&mut self, key: Word) -> KeyState {
        (self.get_keystate_callback)(key)
    }
}
