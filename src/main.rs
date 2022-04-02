mod address_constants;
mod display;
mod keyboard;
mod machine;
mod memory;
mod opcodes;
mod periphery;
mod processor;
mod terminal;
mod timer;

use std::{
    cell::RefCell,
    collections::HashMap,
    error::Error,
    fmt::{Debug, Write},
    io::{self, Read},
    path::{Path, PathBuf},
    rc::Rc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use clap::StructOpt;
use display::{Display, DisplayImplementation, MockDisplay};
use keyboard::{KeyState, Keyboard};
use machine::Machine;
use num_format::{CustomFormat, ToFormattedString};
use opcodes::Opcode;
use periphery::Periphery;
use processor::Processor;
use raylib::prelude::*;
use serde::{Deserialize, Serialize};
use timer::Timer;

use crate::{opcodes::OpcodeDescription, processor::Flag};

pub struct Size2D {
    width: i32,
    height: i32,
}

pub const SCREEN_SIZE: Size2D = Size2D {
    width: 1280,
    height: 720,
};

pub const OPCODE_LENGTH: usize = 16;

pub const fn static_assert(condition: bool) {
    assert!(condition);
}

pub const TARGET_FPS: u64 = 60;

pub type Instruction = u64;
pub type Word = u32;
pub type HalfWord = u16;
pub type Address = u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(clap::Subcommand, Debug)]
enum Action {
    /// Execute a ROM file (typically *.backseat)
    Run {
        /// The path to the ROM file to be executed
        path: Option<PathBuf>,
    },
    /// Emit a sample program as machine code
    Emit {
        /// Output path of the machine code to be written
        path: Option<PathBuf>,
    },
    /// Write the available opcodes and other information such as constants in JSON format    
    Json {
        /// Output path of the JSON file to be written
        path: Option<PathBuf>,
    },
    /// Read machine code (binary) and output a list of Instructions in Rust syntax
    Reverse {
        /// The path of the file to be written
        #[clap(short, long)]
        output_path: Option<PathBuf>,
        /// The path of the input file containing machine code
        #[clap(short, long)]
        input_path: Option<PathBuf>,
    },
}

/// The reference implementation of the backseat-safe-system-2k
#[derive(clap::Parser, Debug)]
struct Args {
    #[clap(subcommand)]
    action: Action,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    match args.action {
        Action::Run { path } => run(path.as_deref()),
        Action::Emit { path } => emit(path.as_deref()),
        Action::Json { path } => print_json(path.as_deref()),
        Action::Reverse {
            output_path,
            input_path,
        } => reverse(output_path.as_deref(), input_path.as_deref()),
    }
}

fn reverse(
    output_filename: Option<&Path>,
    input_filename: Option<&Path>,
) -> Result<(), Box<dyn Error>> {
    let periphery = Periphery {
        timer: Timer::new(ms_since_epoch),
        keyboard: Keyboard::new(Box::new(|_| KeyState::Up)),
        display: MockDisplay::new(&mut (), &()),
    };
    let mut machine = Machine::new(periphery);
    let num_instructions = match input_filename {
        Some(filename) => load_rom(&mut machine, filename)?,
        None => load_from_stdin(&mut machine)?,
    };

    let mut output_string = String::new();
    for i in 0..num_instructions {
        writeln!(
            &mut output_string,
            "{:?}",
            machine
                .memory
                .read_opcode(address_constants::ENTRY_POINT + (i * Instruction::SIZE) as Address)
                .unwrap()
        )?;
    }

    match output_filename {
        Some(filename) => std::fs::write(filename, &output_string)?,
        None => println!("{output_string}"),
    };
    Ok(())
}

fn load_from_stdin(machine: &mut Machine<impl display::Display>) -> Result<usize, Box<dyn Error>> {
    let instructions = read_machine_code_from_stdin()?;
    write_buffer_as_instructions(&instructions, machine)
}

