#![allow(dead_code)]

//! ## Opcodes
//! | Opcode                | Meaning                                   |
//! |-----------------------|-------------------------------------------|
//! | `0000 RR__ CCCC CCCC` | move the value C into register R |
//! | `0001 RR__ AAAA AAAA` | move the value at address A into register R |
//! | `0002 TTSS ____ ____` | move the contents of register S into register T |
//! | `0003 RR00 AAAA AAAA` | move the contents of register R into memory at address A |
//! | `0004 TTPP ____ ____` | move the contents addressed by the value of register P into register T |
//! | `0005 PPSS ____ ____` | move the contents of register S into memory at address specified by register P |

mod machine;
mod memory;
mod processor;
mod terminal;

use machine::Machine;
use raylib::prelude::*;

pub struct Size2D {
    width: i32,
    height: i32,
}

pub const SCREEN_SIZE: Size2D = Size2D {
    width: 1600,
    height: 900,
};

const fn static_assert(condition: bool) {
    assert!(condition);
}

pub type Instruction = u64;
pub type Word = u32;
pub type HalfWord = u16;
pub type Address = u32;

const _: () = static_assert(HalfWord::SIZE * 2 == Word::SIZE);

pub trait AsHalfWords {
    fn as_half_words(&self) -> (HalfWord, HalfWord);
}

impl AsHalfWords for Word {
    fn as_half_words(&self) -> (HalfWord, HalfWord) {
        ((self >> 8 * HalfWord::SIZE) as HalfWord, *self as HalfWord)
    }
}

pub trait AsWords {
    fn as_words(&self) -> (Word, Word);
}

impl AsWords for Instruction {
    fn as_words(&self) -> (Word, Word) {
        ((self >> Word::SIZE * 8) as Word, *self as Word)
    }
}

pub trait Size: Sized {
    const SIZE: usize = std::mem::size_of::<Self>();
}

impl Size for Instruction {}
impl Size for Word {}
impl Size for HalfWord {}

fn save_instructions(machine: &mut Machine, instructions: &[Instruction]) {
    use processor::Processor;
    let mut address = Processor::ENTRY_POINT;
    for &instruction in instructions {
        machine.memory.write_instruction(address, instruction);
        address += Instruction::SIZE as Address;
    }
}

fn main() {
    let (mut raylib_handle, thread) = raylib::init()
        .size(SCREEN_SIZE.width, SCREEN_SIZE.height)
        .title("Backseater")
        .build();
    let mut machine = Machine::new();
    let font = raylib_handle
        .load_font(&thread, "./resources/CozetteVector.ttf")
        .expect("Could not load font");
    /*let text = "MMMM";
    let value = u32::from_be_bytes(text.as_bytes().try_into().unwrap());
    machine
        .memory
        .fill(0..(terminal::WIDTH * terminal::HEIGHT) as Address, value);
    machine.memory.write_data(
        4 * 12,
        u32::from_be_bytes("llll".as_bytes().try_into().unwrap()),
    );*/
    let instructions = &[
        0x0000_4200_4865_6C6C, // "Hell"
        0x0003_4200_0000_0000,
        0x0000_4200_6F20_776F, // "o wo"
        0x0003_4200_0000_0004,
        0x0000_4200_726C_6421, // "rld!"
        0x0003_4200_0000_0008,
    ];
    save_instructions(&mut machine, instructions);
    for _ in 0..instructions.len() {
        machine.make_tick();
    }

    while !raylib_handle.window_should_close() {
        let mut draw_handle = raylib_handle.begin_drawing(&thread);
        draw_handle.clear_background(Color::BLACK);
        machine.render(&mut draw_handle, &font);
        draw_handle.draw_fps(10, 10);
    }
}
