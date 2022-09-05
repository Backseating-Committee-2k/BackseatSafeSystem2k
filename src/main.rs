mod address_constants;
mod cursor;
mod display;
mod dumper;
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
    fmt::Debug,
    io::{self, Read},
    path::{Path, PathBuf},
    rc::Rc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use address_constants::ENTRY_POINT;
use clap::StructOpt;
use cursor::Cursor;
use display::{Display, DisplayImplementation, MockDisplay};
use keyboard::{KeyState, Keyboard};
use machine::Machine;
use memory::Memory;
use num_format::{CustomFormat, ToFormattedString};
use opcodes::Opcode;
use periphery::PeripheryImplementation;
use processor::Processor;
use serde::{Deserialize, Serialize};
use timer::Timer;

#[cfg(feature = "graphics")]
use raylib::prelude::*;

use crate::{
    cursor::CursorMode,
    opcodes::OpcodeDescription,
    processor::{CachedInstruction, ExecutionResult, Flag, InstructionCache, NUM_REGISTERS},
};

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
pub type Halfword = u16;
pub type Byte = u8;
pub type Address = u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Register(pub u8);

impl From<u8> for Register {
    fn from(value: u8) -> Self {
        Self(value)
    }
}

const _: () = static_assert(Halfword::SIZE * 2 == Word::SIZE);

pub trait AsHalfwords {
    fn as_halfwords(&self) -> (Halfword, Halfword);
}