fn read_machine_code_from_stdin() -> Result<Vec<u8>, Box<dyn Error>> {
    let mut instructions = Vec::new();
    std::io::stdin().read_to_end(&mut instructions)?;
    Ok(instructions)
}

fn print_json(output_filename: Option<&Path>) -> Result<(), Box<dyn Error>> {
    #[derive(Serialize)]
    struct JsonInfo {
        opcodes: HashMap<&'static str, OpcodeDescription>,
        constants: HashMap<&'static str, u64>,
        flags: HashMap<&'static str, usize>,
    }

    let json_info = JsonInfo {
        opcodes: Opcode::as_hashmap(),
        constants: HashMap::from([
            ("ENTRY_POINT", address_constants::ENTRY_POINT as _),
            ("NUM_REGISTERS", Processor::NUM_REGISTERS as _),
            ("FLAGS", Processor::FLAGS.0 as _),
            ("INSTRUCTION_POINTER", Processor::INSTRUCTION_POINTER.0 as _),
            ("STACK_POINTER", Processor::STACK_POINTER.0 as _),
            ("STACK_START", address_constants::STACK_START as _),
            ("STACK_SIZE", address_constants::STACK_SIZE as _),
            (
                "FIRST_FRAMEBUFFER_START",
                address_constants::FIRST_FRAMEBUFFER_START as _,
            ),
            (
                "SECOND_FRAMEBUFFER_START",
                address_constants::SECOND_FRAMEBUFFER_START as _,
            ),
            ("FRAMEBUFFER_SIZE", address_constants::FRAMEBUFFER_SIZE as _),
            ("TERMINAL_WIDTH", terminal::WIDTH as _),
            ("TERMINAL_HEIGHT", terminal::HEIGHT as _),
            (
                "TERMINAL_BUFFER_SIZE",
                address_constants::TERMINAL_BUFFER_SIZE as _,
            ),
            (
                "TERMINAL_BUFFER_START",
                address_constants::TERMINAL_BUFFER_START as _,
            ),
            ("DISPLAY_WIDTH", display::WIDTH as _),
            ("DISPLAY_HEIGHT", display::HEIGHT as _),
        ]),
        flags: Flag::as_hashmap(),
    };
    let json_string = serde_json::to_string_pretty(&json_info).unwrap();
    match output_filename {
        Some(filename) => std::fs::write(filename, &json_string)?,
        None => println!("{json_string}"),
    }

    Ok(())
}

fn emit(output_filename: Option<&Path>) -> Result<(), Box<dyn Error>> {
    let opcodes = &[
        Opcode::MoveRegisterImmediate {
            // starting color
            register: 0.into(),
            immediate: 0xFF,
        },
        Opcode::MoveRegisterImmediate {
            // num iterations
            register: 42.into(),
            immediate: (display::WIDTH * display::HEIGHT) as Word,
        },
        // outer loop start
        Opcode::MoveRegisterImmediate {
            // current loop counter
            register: 1.into(),
            immediate: 0,
        },
        Opcode::AddTargetSourceImmediate {
            // current color
            target: 0.into(),
            source: 0.into(),
            immediate: 0x200,
        },
        Opcode::MoveRegisterImmediate {
            register: 2.into(),
            immediate: address_constants::FIRST_FRAMEBUFFER_START,
        },
        // inner loop start
        Opcode::MovePointerSource {
            pointer: 2.into(),
            source: 0.into(),
        },
        Opcode::AddTargetSourceImmediate {
            target: 2.into(),
            source: 2.into(),
            immediate: Word::SIZE as Word,
        },
        Opcode::AddTargetSourceImmediate {
            target: 1.into(),
            source: 1.into(),
            immediate: 1,
        },
        Opcode::CompareTargetLhsRhs {
            target: 10.into(),
            lhs: 1.into(),
            rhs: 42.into(),
        },
        Opcode::JumpAddressIfLessThan {
            comparison: 10.into(),
            address: address_constants::ENTRY_POINT + 5 * Instruction::SIZE as Word,
        },
        Opcode::JumpAddress {
            address: address_constants::ENTRY_POINT + 2 * Instruction::SIZE as Word,
        },
    ];
    let machine_code = opcodes_to_machine_code(opcodes);
    match output_filename {
        Some(filename) => save_opcodes_as_machine_code(opcodes, filename)?,
        None => io::Write::write_all(&mut std::io::stdout(), &machine_code)?,
    }

    Ok(())
}

