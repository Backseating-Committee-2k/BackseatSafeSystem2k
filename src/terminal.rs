// featuring Tom Hanks

use std::mem::size_of;

use crate::{memory::Memory, Address, Word};
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
        let words = &memory[row * WIDTH / size_of::<Word>()..][..WIDTH / size_of::<Word>()];
        let string: String = words
            .iter()
            .flat_map(|word| word.to_be_bytes())
            .map(|c| c.clamp(32, 127))
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
    use super::*;

    #[test]
    fn terminal_character_width_divisible_by_word_size() {
        assert_eq!(WIDTH % size_of::<Word>(), 0);
    }
}
