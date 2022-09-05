use crate::{cursor::Cursor, display, keyboard::Keyboard, timer::Timer};

pub trait Periphery {
    type Handle;
    type Thread;

    fn timer(&mut self) -> &mut Timer;
    fn keyboard(&mut self) -> &mut Keyboard;
    fn display(
        &mut self,
    ) -> &mut dyn display::Display<Handle = Self::Handle, Thread = Self::Thread>;
    fn cursor(&mut self) -> &mut Cursor;
}

pub struct PeripheryImplementation<Display: display::Display> {
    pub timer: Timer,
    pub keyboard: Keyboard,
    pub display: Display,
    pub cursor: Cursor,
}

impl<Display: display::Display> Periphery for PeripheryImplementation<Display> {
    type Handle = Display::Handle;
    type Thread = Display::Thread;

    fn timer(&mut self) -> &mut Timer {
        &mut self.timer
    }

    fn keyboard(&mut self) -> &mut Keyboard {
        &mut self.keyboard
    }

    fn display(
        &mut self,
    ) -> &mut dyn display::Display<Handle = Self::Handle, Thread = Self::Thread> {
        &mut self.display
    }

    fn cursor(&mut self) -> &mut Cursor {
        &mut self.cursor
    }
}