fn run(rom_filename: Option<&Path>) -> Result<(), Box<dyn Error>> {
    let (raylib_handle, raylib_thread) = raylib::init()
        .size(SCREEN_SIZE.width, SCREEN_SIZE.height)
        .title("Backseater")
        .build();
    let raylib_handle = Rc::new(RefCell::new(raylib_handle));
    let raylib_handle_copy = Rc::clone(&raylib_handle);
    let periphery = Periphery {
        timer: Timer::new(ms_since_epoch),
        keyboard: Keyboard::new(Box::new(move |key| {
            match raylib_handle_copy.borrow().is_key_down(
                raylib::input::key_from_i32(key.try_into().expect("keycode out of range"))
                    .expect("invalid keycode"),
            ) {
                true => KeyState::Down,
                false => KeyState::Up,
            }
        })),
        display: DisplayImplementation::new(&mut raylib_handle.borrow_mut(), &raylib_thread),
    };

    let mut machine = Machine::new(periphery);

    match rom_filename {
        Some(filename) => load_rom(&mut machine, filename)?,
        None => load_from_stdin(&mut machine)?,
    };

    let font = raylib_handle
        .borrow_mut()
        .load_font(&raylib_thread, "./resources/CozetteVector.ttf")?;

    let mut time_measurements = TimeMeasurements {
        next_render_time: ms_since_epoch(),
        last_cycle_count: 0,
        last_render_time: 0,
        clock_frequency_accumulator: 0,
        next_clock_frequency_render: ms_since_epoch() + 1000,
        num_clock_frequency_accumulations: 0,
        clock_frequency_average: 0,
    };

    let custom_number_format = CustomFormat::builder().separator(" ").build()?;

    while !raylib_handle.borrow().window_should_close() {
        let current_time = ms_since_epoch();
        render_if_needed(
            current_time,
            &mut time_measurements,
            &mut raylib_handle.borrow_mut(),
            &raylib_thread,
            &mut machine,
            &font,
            &custom_number_format,
        );

        let num_cycles = match (
            time_measurements.clock_frequency_average,
            current_time > time_measurements.next_render_time,
        ) {
            (_, true) => {
                time_measurements.next_render_time = current_time;
                0
            }
            (0, false) => 10_000,
            (_, false) => {
                let remaining_ms_until_next_render =
                    time_measurements.next_render_time - current_time;
                let cycle_duration = 1000.0 / time_measurements.clock_frequency_average as f64;
                (remaining_ms_until_next_render as f64 / cycle_duration - 10.0) as u64
            }
        };

        for _ in 0..num_cycles {
            execute_next_instruction(&mut machine);
        }
    }
    Ok(())
}

fn load_rom(
    machine: &mut Machine<impl display::Display>,
    filename: impl AsRef<Path>,
) -> Result<usize, Box<dyn Error>> {
    let buffer = std::fs::read(filename)?;
    write_buffer_as_instructions(&buffer, machine)
}

