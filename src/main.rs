#![allow(dead_code)]

mod machine;
mod memory;
mod opcodes;
mod processor;
mod terminal;

use std::{error::Error, path::Path};

use machine::Machine;
use processor::Processor;
use raylib::prelude::*;

pub struct Size2D {
    width: i32,
    height: i32,
}

pub const SCREEN_SIZE: Size2D = Size2D {
    width: 1600,
    height: 900,
};

pub const OPCODE_LENGTH: usize = 16;

pub const fn static_assert(condition: bool) {
    assert!(condition);
}

pub type Instruction = u64;
pub type Word = u32;
pub type HalfWord = u16;
pub type Address = u32;

#[derive(Clone, Copy, Debug)]
pub struct Register(pub u8);

impl From<u8> for Register {
    fn from(value: u8) -> Self {
        Self(value)
    }
}

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
    let mut address = Processor::ENTRY_POINT;
    for &instruction in instructions {
        machine.memory.write_instruction(address, instruction);
        address += Instruction::SIZE as Address;
    }
}

fn load_rom(machine: &mut Machine, filename: impl AsRef<Path>) -> Result<(), Box<dyn Error>> {
    let buffer = std::fs::read(filename)?;
    if buffer.len() % Instruction::SIZE != 0 {
        return Err(format!("Filesize must be divisible by {}", Instruction::SIZE).into());
    }
    let iterator = buffer
        .chunks_exact(Instruction::SIZE)
        .map(|slice| Instruction::from_be_bytes(slice.try_into().unwrap()));
    for (instruction, address) in
        iterator.zip((Processor::ENTRY_POINT..).step_by(Instruction::SIZE))
    {
        machine.memory.write_instruction(address, instruction);
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let (mut raylib_handle, thread) = raylib::init()
        .size(SCREEN_SIZE.width, SCREEN_SIZE.height)
        .title("Backseater")
        .build();
    let mut machine = Machine::new();
    let font = raylib_handle
        .load_font(&thread, "./resources/CozetteVector.ttf")
        .expect("Could not load font");
    load_rom(&mut machine, "./roms/hello_world.backseat")?;
    let mut is_halted = false;
    while !raylib_handle.window_should_close() {
        let mut draw_handle = raylib_handle.begin_drawing(&thread);
        draw_handle.clear_background(Color::BLACK);
        machine.render(&mut draw_handle, &font);
        draw_handle.draw_fps(10, 10);
        match (is_halted, machine.is_halted()) {
            (false, true) => {
                is_halted = true;
                println!("HALT AND CATCH FIRE");
            }
            (false, false) => {
                machine.make_tick();
            }
            (_, _) => {}
        }
    }
    Ok(())
}
