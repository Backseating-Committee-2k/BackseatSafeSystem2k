use crate::{terminal, Address, Size, Word};

pub const STACK_START: Address =
    (terminal::WIDTH * terminal::HEIGHT + 2) as Address * Word::SIZE as Address;
pub const STACK_SIZE: usize = 512 * 1024;
pub const ENTRY_POINT: Address = STACK_START + STACK_SIZE as Address;
