// featuring Tom Hanks

use crate::{address_constants, cursor::Cursor, memory::Memory, Address, Size, Word};

#[cfg(feature = "graphics")]
use raylib::prelude::*;

pub const WIDTH: usize = 80;
pub const HEIGHT: usize = 25;

#[cfg(feature = "graphics")]
pub fn render(
    memory: &Memory,
    draw_handle: &mut RaylibDrawHandle,
    position: Vector2,
    font: &Font,
    font_height: f32,
    cursor: &Cursor,
) {
    let cursor_pointer = memory.read_data(address_constants::TERMINAL_CURSOR_POINTER) as usize;
    debug_assert_eq!(address_constants::TERMINAL_BUFFER_START, 0); // to assume we get no overflow
    let cursor_index = cursor_pointer - address_constants::TERMINAL_BUFFER_START as usize;
    let cursor_row = cursor_index / WIDTH;
    let cursor_column = cursor_index % WIDTH;
    for row in 0..HEIGHT {
        // let words = &memory[row * WIDTH..][..WIDTH];
        let mut string: String = (0..WIDTH)
            .map(|i| {
                memory.read_byte(
                    address_constants::TERMINAL_BUFFER_START + (row * WIDTH + i) as Address,
                )
            })
            .map(|byte| {
                if !(32..=255).contains(&byte) {
                    b' '
                } else {
                    byte as u8
                }
            })
            .map(|c| c as char)
            .collect();
        if row == cursor_row && cursor.visible {
            let bytes = unsafe { string.as_bytes_mut() };
            debug_assert!(bytes[cursor_column].is_ascii());
            bytes[cursor_column] = b'_';
        }
        let text = string.as_str();

        draw_handle.draw_text_ex(
            font,
            text,
            Vector2::new(position.x, position.y + row as f32 * font_height as f32),
            font_height,
            5.0,
            Color::WHITE,
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::{Size, Word};

    use super::*;

    #[test]
    fn terminal_character_width_divisible_by_word_size() {
        assert_eq!(WIDTH % Word::SIZE, 0);
    }
}