fn write_buffer_as_instructions(
    buffer: &[u8],
    machine: &mut Machine<impl display::Display>,
) -> Result<usize, Box<dyn Error>> {
    if buffer.len() % Instruction::SIZE != 0 {
        return Err(format!("Filesize must be divisible by {}", Instruction::SIZE).into());
    }
    let iterator = buffer
        .chunks_exact(Instruction::SIZE)
        .map(|slice| Instruction::from_be_bytes(slice.try_into().unwrap()));
    let num_instructions = iterator.len();
    for (instruction, address) in
        iterator.zip((address_constants::ENTRY_POINT..).step_by(Instruction::SIZE))
    {
        machine.memory.write_opcode(
            address,
            instruction.try_into().expect("Invalid instruction"),
        );
    }
    Ok(num_instructions)
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

fn execute_next_instruction(machine: &mut Machine<DisplayImplementation>) {
    if !machine.is_halted() {
        machine.execute_next_instruction();
    }
}

fn opcodes_to_machine_code(instructions: &[Opcode]) -> Vec<u8> {
    instructions
        .iter()
        .map(|opcode| opcode.as_instruction())
        .flat_map(|instruction| instruction.to_be_bytes())
        .collect()
}

fn save_opcodes_as_machine_code(instructions: &[Opcode], filename: &Path) -> io::Result<()> {
    let file_contents = opcodes_to_machine_code(instructions);
    std::fs::write(filename, &file_contents)
}

struct TimeMeasurements {
    next_render_time: u64,
    last_cycle_count: u64,
    last_render_time: u64,
    clock_frequency_accumulator: u64,
    next_clock_frequency_render: u64,
    num_clock_frequency_accumulations: u64,
    clock_frequency_average: u64,
}

fn render_if_needed(
    current_time: u64,
    time_measurements: &mut TimeMeasurements,
    raylib_handle: &mut RaylibHandle,
    thread: &RaylibThread,
    machine: &mut Machine<DisplayImplementation>,
    font: &Font,
    custom_number_format: &CustomFormat,
) {
    if current_time >= time_measurements.next_render_time {
        time_measurements.next_render_time += 1000 / TARGET_FPS;

        let mut draw_handle = raylib_handle.begin_drawing(thread);
        render(&mut draw_handle, machine, font);

        let current_cycle_count = machine.processor.get_cycle_count();
        if current_time != time_measurements.last_render_time {
            calculate_clock_frequency(current_time, time_measurements, current_cycle_count);
            draw_clock_frequency(
                time_measurements,
                custom_number_format,
                &mut draw_handle,
                font,
            );
        }
        time_measurements.last_render_time = current_time;
        time_measurements.last_cycle_count = current_cycle_count;
    }
}

fn render(
    draw_handle: &mut RaylibDrawHandle,
    machine: &mut Machine<DisplayImplementation>,
    font: &Font,
) {
    draw_handle.clear_background(Color::BLACK);
    machine.render(draw_handle, font);
    draw_handle.draw_fps(SCREEN_SIZE.width - 150, 10);
}

fn calculate_clock_frequency(
    current_time: u64,
    time_measurements: &mut TimeMeasurements,
    current_cycle_count: u64,
) {
    let time_since_last_render = current_time - time_measurements.last_render_time;
    let cycles_since_last_render = current_cycle_count - time_measurements.last_cycle_count;
    let clock_frequency = 1000 * cycles_since_last_render / time_since_last_render;
    time_measurements.clock_frequency_accumulator += clock_frequency;
    time_measurements.num_clock_frequency_accumulations += 1;
    if current_time >= time_measurements.next_clock_frequency_render {
        time_measurements.clock_frequency_average = time_measurements.clock_frequency_accumulator
            / time_measurements.num_clock_frequency_accumulations;
        time_measurements.next_clock_frequency_render = current_time + 1000;
        time_measurements.clock_frequency_accumulator = 0;
        time_measurements.num_clock_frequency_accumulations = 0;
    }
}

fn draw_clock_frequency(
    time_measurements: &TimeMeasurements,
    custom_number_format: &CustomFormat,
    draw_handle: &mut RaylibDrawHandle,
    font: &Font,
) {
    draw_handle.draw_text_ex(
        font,
        &*format!(
            "{} kHz",
            (time_measurements.clock_frequency_average / 1000)
                .to_formatted_string(custom_number_format)
        ),
        Vector2::new(SCREEN_SIZE.width as f32 - 200.0, 100.0),
        30.0,
        1.0,
        Color::WHITE,
    );
}
