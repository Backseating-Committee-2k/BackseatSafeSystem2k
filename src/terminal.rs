// featuring Tom Hanks

use crate::{memory::Memory, Address};
use raylib::prelude::*;

pub const WIDTH: usize = 80;
pub const HEIGHT: usize = 25;
pub const MEMORY_OFFSET: Address = 0x0;

pub fn render(
    memory: &Memory,
    draw_handle: &mut RaylibDrawHandle,
    position: Vector2,
    font: &Font,
    font_height: f32,
) {
    for row in 0..HEIGHT {
        let words = &memory[row * WIDTH..][..WIDTH];
        let string: String = words
            .iter()
            .map(|&word| {
                if !(32..=255).contains(&word) {
                    b' '
                } else {
                    word as u8
                }
            })
            .map(|c| c as char)
            .collect();
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
