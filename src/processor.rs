#![allow(non_upper_case_globals)]

use std::ops::{Index, IndexMut};

use crate::keyboard::KeyState;
use crate::opcodes::Opcode;
use crate::periphery::Periphery;
use crate::{address_constants, display};
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

pub struct Processor {
    pub registers: Registers<{ Self::NUM_REGISTERS }>,
    cycle_count: u64,
}

impl Processor {
    pub const NUM_REGISTERS: usize = 256;
    pub const FLAGS: Register = Register((Self::NUM_REGISTERS - 3) as _);
    pub const INSTRUCTION_POINTER: Register = Register((Self::NUM_REGISTERS - 2) as _);
    pub const STACK_POINTER: Register = Register((Self::NUM_REGISTERS - 1) as _);

    pub fn new() -> Self {
        let mut result = Self {
            registers: Registers([0; Self::NUM_REGISTERS]),
            cycle_count: 0,
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

    pub fn execute_next_instruction(
        &mut self,
        memory: &mut Memory,
        periphery: &mut Periphery<impl display::Display>,
    ) -> ExecutionResult {
        use crate::processor::Opcode::*;
        let mut halted = false;
        match memory.read_opcode(self.get_instruction_pointer()) {
            Ok(opcode) => {
                match opcode {
                    MoveRegisterImmediate {
                        register,
                        immediate,
                    } => self.registers[register] = immediate,
                    MoveRegisterAddress {
                        register,
                        source_address: address,
                    } => self.registers[register] = memory.read_data(address),
                    MoveTargetSource { target, source } => {
                        self.registers[target] = self.registers[source]
                    }
                    MoveAddressRegister {
                        register,
                        target_address: address,
                    } => memory.write_data(address, self.registers[register]),
                    MoveTargetPointer { target, pointer } => {
                        self.registers[target] = memory.read_data(self.registers[pointer])
                    }
                    MovePointerSource { pointer, source } => {
                        memory.write_data(self.registers[pointer], self.registers[source]);
                    }
                    HaltAndCatchFire {} => {
                        println!("HALT AND CATCH FIRE!");
                        halted = true;
                    }
                    AddTargetLhsRhs { target, lhs, rhs } => {
                        let lhs = self.registers[lhs];
                        let rhs = self.registers[rhs];
                        let did_overflow;
                        (self.registers[target], did_overflow) = lhs.overflowing_add(rhs);
                        self.set_flag(Flag::Zero, self.registers[target] == 0);
                        self.set_flag(Flag::Carry, did_overflow);
                    }
                    SubtractTargetLhsRhs { target, lhs, rhs } => {
                        let lhs = self.registers[lhs];
                        let rhs = self.registers[rhs];
                        let did_overflow;
                        (self.registers[target], did_overflow) = lhs.overflowing_sub(rhs);
                        self.set_flag(Flag::Zero, self.registers[target] == 0);
                        self.set_flag(Flag::Carry, did_overflow);
                    }
                    SubtractWithCarryTargetLhsRhs { target, lhs, rhs } => {
                        let lhs = self.registers[lhs];
                        let rhs = self.registers[rhs];
                        let carry_flag_set = self.get_flag(Flag::Carry);
                        let did_overflow;
                        (self.registers[target], did_overflow) = lhs.overflowing_sub(rhs);
                        let did_overflow_after_subtracting_carry;
                        (self.registers[target], did_overflow_after_subtracting_carry) =
                            self.registers[target].overflowing_sub(carry_flag_set as _);
                        self.set_flag(Flag::Zero, self.registers[target] == 0);
                        self.set_flag(
                            Flag::Carry,
                            did_overflow || did_overflow_after_subtracting_carry,
                        );
                    }
                    MultiplyHighLowLhsRhs {
                        high,
                        low,
                        lhs,
                        rhs,
                    } => {
                        let lhs = self.registers[lhs];
                        let rhs = self.registers[rhs];
                        let result = lhs as u64 * rhs as u64;
                        self.registers[high] = (result >> 32) as u32;
                        self.registers[low] = result as u32;
                        self.set_flag(Flag::Zero, self.registers[low] == 0);
                        self.set_flag(Flag::Carry, self.registers[high] > 0);
                    }
                    DivmodTargetModLhsRhs {
                        result,
                        remainder,
                        lhs,
                        rhs,
                    } => {
                        let lhs = self.registers[lhs];
                        let rhs = self.registers[rhs];
                        if rhs == 0 {
                            self.registers[result] = 0;
                            self.registers[remainder] = lhs;
                            self.set_flag(Flag::Zero, true);
                            self.set_flag(Flag::DivideByZero, true);
                        } else {
                            (self.registers[result], self.registers[remainder]) =
                                (lhs / rhs, lhs % rhs);
                            self.set_flag(Flag::Zero, self.registers[result] == 0);
                            self.set_flag(Flag::DivideByZero, false);
                        }
                    }
                    AndTargetLhsRhs { target, lhs, rhs } => {
                        let lhs = self.registers[lhs];
                        let rhs = self.registers[rhs];
                        self.registers[target] = lhs & rhs;
                        self.set_flag(Flag::Zero, self.registers[target] == 0);
                    }
                    OrTargetLhsRhs { target, lhs, rhs } => {
                        let lhs = self.registers[lhs];
                        let rhs = self.registers[rhs];
                        self.registers[target] = lhs | rhs;
                        self.set_flag(Flag::Zero, self.registers[target] == 0);
                    }
                    XorTargetLhsRhs { target, lhs, rhs } => {
                        let lhs = self.registers[lhs];
                        let rhs = self.registers[rhs];
                        self.registers[target] = lhs ^ rhs;
                        self.set_flag(Flag::Zero, self.registers[target] == 0);
                    }
                    NotTargetSource { target, source } => {
                        self.registers[target] = !self.registers[source];
                        self.set_flag(Flag::Zero, self.registers[target] == 0);
                    }
                    LeftShiftTargetLhsRhs { target, lhs, rhs } => {
                        let lhs = self.registers[lhs];
                        let rhs = self.registers[rhs];
                        if rhs > Word::BITS {
                            self.registers[target] = 0;
                            self.set_flag(Flag::Zero, true);
                            self.set_flag(Flag::Carry, lhs > 0);
                        } else {
                            let result = lhs << rhs;
                            self.registers[target] = result;
                            self.set_flag(Flag::Zero, result == 0);
                            self.set_flag(Flag::Carry, rhs > lhs.leading_zeros());
                        }
                    }
                    RightShiftTargetLhsRhs { target, lhs, rhs } => {
                        let lhs = self.registers[lhs];
                        let rhs = self.registers[rhs];
                        if rhs > Word::BITS {
                            self.registers[target] = 0;
                            self.set_flag(Flag::Zero, true);
                            self.set_flag(Flag::Carry, lhs > 0);
                        } else {
                            let result = lhs >> rhs;
                            self.registers[target] = result;
                            self.set_flag(Flag::Zero, result == 0);
                            self.set_flag(Flag::Carry, rhs > lhs.trailing_zeros());
                        }
                    }
                    AddTargetSourceImmediate {
                        target,
                        source,
                        immediate,
                    } => {
                        let carry;
                        (self.registers[target], carry) =
                            self.registers[source].overflowing_add(immediate);
                        self.set_flag(Flag::Zero, self.registers[target] == 0);
                        self.set_flag(Flag::Carry, carry);
                    }
                    SubtractTargetSourceImmediate {
                        target,
                        source,
                        immediate,
                    } => {
                        self.registers[target] = self.registers[source].wrapping_sub(immediate);
                        self.set_flag(Flag::Zero, self.registers[target] == 0);
                        self.set_flag(Flag::Carry, immediate > self.registers[source]);
                    }
                    CompareTargetLhsRhs { target, lhs, rhs } => {
                        let lhs = self.registers[lhs];
                        let rhs = self.registers[rhs];
                        self.registers[target] = match lhs.cmp(&rhs) {
                            std::cmp::Ordering::Less => Word::MAX,
                            std::cmp::Ordering::Equal => 0,
                            std::cmp::Ordering::Greater => 1,
                        };
                        self.set_flag(Flag::Zero, self.registers[target] == 0);
                    }
                    PushRegister { register } => {
                        self.stack_push(memory, self.registers[register]);
                    }
                    PopRegister { register } => {
                        self.registers[register] = self.stack_pop(memory);
                    }
                    Pop {} => {
                        self.stack_pop(memory);
                    }
                    CallAddress {
                        source_address: address,
                    } => {
                        self.push_instruction_pointer(memory);
                        self.set_instruction_pointer(address);
                    }
                    Return {} => {
                        let return_address = self.stack_pop(memory);
                        self.set_instruction_pointer(return_address);
                    }
                    JumpImmediate { immediate: address } => {
                        self.set_instruction_pointer(address);
                    }
                    JumpRegister { register } => {
                        self.set_instruction_pointer(self.registers[register]);
                    }
                    JumpImmediateIfEqual {
                        comparison,
                        immediate: address,
                    } => match self.registers[comparison] {
                        0 => self.set_instruction_pointer(address),
                        _ => self.advance_instruction_pointer(Direction::Forwards),
                    },
                    JumpImmediateIfGreaterThan {
                        comparison,
                        immediate: address,
                    } => match self.registers[comparison] {
                        1 => self.set_instruction_pointer(address),
                        _ => self.advance_instruction_pointer(Direction::Forwards),
                    },
                    JumpImmediateIfLessThan {
                        comparison,
                        immediate: address,
                    } => match self.registers[comparison] {
                        Word::MAX => self.set_instruction_pointer(address),
                        _ => self.advance_instruction_pointer(Direction::Forwards),
                    },
                    JumpImmediateIfGreaterThanOrEqual {
                        comparison,
                        immediate: address,
                    } => match self.registers[comparison] {
                        1 | 0 => self.set_instruction_pointer(address),
                        _ => self.advance_instruction_pointer(Direction::Forwards),
                    },
                    JumpImmediateIfLessThanOrEqual {
                        comparison,
                        immediate: address,
                    } => match self.registers[comparison] {
                        Word::MAX | 0 => self.set_instruction_pointer(address),
                        _ => self.advance_instruction_pointer(Direction::Forwards),
                    },
                    JumpImmediateIfZero { immediate: address } => match self.get_flag(Flag::Zero) {
                        true => self.set_instruction_pointer(address),
                        false => self.advance_instruction_pointer(Direction::Forwards),
                    },
                    JumpImmediateIfNotZero { immediate: address } => {
                        match self.get_flag(Flag::Zero) {
                            false => self.set_instruction_pointer(address),
                            true => self.advance_instruction_pointer(Direction::Forwards),
                        }
                    }
                    JumpImmediateIfCarry { immediate: address } => match self.get_flag(Flag::Carry)
                    {
                        true => self.set_instruction_pointer(address),
                        false => self.advance_instruction_pointer(Direction::Forwards),
                    },
                    JumpImmediateIfNotCarry { immediate: address } => {
                        match self.get_flag(Flag::Carry) {
                            false => self.set_instruction_pointer(address),
                            true => self.advance_instruction_pointer(Direction::Forwards),
                        }
                    }
                    JumpImmediateIfDivideByZero { immediate: address } => {
                        match self.get_flag(Flag::DivideByZero) {
                            true => self.set_instruction_pointer(address),
                            false => self.advance_instruction_pointer(Direction::Forwards),
                        }
                    }
                    JumpImmediateIfNotDivideByZero { immediate: address } => {
                        match self.get_flag(Flag::DivideByZero) {
                            false => self.set_instruction_pointer(address),
                            true => self.advance_instruction_pointer(Direction::Forwards),
                        }
                    }
                    JumpRegisterIfEqual {
                        pointer,
                        comparison,
                    } => match self.registers[comparison] {
                        0 => self.set_instruction_pointer(self.registers[pointer]),
                        _ => self.advance_instruction_pointer(Direction::Forwards),
                    },
                    JumpRegisterIfGreaterThan {
                        pointer,
                        comparison,
                    } => match self.registers[comparison] {
                        1 => self.set_instruction_pointer(self.registers[pointer]),
                        _ => self.advance_instruction_pointer(Direction::Forwards),
                    },
                    JumpRegisterIfLessThan {
                        pointer,
                        comparison,
                    } => match self.registers[comparison] {
                        Word::MAX => self.set_instruction_pointer(self.registers[pointer]),
                        _ => self.advance_instruction_pointer(Direction::Forwards),
                    },
                    JumpRegisterIfGreaterThanOrEqual {
                        pointer,
                        comparison,
                    } => match self.registers[comparison] {
                        1 | 0 => self.set_instruction_pointer(self.registers[pointer]),
                        _ => self.advance_instruction_pointer(Direction::Forwards),
                    },
                    JumpRegisterIfLessThanOrEqual {
                        pointer,
                        comparison,
                    } => match self.registers[comparison] {
                        Word::MAX | 0 => self.set_instruction_pointer(self.registers[pointer]),
                        _ => self.advance_instruction_pointer(Direction::Forwards),
                    },
                    JumpRegisterIfZero { pointer } => match self.get_flag(Flag::Zero) {
                        true => self.set_instruction_pointer(self.registers[pointer]),
                        false => self.advance_instruction_pointer(Direction::Forwards),
                    },
                    JumpRegisterIfNotZero { pointer } => match self.get_flag(Flag::Zero) {
                        false => self.set_instruction_pointer(self.registers[pointer]),
                        true => self.advance_instruction_pointer(Direction::Forwards),
                    },
                    JumpRegisterIfCarry { pointer } => match self.get_flag(Flag::Carry) {
                        true => self.set_instruction_pointer(self.registers[pointer]),
                        false => self.advance_instruction_pointer(Direction::Forwards),
                    },
                    JumpRegisterIfNotCarry { pointer } => match self.get_flag(Flag::Carry) {
                        false => self.set_instruction_pointer(self.registers[pointer]),
                        true => self.advance_instruction_pointer(Direction::Forwards),
                    },
                    JumpRegisterIfDivideByZero { pointer } => {
                        match self.get_flag(Flag::DivideByZero) {
                            true => self.set_instruction_pointer(self.registers[pointer]),
                            false => self.advance_instruction_pointer(Direction::Forwards),
                        }
                    }
                    JumpRegisterIfNotDivideByZero { pointer } => {
                        match self.get_flag(Flag::DivideByZero) {
                            false => self.set_instruction_pointer(self.registers[pointer]),
                            true => self.advance_instruction_pointer(Direction::Forwards),
                        }
                    }
                    NoOp {} => {}
                    GetKeyState { target, keycode } => {
                        self.registers[target] = matches!(
                            periphery
                                .keyboard
                                .get_keystate(self.registers[keycode] as _),
                            KeyState::Down
                        )
                        .into();
                        self.set_flag(Flag::Zero, self.registers[target] == 0);
                    }
                    PollTime { high, low } => {
                        let time = periphery.timer.get_ms_since_epoch();
                        self.registers[low] = time as Word;
                        self.registers[high] = (time >> Word::BITS) as Word;
                    }
                    AddWithCarryTargetLhsRhs { target, lhs, rhs } => {
                        let result = self.registers[lhs]
                            .wrapping_add(self.registers[rhs])
                            .wrapping_add(self.get_flag(Flag::Carry).into());
                        let overflow_happened = (self.registers[lhs] as u64
                            + self.registers[rhs] as u64
                            + self.get_flag(Flag::Carry) as u64)
                            > Word::MAX as u64;
                        self.registers[target] = result;
                        self.set_flag(Flag::Zero, self.registers[target] == 0);
                        self.set_flag(Flag::Carry, overflow_happened);
                    }
                    CallRegister { register } => {
                        self.push_instruction_pointer(memory);
                        self.set_instruction_pointer(self.registers[register]);
                    }
                    CallPointer { pointer } => {
                        self.push_instruction_pointer(memory);
                        self.set_instruction_pointer(memory.read_data(self.registers[pointer]));
                    }
                    SwapFramebuffers {} => periphery.display.swap(),
                    InvisibleFramebufferAddress { target } => {
                        self.registers[target] = periphery.display.invisible_framebuffer_address()
                    }
                    PollCycleCountHighLow { high, low } => {
                        self.registers[low] = self.cycle_count as Word;
                        self.registers[high] = (self.cycle_count >> Word::BITS) as Word;
                    }
                    DumpRegisters {} => {
                        let data: Vec<_> = self
                            .registers
                            .0
                            .iter()
                            .flat_map(|word| word.to_be_bytes())
                            .collect();
                        if let Err(error) = dumper::dump("registers", &data) {
                            eprintln!("Error dumping registers: {}", error);
                        }
                    }
                    DumpMemory {} => {
                        if let Err(error) = dumper::dump("memory", memory.data()) {
                            eprintln!("Error dumping memory: {}", error);
                        }
                    }
                    AssertRegisterRegister { expected, actual } => {
                        debug_assert_eq!(self.registers[expected], self.registers[actual]);
                    }
                    AssertRegisterImmediate { actual, immediate } => {
                        debug_assert_eq!(self.registers[actual], immediate);
                    }
                    AssertPointerImmediate { pointer, immediate } => {
                        debug_assert_eq!(memory.read_data(self.registers[pointer]), immediate);
                    }
                    DebugBreak {} => panic!(),
                    PrintRegister { register } => {
                        eprintln!(
                            "value of register {:#x}: {:#x} ({})",
                            register.0, self.registers[register], self.registers[register]
                        );
                    }
                    BoolCompareEquals { target, lhs, rhs } => {
                        self.registers[target] = if self.registers[lhs] == self.registers[rhs] {
                            1
                        } else {
                            0
                        };
                    }
                    BoolCompareNotEquals { target, lhs, rhs } => {
                        self.registers[target] = if self.registers[lhs] == self.registers[rhs] {
                            0
                        } else {
                            1
                        };
                    }
                    BoolCompareGreater { target, lhs, rhs } => {
                        self.registers[target] = if self.registers[lhs] > self.registers[rhs] {
                            1
                        } else {
                            0
                        };
                    }
                    BoolCompareGreaterOrEquals { target, lhs, rhs } => {
                        self.registers[target] = if self.registers[lhs] >= self.registers[rhs] {
                            1
                        } else {
                            0
                        };
                    }
                    BoolCompareLess { target, lhs, rhs } => {
                        self.registers[target] = if self.registers[lhs] < self.registers[rhs] {
                            1
                        } else {
                            0
                        };
                    }
                    BoolCompareLessOrEquals { target, lhs, rhs } => {
                        self.registers[target] = if self.registers[lhs] <= self.registers[rhs] {
                            1
                        } else {
                            0
                        };
                    }
                }
                self.increase_cycle_count(opcode.get_num_cycles().into());

                if opcode.should_increment_instruction_pointer() {
                    self.advance_instruction_pointer(Direction::Forwards);
                }
            }
            Err(err) => {
                eprintln!("Error making tick: {}", err);
                return ExecutionResult::Error;
            }
        }
        match halted {
            true => ExecutionResult::Halted,
            false => ExecutionResult::Normal,
        }
    }

    fn push_instruction_pointer(&mut self, memory: &mut Memory) {
        self.stack_push(
            memory,
            self.get_instruction_pointer() + Instruction::SIZE as Address,
        );
    }
}
