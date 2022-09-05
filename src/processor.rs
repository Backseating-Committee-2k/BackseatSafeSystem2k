#![allow(non_upper_case_globals)]

use std::ops::{Index, IndexMut};

use crate::keyboard::KeyState;
use crate::opcodes::Opcode;
use crate::periphery::Periphery;
use crate::{address_constants, Byte, Halfword};
use crate::{dumper, static_assert};
use crate::{memory::Memory, Address, Instruction, Word};
use crate::{Register, Size};
use bitflags::bitflags;
use std::collections::HashMap;

const _: () = static_assert(address_constants::ENTRY_POINT as usize % Instruction::SIZE == 0);

pub enum Direction {
    Forwards,
    Backwards,
}

pub enum ExecutionResult {
    Error,
    Normal,
    Halted,
}

macro_rules! define_flags {
    ($(($flag_name:ident, shift = $shift:literal)),+) => {
        bitflags! {
            pub struct Flag: Word {
                $(
                    const $flag_name = 0b1 << $shift;
                )+
            }
        }

        impl Flag {
            pub fn as_hashmap() -> HashMap<&'static str, usize> {
                HashMap::from([
                    $(
                        (stringify!($flag_name), $shift),
                    )+
                ])
            }
        }
    };
}

define_flags![
    (Zero, shift = 0),
    (Carry, shift = 1),
    (DivideByZero, shift = 2)
];

pub struct Registers<const SIZE: usize>([Word; SIZE]);

impl<const SIZE: usize> Registers<SIZE> {
    const _ASSERT_VALID_REGISTER_COUNT: () = assert!(SIZE - 1 < u8::MAX as usize);
}

impl<const SIZE: usize> Index<Register> for Registers<SIZE> {
    type Output = Word;

    fn index(&self, index: Register) -> &Self::Output {
        &self.0[index.0 as usize]
    }
}

impl<const SIZE: usize> IndexMut<Register> for Registers<SIZE> {
    fn index_mut(&mut self, index: Register) -> &mut Self::Output {
        &mut self.0[index.0 as usize]
    }
}

pub const NUM_REGISTERS: usize = 256;

pub type CachedInstruction<ConcretePeriphery> =
    Box<dyn Fn(&mut Processor, &mut Memory, &mut ConcretePeriphery) -> ExecutionResult>;

pub struct InstructionCache<ConcretePeriphery: Periphery> {
    pub cache:
        Box<[Option<CachedInstruction<ConcretePeriphery>>; Memory::SIZE / Instruction::SIZE]>,
}

pub struct Processor {
    pub registers: Registers<{ NUM_REGISTERS }>,
    cycle_count: u64,
    exit_on_halt: bool,
    checkpoint_counter: Word,
}

impl Processor {
    pub const FLAGS: Register = Register((NUM_REGISTERS - 3) as _);
    pub const INSTRUCTION_POINTER: Register = Register((NUM_REGISTERS - 2) as _);
    pub const STACK_POINTER: Register = Register((NUM_REGISTERS - 1) as _);

    pub fn new(exit_on_halt: bool) -> Self {
        let mut result = Self {
            registers: Registers([0; NUM_REGISTERS]),
            cycle_count: 0,
            exit_on_halt,
            checkpoint_counter: 0,
        };
        result.registers[Self::INSTRUCTION_POINTER] = address_constants::ENTRY_POINT;
        result.registers[Self::STACK_POINTER] = address_constants::STACK_START;
        result
    }

    pub fn get_flag(&self, flag: Flag) -> bool {
        self.registers[Self::FLAGS] & flag.bits == flag.bits
    }

    pub fn set_flag(&mut self, flag: Flag, set: bool) {
        let mut flags = Flag::from_bits(self.registers[Self::FLAGS]).expect("Invalid flags value");
        flags.set(flag, set);
        self.registers[Self::FLAGS] = flags.bits;
    }

    pub fn get_stack_pointer(&self) -> Address {
        self.registers[Self::STACK_POINTER]
    }

    pub fn set_stack_pointer(&mut self, address: Address) {
        debug_assert!((address_constants::STACK_START
            ..=address_constants::STACK_START + address_constants::STACK_SIZE as Address)
            .contains(&address));
        self.registers[Self::STACK_POINTER] = address;
    }