impl AsHalfwords for Word {
    fn as_halfwords(&self) -> (Halfword, Halfword) {
        (
            (self >> (8 * Halfword::SIZE)) as Halfword,
            *self as Halfword,
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
impl Size for Halfword {}
impl Size for Byte {}

#[derive(clap::Subcommand, Debug)]
enum Action {
    /// Execute a ROM file (typically *.backseat)
    Run {
        /// The path to the ROM file to be executed
        path: Option<PathBuf>,

        /// Applying this flag makes the application quit when executing the 'halt and catch fire'-
        /// instruction.
        #[clap(short, long, action)]
        exit_on_halt: bool,
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
        Action::Run { path, exit_on_halt } => run(path.as_deref(), exit_on_halt),
        Action::Emit { path } => emit(path.as_deref()),
        Action::Json { path } => print_json(path.as_deref()),
    }
}

fn load_from_stdin(machine: &mut Machine<impl display::Display>) -> Result<(), Box<dyn Error>> {
    let instructions = read_machine_code_from_stdin()?;
    write_buffer(&instructions, machine)
}

fn read_machine_code_from_stdin() -> Result<Vec<u8>, Box<dyn Error>> {
    let mut instructions = Vec::new();
    std::io::stdin().read_to_end(&mut instructions)?;
    Ok(instructions)
}

#[derive(Serialize)]
enum Constant {
    Register(Register),
    Address(Address),
    UnsignedInteger(u64),
}

fn print_json(output_filename: Option<&Path>) -> Result<(), Box<dyn Error>> {
    #[derive(Serialize)]
    struct JsonInfo {
        opcodes: HashMap<&'static str, OpcodeDescription>,
        constants: HashMap<&'static str, Constant>,
        flags: HashMap<&'static str, usize>,
    }

    let json_info = JsonInfo {
        opcodes: Opcode::as_hashmap(),
        constants: HashMap::from([
            (
                "ENTRY_POINT",
                Constant::Address(address_constants::ENTRY_POINT),
            ),
            (
                "NUM_REGISTERS",
                Constant::UnsignedInteger(NUM_REGISTERS as _),
            ),
            ("FLAGS", Constant::Register(Processor::FLAGS.0.into())),
            (
                "INSTRUCTION_POINTER",
                Constant::Register(Processor::INSTRUCTION_POINTER.0.into()),
            ),
            (
                "STACK_POINTER",
                Constant::Register(Processor::STACK_POINTER.0.into()),
            ),
            (
                "STACK_START",
                Constant::Address(address_constants::STACK_START),
            ),
            (
                "STACK_SIZE",
                Constant::UnsignedInteger(address_constants::STACK_SIZE as _),
            ),
            (
                "FIRST_FRAMEBUFFER_START",
                Constant::Address(address_constants::FIRST_FRAMEBUFFER_START),
            ),
            (
                "SECOND_FRAMEBUFFER_START",
                Constant::Address(address_constants::SECOND_FRAMEBUFFER_START),
            ),
            (
                "FRAMEBUFFER_SIZE",
                Constant::UnsignedInteger(address_constants::FRAMEBUFFER_SIZE as _),
            ),
            (
                "TERMINAL_WIDTH",
                Constant::UnsignedInteger(terminal::WIDTH as _),
            ),
            (
                "TERMINAL_HEIGHT",
                Constant::UnsignedInteger(terminal::HEIGHT as _),
            ),
            (
                "TERMINAL_BUFFER_SIZE",
                Constant::UnsignedInteger(address_constants::TERMINAL_BUFFER_SIZE as _),
            ),
            (
                "TERMINAL_BUFFER_START",
                Constant::Address(address_constants::TERMINAL_BUFFER_START),
            ),
            (
                "TERMINAL_BUFFER_END",
                Constant::Address(address_constants::TERMINAL_BUFFER_END),
            ),
            (
                "TERMINAL_CURSOR_POINTER",
                Constant::Address(address_constants::TERMINAL_CURSOR_POINTER),
            ),
            (
                "TERMINAL_CURSOR_MODE",
                Constant::Address(address_constants::TERMINAL_CURSOR_MODE),
            ),
            (
                "TERMINAL_CURSOR_MODE_BLINKING",
                Constant::UnsignedInteger(CursorMode::Blinking as _),
            ),
            (
                "TERMINAL_CURSOR_MODE_VISIBLE",
                Constant::UnsignedInteger(CursorMode::Visible as _),
            ),
            (
                "TERMINAL_CURSOR_MODE_INVISIBLE",
                Constant::UnsignedInteger(CursorMode::Invisible as _),
            ),
            (
                "DISPLAY_WIDTH",
                Constant::UnsignedInteger(display::WIDTH as _),
            ),
            (
                "DISPLAY_HEIGHT",
                Constant::UnsignedInteger(display::HEIGHT as _),
            ),
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
        Opcode::JumpImmediateIfLessThan {
            comparison: 10.into(),
            immediate: address_constants::ENTRY_POINT + 5 * Instruction::SIZE as Word,
        },
        Opcode::JumpImmediate {
            immediate: address_constants::ENTRY_POINT + 2 * Instruction::SIZE as Word,
        },
    ];
    let machine_code = opcodes_to_machine_code(opcodes);
    match output_filename {
        Some(filename) => save_opcodes_as_machine_code(opcodes, filename)?,
        None => io::Write::write_all(&mut std::io::stdout(), &machine_code)?,
    }

    Ok(())
}

fn run(rom_filename: Option<&Path>, exit_on_halt: bool) -> Result<(), Box<dyn Error>> {
    #[cfg(feature = "graphics")]
    let (raylib_handle, raylib_thread) = raylib::init()
        .size(SCREEN_SIZE.width, SCREEN_SIZE.height)
        .title("Backseater")
        .build();

    #[cfg(feature = "graphics")]
    let raylib_handle = Rc::new(RefCell::new(raylib_handle));

    #[cfg(feature = "graphics")]
    let raylib_handle_copy = Rc::clone(&raylib_handle);
    let periphery = PeripheryImplementation {
        timer: Timer::new(ms_since_epoch),
        keyboard: Keyboard::new(Box::new(move |key| {
            #[cfg(feature = "graphics")]
            match raylib_handle_copy.borrow().is_key_down(
                raylib::input::key_from_i32(key.try_into().expect("keycode out of range"))
                    .expect("invalid keycode"),
            ) {
                true => KeyState::Down,
                false => KeyState::Up,
            }

            #[cfg(not(feature = "graphics"))]
            KeyState::Up
        })),
        #[cfg(feature = "graphics")]
        display: DisplayImplementation::new(&mut raylib_handle.borrow_mut(), &raylib_thread),

        #[cfg(not(feature = "graphics"))]
        display: MockDisplay::new(&mut (), &mut ()),

        cursor: Cursor {
            visible: true,
            time_of_next_toggle: Instant::now() + Cursor::TOGGLE_INTERVAL,
        },
    };

    let mut machine = Machine::new(periphery, exit_on_halt);

    match rom_filename {
        Some(filename) => load_rom(&mut machine, filename)?,
        None => load_from_stdin(&mut machine)?,
    };
    machine.generate_instruction_cache();

    #[cfg(feature = "graphics")]
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

    while {
        #[cfg(feature = "graphics")]
        {
            !raylib_handle.borrow().window_should_close()
        }
        #[cfg(not(feature = "graphics"))]
        {
            true
        }
    } {
        let current_time = ms_since_epoch();
        #[cfg(feature = "graphics")]
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

fn load_rom<Display: display::Display + 'static>(
    machine: &mut Machine<Display>,
    filename: impl AsRef<Path>,
) -> Result<(), Box<dyn Error>> {
    let buffer = std::fs::read(filename)?;
    write_buffer(&buffer, machine)
}

fn write_buffer(
    buffer: &[u8],
    machine: &mut Machine<impl display::Display>,
) -> Result<(), Box<dyn Error>> {
    if (Memory::SIZE - ENTRY_POINT as usize) < buffer.len() {
        return Err(format!("Buffer size {} too big", buffer.len()).into());
    }
    if buffer.len() % Word::SIZE != 0 {
        return Err(format!("Filesize must be divisible by {}", Word::SIZE).into());
    }
    machine.memory.data_mut()[ENTRY_POINT as usize..][..buffer.len()].copy_from_slice(buffer);
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

fn execute_next_instruction<Display>(machine: &mut Machine<Display>)
where
    Display: crate::Display + 'static,
{
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

#[cfg(feature = "graphics")]
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

#[cfg(feature = "graphics")]
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

#[cfg(feature = "graphics")]
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
