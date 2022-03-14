mod machine;
mod memory;
mod opcodes;
mod processor;
mod terminal;

use std::{
    env,
    error::Error,
    path::Path,
    thread::current,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use machine::Machine;
use num_format::{CustomFormat, Locale, ToFormattedString};
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
        (
            (self >> (8 * HalfWord::SIZE)) as HalfWord,
            *self as HalfWord,
        )
    }
}

pub trait AsWords {
    fn as_words(&self) -> (Word, Word);
}

impl AsWords for Instruction {
    fn as_words(&self) -> (Word, Word) {
        ((self >> (Word::SIZE * 8)) as Word, *self as Word)
    }
}

pub trait Size: Sized {
    const SIZE: usize = std::mem::size_of::<Self>();
}

impl Size for Instruction {}
impl Size for Word {}
impl Size for HalfWord {}

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
        machine.memory.write_opcode(
            address,
            instruction.try_into().expect("Invalid instruction"),
        );
    }
    Ok(())
}

fn duration_since_epoch() -> Duration {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
}

fn ms_since_epoch() -> u64 {
    let since_the_epoch = duration_since_epoch();
    since_the_epoch.as_secs() * 1000 + since_the_epoch.subsec_nanos() as u64 / 1_000_000
}

fn main() -> Result<(), Box<dyn Error>> {
    let rom_filename = env::args()
        .nth(1)
        .ok_or("Please specify the ROM to be loaded as a command line argument.")?;
    let mut machine = Machine::new();
    load_rom(&mut machine, rom_filename)?;
    let (mut raylib_handle, thread) = raylib::init()
        .size(SCREEN_SIZE.width, SCREEN_SIZE.height)
        .title("Backseater")
        .build();
    let font = raylib_handle.load_font(&thread, "./resources/CozetteVector.ttf")?;
    let mut is_halted = false;

    let target_fps = 60;
    let mut next_render_time = ms_since_epoch();
    let mut last_cycle_count = 0;
    let mut last_render_time = 0;
    let mut clock_frequency_accumulator = 0;
    let mut next_clock_frequency_render = ms_since_epoch() + 1000;
    let mut num_clock_frequency_accumulations = 0;
    let mut clock_frequency_average = 0;

    while !raylib_handle.window_should_close() {
        let current_time = ms_since_epoch();
        if current_time >= next_render_time {
            next_render_time = current_time + current_time - next_render_time + 1000 / target_fps;

            let mut draw_handle = raylib_handle.begin_drawing(&thread);
            draw_handle.clear_background(Color::BLACK);
            machine.render(&mut draw_handle, &font);
            draw_handle.draw_fps(SCREEN_SIZE.width - 150, 10);

            let current_cycle_count = machine.processor.get_cycle_count();
            if current_time != last_render_time {
                let time_since_last_render = current_time - last_render_time;
                let cycles_since_last_render = current_cycle_count - last_cycle_count;
                let clock_frequency = cycles_since_last_render / time_since_last_render * 1000;
                clock_frequency_accumulator += clock_frequency;
                num_clock_frequency_accumulations += 1;

                if current_time >= next_clock_frequency_render {
                    clock_frequency_average =
                        clock_frequency_accumulator / num_clock_frequency_accumulations;
                    next_clock_frequency_render = current_time + 1000;
                    clock_frequency_accumulator = 0;
                    num_clock_frequency_accumulations = 0;
                }
                let format = CustomFormat::builder().separator(" ").build()?;
                draw_handle.draw_text_ex(
                    &font,
                    &format!(
                        "{} kHz",
                        (clock_frequency_average / 1000).to_formatted_string(&format)
                    ),
                    Vector2::new(SCREEN_SIZE.width as f32 - 200.0, 100.0),
                    30.0,
                    1.0,
                    Color::WHITE,
                );
            }
            last_render_time = current_time;
            last_cycle_count = current_cycle_count;
        }

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
