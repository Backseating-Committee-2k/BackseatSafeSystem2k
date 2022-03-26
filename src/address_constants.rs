use crate::{display, terminal, Address, Size, Word};

pub const TERMINAL_BUFFER_START: Address = 0;
pub const TERMINAL_BUFFER_SIZE: usize =
    ((terminal::WIDTH * terminal::HEIGHT + 2) as Address * Word::SIZE as Address) as usize;
pub const FRAMEBUFFER_SIZE: usize = display::WIDTH * display::HEIGHT * 4; // RGBA
pub const FIRST_FRAMEBUFFER_START: Address =
    TERMINAL_BUFFER_START + TERMINAL_BUFFER_SIZE as Address;
pub const SECOND_FRAMBUFFER_START: Address = FIRST_FRAMEBUFFER_START + FRAMEBUFFER_SIZE as Address;
pub const STACK_START: Address = SECOND_FRAMBUFFER_START + FRAMEBUFFER_SIZE as Address;
pub const STACK_SIZE: usize = 512 * 1024;
pub const ENTRY_POINT: Address = STACK_START + STACK_SIZE as Address;