    pub fn advance_stack_pointer(&mut self, step: usize, direction: Direction) {
        match direction {
            Direction::Forwards => {
                self.set_stack_pointer(self.get_stack_pointer() + step as Address)
            }
            Direction::Backwards => {
                self.set_stack_pointer(self.get_stack_pointer() - step as Address)
            }
        }
    }

    pub fn stack_push(&mut self, memory: &mut Memory, value: Word) {
        memory.write_data(self.get_stack_pointer(), value);
        self.advance_stack_pointer(Word::SIZE, Direction::Forwards);
    }

    pub fn stack_pop(&mut self, memory: &mut Memory) -> Word {
        self.advance_stack_pointer(Word::SIZE, Direction::Backwards);
        memory.read_data(self.get_stack_pointer())
    }

    pub fn set_instruction_pointer(&mut self, address: Address) {
        self.registers[Self::INSTRUCTION_POINTER] = address;
    }

    pub fn get_instruction_pointer(&self) -> Address {
        self.registers[Self::INSTRUCTION_POINTER]
    }

    pub fn advance_instruction_pointer(&mut self, direction: Direction) {
        match direction {
            Direction::Forwards => self.set_instruction_pointer(
                self.get_instruction_pointer() + Instruction::SIZE as Address,
            ),
            Direction::Backwards => self.set_instruction_pointer(
                self.get_instruction_pointer()
                    .saturating_sub(Instruction::SIZE as Address),
            ),
        }
    }

    pub fn get_cycle_count(&self) -> u64 {
        self.cycle_count
    }

    pub fn increase_cycle_count(&mut self, amount: u64) {
        self.cycle_count += amount;
    }

    pub fn generate_cached_instruction<ConcretePeriphery: Periphery>(
        opcode: Opcode,
    ) -> CachedInstruction<ConcretePeriphery> {
        use crate::processor::Opcode::*;
        let handle_cycle_count_and_instruction_pointer = move |processor: &mut Processor| {
            processor.increase_cycle_count(opcode.get_num_cycles().into());
            if opcode.should_increment_instruction_pointer() {
                processor.advance_instruction_pointer(Direction::Forwards);
            }
        };

        match opcode {
            MoveRegisterImmediate {
                register,
                immediate,
            } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    processor.registers[register] = immediate;
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            MoveRegisterAddress {
                register,
                source_address: address,
            } => Box::new(
                move |processor: &mut Processor,
                      memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    processor.registers[register] = memory.read_data(address);
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            MoveTargetSource { target, source } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    processor.registers[target] = processor.registers[source];
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            MoveAddressRegister {
                register,
                target_address: address,
            } => Box::new(
                move |processor: &mut Processor,
                      memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    memory.write_data(address, processor.registers[register]);
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            MoveTargetPointer { target, pointer } => Box::new(
                move |processor: &mut Processor,
                      memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    processor.registers[target] = memory.read_data(processor.registers[pointer]);
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            MovePointerSource { pointer, source } => Box::new(
                move |processor: &mut Processor,
                      memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    memory.write_data(processor.registers[pointer], processor.registers[source]);
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            MoveByteRegisterAddress {
                register,
                source_address,
            } => Box::new(
                move |processor: &mut Processor,
                      memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    processor.registers[register] = memory.read_byte(source_address) as Word;
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            MoveByteAddressRegister {
                register,
                target_address,
            } => Box::new(
                move |processor: &mut Processor,
                      memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    memory.write_byte(target_address, processor.registers[register] as u8);
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            MoveByteTargetPointer { target, pointer } => Box::new(
                move |processor: &mut Processor,
                      memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    processor.registers[target] =
                        memory.read_byte(processor.registers[pointer]) as Word;
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            MoveBytePointerSource { pointer, source } => Box::new(
                move |processor: &mut Processor,
                      memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    memory.write_byte(
                        processor.registers[pointer],
                        processor.registers[source] as u8,
                    );
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            MoveHalfwordRegisterAddress {
                register,
                source_address,
            } => Box::new(
                move |processor: &mut Processor,
                      memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    processor.registers[register] = memory.read_halfword(source_address).into();
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            MoveHalfwordAddressRegister {
                register,
                target_address,
            } => Box::new(
                move |processor: &mut Processor,
                      memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    memory.write_halfword(target_address, processor.registers[register] as u16);
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            MoveHalfwordTargetPointer { target, pointer } => Box::new(
                move |processor: &mut Processor,
                      memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    processor.registers[target] =
                        memory.read_halfword(processor.registers[pointer]).into();
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            MoveHalfwordPointerSource { pointer, source } => Box::new(
                move |processor: &mut Processor,
                      memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    memory.write_halfword(
                        processor.registers[pointer],
                        processor.registers[source] as u16,
                    );
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            MovePointerSourceOffset {
                pointer,
                source,
                immediate,
            } => Box::new(
                move |processor: &mut Processor,
                      memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    memory.write_data(
                        processor.registers[pointer] + immediate,
                        processor.registers[source],
                    );
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            MoveBytePointerSourceOffset {
                pointer,
                source,
                immediate,
            } => Box::new(
                move |processor: &mut Processor,
                      memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    memory.write_byte(
                        processor.registers[pointer] + immediate,
                        processor.registers[source] as Byte,
                    );
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            MoveHalfwordPointerSourceOffset {
                pointer,
                source,
                immediate,
            } => Box::new(
                move |processor: &mut Processor,
                      memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    memory.write_halfword(
                        processor.registers[pointer] + immediate,
                        processor.registers[source] as Halfword,
                    );
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            MoveTargetPointerOffset {
                target,
                pointer,
                immediate,
            } => Box::new(
                move |processor: &mut Processor,
                      memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    processor.registers[target] =
                        memory.read_data(processor.registers[pointer] + immediate);
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            MoveByteTargetPointerOffset {
                target,
                pointer,
                immediate,
            } => Box::new(
                move |processor: &mut Processor,
                      memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    processor.registers[target] = memory
                        .read_byte(processor.registers[pointer] + immediate)
                        .into();
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            MoveHalfwordTargetPointerOffset {
                target,
                pointer,
                immediate,
            } => Box::new(
                move |processor: &mut Processor,
                      memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    processor.registers[target] = memory
                        .read_halfword(processor.registers[pointer] + immediate)
                        .into();
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            HaltAndCatchFire {} => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    println!("HALT AND CATCH FIRE!");
                    if processor.exit_on_halt {
                        std::process::exit(0);
                    }
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Halted
                },
            ) as CachedInstruction<ConcretePeriphery>,
            AddTargetLhsRhs { target, lhs, rhs } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    let lhs = processor.registers[lhs];
                    let rhs = processor.registers[rhs];
                    let did_overflow;
                    (processor.registers[target], did_overflow) = lhs.overflowing_add(rhs);
                    processor.set_flag(Flag::Zero, processor.registers[target] == 0);
                    processor.set_flag(Flag::Carry, did_overflow);
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            SubtractTargetLhsRhs { target, lhs, rhs } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    let lhs = processor.registers[lhs];
                    let rhs = processor.registers[rhs];
                    let did_overflow;
                    (processor.registers[target], did_overflow) = lhs.overflowing_sub(rhs);
                    processor.set_flag(Flag::Zero, processor.registers[target] == 0);
                    processor.set_flag(Flag::Carry, did_overflow);
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            SubtractWithCarryTargetLhsRhs { target, lhs, rhs } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    let lhs = processor.registers[lhs];
                    let rhs = processor.registers[rhs];
                    let carry_flag_set = processor.get_flag(Flag::Carry);
                    let did_overflow;
                    (processor.registers[target], did_overflow) = lhs.overflowing_sub(rhs);
                    let did_overflow_after_subtracting_carry;
                    (
                        processor.registers[target],
                        did_overflow_after_subtracting_carry,
                    ) = processor.registers[target].overflowing_sub(carry_flag_set as _);
                    processor.set_flag(Flag::Zero, processor.registers[target] == 0);
                    processor.set_flag(
                        Flag::Carry,
                        did_overflow || did_overflow_after_subtracting_carry,
                    );
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            MultiplyHighLowLhsRhs {
                high,
                low,
                lhs,
                rhs,
            } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    let lhs = processor.registers[lhs];
                    let rhs = processor.registers[rhs];
                    let result = lhs as u64 * rhs as u64;
                    processor.registers[high] = (result >> 32) as u32;
                    processor.registers[low] = result as u32;
                    processor.set_flag(Flag::Zero, processor.registers[low] == 0);
                    processor.set_flag(Flag::Carry, processor.registers[high] > 0);
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            DivmodTargetModLhsRhs {
                result,
                remainder,
                lhs,
                rhs,
            } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    let lhs = processor.registers[lhs];
                    let rhs = processor.registers[rhs];
                    if rhs == 0 {
                        processor.registers[result] = 0;
                        processor.registers[remainder] = lhs;
                        processor.set_flag(Flag::Zero, true);
                        processor.set_flag(Flag::DivideByZero, true);
                    } else {
                        (processor.registers[result], processor.registers[remainder]) =
                            (lhs / rhs, lhs % rhs);
                        processor.set_flag(Flag::Zero, processor.registers[result] == 0);
                        processor.set_flag(Flag::DivideByZero, false);
                    }
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            AndTargetLhsRhs { target, lhs, rhs } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    let lhs = processor.registers[lhs];
                    let rhs = processor.registers[rhs];
                    processor.registers[target] = lhs & rhs;
                    processor.set_flag(Flag::Zero, processor.registers[target] == 0);
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            OrTargetLhsRhs { target, lhs, rhs } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    let lhs = processor.registers[lhs];
                    let rhs = processor.registers[rhs];
                    processor.registers[target] = lhs | rhs;
                    processor.set_flag(Flag::Zero, processor.registers[target] == 0);
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            XorTargetLhsRhs { target, lhs, rhs } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    let lhs = processor.registers[lhs];
                    let rhs = processor.registers[rhs];
                    processor.registers[target] = lhs ^ rhs;
                    processor.set_flag(Flag::Zero, processor.registers[target] == 0);
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            NotTargetSource { target, source } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    processor.registers[target] = !processor.registers[source];
                    processor.set_flag(Flag::Zero, processor.registers[target] == 0);
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            LeftShiftTargetLhsRhs { target, lhs, rhs } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    let lhs = processor.registers[lhs];
                    let rhs = processor.registers[rhs];
                    if rhs > Word::BITS {
                        processor.registers[target] = 0;
                        processor.set_flag(Flag::Zero, true);
                        processor.set_flag(Flag::Carry, lhs > 0);
                    } else {
                        let result = lhs << rhs;
                        processor.registers[target] = result;
                        processor.set_flag(Flag::Zero, result == 0);
                        processor.set_flag(Flag::Carry, rhs > lhs.leading_zeros());
                    }
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            RightShiftTargetLhsRhs { target, lhs, rhs } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    let lhs = processor.registers[lhs];
                    let rhs = processor.registers[rhs];
                    if rhs > Word::BITS {
                        processor.registers[target] = 0;
                        processor.set_flag(Flag::Zero, true);
                        processor.set_flag(Flag::Carry, lhs > 0);
                    } else {
                        let result = lhs >> rhs;
                        processor.registers[target] = result;
                        processor.set_flag(Flag::Zero, result == 0);
                        processor.set_flag(Flag::Carry, rhs > lhs.trailing_zeros());
                    }
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            AddTargetSourceImmediate {
                target,
                source,
                immediate,
            } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    let carry;
                    (processor.registers[target], carry) =
                        processor.registers[source].overflowing_add(immediate);
                    processor.set_flag(Flag::Zero, processor.registers[target] == 0);
                    processor.set_flag(Flag::Carry, carry);
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            SubtractTargetSourceImmediate {
                target,
                source,
                immediate,
            } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    processor.registers[target] =
                        processor.registers[source].wrapping_sub(immediate);
                    processor.set_flag(Flag::Zero, processor.registers[target] == 0);
                    processor.set_flag(Flag::Carry, immediate > processor.registers[source]);
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            CompareTargetLhsRhs { target, lhs, rhs } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    let lhs = processor.registers[lhs];
                    let rhs = processor.registers[rhs];
                    processor.registers[target] = match lhs.cmp(&rhs) {
                        std::cmp::Ordering::Less => Word::MAX,
                        std::cmp::Ordering::Equal => 0,
                        std::cmp::Ordering::Greater => 1,
                    };
                    processor.set_flag(Flag::Zero, processor.registers[target] == 0);
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            PushRegister { register } => Box::new(
                move |processor: &mut Processor,
                      memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    processor.stack_push(memory, processor.registers[register]);
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            PushImmediate { immediate } => Box::new(
                move |processor: &mut Processor,
                      memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    processor.stack_push(memory, immediate);
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            PopRegister { register } => Box::new(
                move |processor: &mut Processor,
                      memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    processor.registers[register] = processor.stack_pop(memory);
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            Pop {} => Box::new(
                move |processor: &mut Processor,
                      memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    processor.stack_pop(memory);
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            CallAddress {
                source_address: address,
            } => Box::new(
                move |processor: &mut Processor,
                      memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    processor.push_instruction_pointer(memory);
                    processor.set_instruction_pointer(address);
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            Return {} => Box::new(
                move |processor: &mut Processor,
                      memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    let return_address = processor.stack_pop(memory);
                    processor.set_instruction_pointer(return_address);
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            JumpImmediate { immediate: address } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    processor.set_instruction_pointer(address);
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            JumpRegister { register } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    processor.set_instruction_pointer(processor.registers[register]);
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            JumpImmediateIfEqual {
                comparison,
                immediate: address,
            } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    match processor.registers[comparison] {
                        0 => processor.set_instruction_pointer(address),
                        _ => processor.advance_instruction_pointer(Direction::Forwards),
                    }
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            JumpImmediateIfGreaterThan {
                comparison,
                immediate: address,
            } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    match processor.registers[comparison] {
                        1 => processor.set_instruction_pointer(address),
                        _ => processor.advance_instruction_pointer(Direction::Forwards),
                    };
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            JumpImmediateIfLessThan {
                comparison,
                immediate: address,
            } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    match processor.registers[comparison] {
                        Word::MAX => processor.set_instruction_pointer(address),
                        _ => processor.advance_instruction_pointer(Direction::Forwards),
                    };
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            JumpImmediateIfGreaterThanOrEqual {
                comparison,
                immediate: address,
            } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    match processor.registers[comparison] {
                        1 | 0 => processor.set_instruction_pointer(address),
                        _ => processor.advance_instruction_pointer(Direction::Forwards),
                    };
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            JumpImmediateIfLessThanOrEqual {
                comparison,
                immediate: address,
            } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    match processor.registers[comparison] {
                        Word::MAX | 0 => processor.set_instruction_pointer(address),
                        _ => processor.advance_instruction_pointer(Direction::Forwards),
                    }
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            JumpImmediateIfZero { immediate: address } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    match processor.get_flag(Flag::Zero) {
                        true => processor.set_instruction_pointer(address),
                        false => processor.advance_instruction_pointer(Direction::Forwards),
                    };
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            JumpImmediateIfNotZero { immediate: address } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    match processor.get_flag(Flag::Zero) {
                        false => processor.set_instruction_pointer(address),
                        true => processor.advance_instruction_pointer(Direction::Forwards),
                    };
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            JumpImmediateIfCarry { immediate: address } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    match processor.get_flag(Flag::Carry) {
                        true => processor.set_instruction_pointer(address),
                        false => processor.advance_instruction_pointer(Direction::Forwards),
                    };
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            JumpImmediateIfNotCarry { immediate: address } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    match processor.get_flag(Flag::Carry) {
                        false => processor.set_instruction_pointer(address),
                        true => processor.advance_instruction_pointer(Direction::Forwards),
                    };
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            JumpImmediateIfDivideByZero { immediate: address } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    match processor.get_flag(Flag::DivideByZero) {
                        true => processor.set_instruction_pointer(address),
                        false => processor.advance_instruction_pointer(Direction::Forwards),
                    };
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            JumpImmediateIfNotDivideByZero { immediate: address } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    match processor.get_flag(Flag::DivideByZero) {
                        false => processor.set_instruction_pointer(address),
                        true => processor.advance_instruction_pointer(Direction::Forwards),
                    }
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            JumpRegisterIfEqual {
                pointer,
                comparison,
            } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    match processor.registers[comparison] {
                        0 => processor.set_instruction_pointer(processor.registers[pointer]),
                        _ => processor.advance_instruction_pointer(Direction::Forwards),
                    };
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            JumpRegisterIfGreaterThan {
                pointer,
                comparison,
            } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    match processor.registers[comparison] {
                        1 => processor.set_instruction_pointer(processor.registers[pointer]),
                        _ => processor.advance_instruction_pointer(Direction::Forwards),
                    };
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            JumpRegisterIfLessThan {
                pointer,
                comparison,
            } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    match processor.registers[comparison] {
                        Word::MAX => {
                            processor.set_instruction_pointer(processor.registers[pointer])
                        }
                        _ => processor.advance_instruction_pointer(Direction::Forwards),
                    };
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            JumpRegisterIfGreaterThanOrEqual {
                pointer,
                comparison,
            } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    match processor.registers[comparison] {
                        1 | 0 => processor.set_instruction_pointer(processor.registers[pointer]),
                        _ => processor.advance_instruction_pointer(Direction::Forwards),
                    }
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            JumpRegisterIfLessThanOrEqual {
                pointer,
                comparison,
            } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    match processor.registers[comparison] {
                        Word::MAX | 0 => {
                            processor.set_instruction_pointer(processor.registers[pointer])
                        }
                        _ => processor.advance_instruction_pointer(Direction::Forwards),
                    };
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            JumpRegisterIfZero { pointer } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    match processor.get_flag(Flag::Zero) {
                        true => processor.set_instruction_pointer(processor.registers[pointer]),
                        false => processor.advance_instruction_pointer(Direction::Forwards),
                    };
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            JumpRegisterIfNotZero { pointer } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    match processor.get_flag(Flag::Zero) {
                        false => processor.set_instruction_pointer(processor.registers[pointer]),
                        true => processor.advance_instruction_pointer(Direction::Forwards),
                    };
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            JumpRegisterIfCarry { pointer } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    match processor.get_flag(Flag::Carry) {
                        true => processor.set_instruction_pointer(processor.registers[pointer]),
                        false => processor.advance_instruction_pointer(Direction::Forwards),
                    };
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            JumpRegisterIfNotCarry { pointer } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    match processor.get_flag(Flag::Carry) {
                        false => processor.set_instruction_pointer(processor.registers[pointer]),
                        true => processor.advance_instruction_pointer(Direction::Forwards),
                    };
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            JumpRegisterIfDivideByZero { pointer } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    match processor.get_flag(Flag::DivideByZero) {
                        true => processor.set_instruction_pointer(processor.registers[pointer]),
                        false => processor.advance_instruction_pointer(Direction::Forwards),
                    };
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            JumpRegisterIfNotDivideByZero { pointer } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    match processor.get_flag(Flag::DivideByZero) {
                        false => processor.set_instruction_pointer(processor.registers[pointer]),
                        true => processor.advance_instruction_pointer(Direction::Forwards),
                    };
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            NoOp {} => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            GetKeyState { target, keycode } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      periphery: &mut ConcretePeriphery| {
                    processor.registers[target] = matches!(
                        periphery
                            .keyboard()
                            .get_keystate(processor.registers[keycode] as _),
                        KeyState::Down
                    )
                    .into();
                    processor.set_flag(Flag::Zero, processor.registers[target] == 0);
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            PollTime { high, low } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      periphery: &mut ConcretePeriphery| {
                    let time = periphery.timer().get_ms_since_epoch();
                    processor.registers[low] = time as Word;
                    processor.registers[high] = (time >> Word::BITS) as Word;
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            AddWithCarryTargetLhsRhs { target, lhs, rhs } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    let result = processor.registers[lhs]
                        .wrapping_add(processor.registers[rhs])
                        .wrapping_add(processor.get_flag(Flag::Carry).into());
                    let overflow_happened = (processor.registers[lhs] as u64
                        + processor.registers[rhs] as u64
                        + processor.get_flag(Flag::Carry) as u64)
                        > Word::MAX as u64;
                    processor.registers[target] = result;
                    processor.set_flag(Flag::Zero, processor.registers[target] == 0);
                    processor.set_flag(Flag::Carry, overflow_happened);
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            CallRegister { register } => Box::new(
                move |processor: &mut Processor,
                      memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    processor.push_instruction_pointer(memory);
                    processor.set_instruction_pointer(processor.registers[register]);
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            CallPointer { pointer } => Box::new(
                move |processor: &mut Processor,
                      memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    processor.push_instruction_pointer(memory);
                    processor
                        .set_instruction_pointer(memory.read_data(processor.registers[pointer]));
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            SwapFramebuffers {} => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      periphery: &mut ConcretePeriphery| {
                    periphery.display().swap();
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            InvisibleFramebufferAddress { target } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      periphery: &mut ConcretePeriphery| {
                    processor.registers[target] =
                        periphery.display().invisible_framebuffer_address();
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            PollCycleCountHighLow { high, low } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    processor.registers[low] = processor.cycle_count as Word;
                    processor.registers[high] = (processor.cycle_count >> Word::BITS) as Word;
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            DumpRegisters {} => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    let data: Vec<_> = processor
                        .registers
                        .0
                        .iter()
                        .flat_map(|word| word.to_be_bytes())
                        .collect();
                    if let Err(error) = dumper::dump("registers", &data) {
                        eprintln!("Error dumping registers: {}", error);
                    }
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            DumpMemory {} => Box::new(
                move |processor: &mut Processor,
                      memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    if let Err(error) = dumper::dump("memory", memory.data()) {
                        eprintln!("Error dumping memory: {}", error);
                    }
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            AssertRegisterRegister { expected, actual } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    debug_assert_eq!(processor.registers[actual], processor.registers[expected]);
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            AssertRegisterImmediate { actual, immediate } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    debug_assert_eq!(processor.registers[actual], immediate);
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            AssertPointerImmediate { pointer, immediate } => Box::new(
                move |processor: &mut Processor,
                      memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    debug_assert_eq!(memory.read_data(processor.registers[pointer]), immediate);
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            DebugBreak {} => Box::new(
                move |_processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery|
                      -> ExecutionResult {
                    panic!();
                },
            ) as CachedInstruction<ConcretePeriphery>,
            PrintRegister { register } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    eprintln!(
                        "value of register {:#x}: {:#x} ({})",
                        register.0, processor.registers[register], processor.registers[register]
                    );
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
            BoolCompareEquals { target, lhs, rhs } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    processor.registers[target] =
                        if processor.registers[lhs] == processor.registers[rhs] {
                            1
                        } else {
                            0
                        };
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            BoolCompareNotEquals { target, lhs, rhs } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    processor.registers[target] =
                        if processor.registers[lhs] == processor.registers[rhs] {
                            0
                        } else {
                            1
                        };
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            BoolCompareGreater { target, lhs, rhs } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    processor.registers[target] =
                        if processor.registers[lhs] > processor.registers[rhs] {
                            1
                        } else {
                            0
                        };
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            BoolCompareGreaterOrEquals { target, lhs, rhs } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    processor.registers[target] =
                        if processor.registers[lhs] >= processor.registers[rhs] {
                            1
                        } else {
                            0
                        };
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            BoolCompareLess { target, lhs, rhs } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    processor.registers[target] =
                        if processor.registers[lhs] < processor.registers[rhs] {
                            1
                        } else {
                            0
                        };
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            BoolCompareLessOrEquals { target, lhs, rhs } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    processor.registers[target] =
                        if processor.registers[lhs] <= processor.registers[rhs] {
                            1
                        } else {
                            0
                        };
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            )
                as CachedInstruction<ConcretePeriphery>,
            Checkpoint { immediate } => Box::new(
                move |processor: &mut Processor,
                      _memory: &mut Memory,
                      _periphery: &mut ConcretePeriphery| {
                    if immediate != processor.checkpoint_counter {
                        panic!(
                            "checkpoint counter mismatch: expected {}, got {}",
                            immediate, processor.checkpoint_counter
                        );
                    }
                    processor.checkpoint_counter += 1;
                    handle_cycle_count_and_instruction_pointer(processor);
                    ExecutionResult::Normal
                },
            ) as CachedInstruction<ConcretePeriphery>,
        }
    }

    pub fn execute_next_instruction<ConcretePeriphery: Periphery>(
        &mut self,
        memory: &mut Memory,
        periphery: &mut ConcretePeriphery,
        instruction_cache: &mut InstructionCache<ConcretePeriphery>,
    ) -> ExecutionResult {
        let instruction_address = self.get_instruction_pointer();
        let cache_index = instruction_address / Instruction::SIZE as Address;
        match &instruction_cache.cache[cache_index as usize] {
            Some(cached_instruction) => cached_instruction(self, memory, periphery),
            None => match memory.read_opcode(instruction_address) {
                Ok(opcode) => {
                    let cached_instruction = Self::generate_cached_instruction(opcode);
                    instruction_cache.cache[cache_index as usize] = Some(cached_instruction);
                    eprint!(".");
                    instruction_cache.cache[cache_index as usize]
                        .as_ref()
                        .unwrap()(self, memory, periphery)
                }
                Err(err) => {
                    eprintln!("Error making tick: {}", err);
                    ExecutionResult::Error
                }
            },
        }
    }

    fn push_instruction_pointer(&mut self, memory: &mut Memory) {
        self.stack_push(
            memory,
            self.get_instruction_pointer() + Instruction::SIZE as Address,
        );
    }
}
