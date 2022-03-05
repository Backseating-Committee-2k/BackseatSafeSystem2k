#![allow(non_upper_case_globals)]

use std::ops::{Index, IndexMut};

use crate::terminal;
use crate::{memory::Memory, Address, Instruction, Word};
use crate::{static_assert, AsHalfWords, AsWords};
use crate::{Register, Size};
use bitflags::bitflags;

const _: () = static_assert(Processor::ENTRY_POINT as usize % Instruction::SIZE == 0);

pub enum Direction {
    Forwards,
    Backwards,
}

bitflags! {
    pub struct Flag: Word {
        const Zero = 0b1 << 0;
        const Carry = 0b1 << 1;
        const DivideByZero = 0b1 << 2;
    }
}

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
}

impl Processor {
    pub const NUM_REGISTERS: usize = 256;
    pub const FLAGS: Register = Register((Self::NUM_REGISTERS - 3) as _);
    pub const INSTRUCTION_POINTER: Register = Register((Self::NUM_REGISTERS - 2) as _);
    pub const STACK_POINTER: Register = Register((Self::NUM_REGISTERS - 1) as _);
    pub const STACK_START: Address =
        (terminal::WIDTH * terminal::HEIGHT + 2) as Address * Word::SIZE as Address;
    pub const STACK_SIZE: usize = 512 * 1024;
    pub const ENTRY_POINT: Address = Self::STACK_START + Self::STACK_SIZE as Address; // gonna change!

    pub fn new() -> Self {
        let mut result = Self {
            registers: Registers([0; Self::NUM_REGISTERS]),
        };
        result.registers[Self::INSTRUCTION_POINTER] = Self::ENTRY_POINT;
        result.registers[Self::STACK_POINTER] = Self::STACK_START;
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
        debug_assert!(
            (Self::STACK_START..=Self::STACK_START + Self::STACK_SIZE as Address)
                .contains(&address)
        );
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

    fn set_instruction_pointer(&mut self, address: Address) {
        self.registers[Self::INSTRUCTION_POINTER] = address;
    }

    fn advance_instruction_pointer(&mut self, direction: Direction) {
        match direction {
            Direction::Forwards => self.set_instruction_pointer(
                self.registers[Self::INSTRUCTION_POINTER] + Instruction::SIZE as Address,
            ),
            Direction::Backwards => self.set_instruction_pointer(
                self.registers[Self::INSTRUCTION_POINTER]
                    .saturating_sub(Instruction::SIZE as Address),
            ),
        }
    }

    pub fn make_tick(&mut self, memory: &mut Memory) {
        let instruction = memory.read_instruction(self.registers[Self::INSTRUCTION_POINTER]);
        let opcode = instruction.as_words().0.as_half_words().0;
        let register_values = &instruction.to_be_bytes()[2..];
        let mut registers = [Register(0); 6];
        for (i, register) in registers.iter_mut().enumerate() {
            *register = Register(register_values[i]);
        }
        let constant = instruction.as_words().1;
        let address = constant;
        match opcode {
            0x0000 => self.registers[registers[0]] = constant,
            0x0001 => self.registers[registers[0]] = memory.read_data(address),
            0x0002 => self.registers[registers[0]] = self.registers[registers[1]],
            0x0003 => memory.write_data(address, self.registers[registers[0]]),
            0x0004 => self.registers[registers[0]] = memory.read_data(self.registers[registers[1]]),
            0x0005 => memory.write_data(self.registers[registers[0]], self.registers[registers[1]]),
            0x0006 => return,
            0x0007 => {
                let lhs = self.registers[registers[1]];
                let rhs = self.registers[registers[2]];
                let target = &mut self.registers[registers[0]];
                let (result, did_overflow) = lhs.overflowing_add(rhs);
                *target = result;
                self.set_flag(Flag::Zero, result == 0);
                self.set_flag(Flag::Carry, did_overflow);
            }
            0x0008 => {
                let lhs = self.registers[registers[1]];
                let rhs = self.registers[registers[2]];
                let target = &mut self.registers[registers[0]];
                let (result, did_overflow) = lhs.overflowing_sub(rhs);
                *target = result;
                self.set_flag(Flag::Zero, result == 0);
                self.set_flag(Flag::Carry, did_overflow);
            }
            0x0009 => {
                let lhs = self.registers[registers[1]];
                let rhs = self.registers[registers[2]];
                let carry_flag_set = self.get_flag(Flag::Carry);
                let target = &mut self.registers[registers[0]];
                let (result, did_overflow) = lhs.overflowing_sub(rhs);
                let (result, did_overflow_after_subtracting_carry) =
                    result.overflowing_sub(carry_flag_set as _);
                *target = result;
                self.set_flag(Flag::Zero, result == 0);
                self.set_flag(
                    Flag::Carry,
                    did_overflow || did_overflow_after_subtracting_carry,
                );
            }
            0x000A => {
                let lhs = self.registers[registers[2]];
                let rhs = self.registers[registers[3]];
                let result = lhs as u64 * rhs as u64;
                let high_result = (result >> 32) as u32;
                let low_result = result as u32;
                self.registers[registers[0]] = high_result;
                self.registers[registers[1]] = low_result;
                self.set_flag(Flag::Zero, low_result == 0);
                self.set_flag(Flag::Carry, high_result > 0);
            }
            0x000B => {
                let lhs = self.registers[registers[2]];
                let rhs = self.registers[registers[3]];
                if rhs == 0 {
                    self.registers[registers[0]] = 0;
                    self.registers[registers[1]] = lhs;
                    self.set_flag(Flag::Zero, true);
                    self.set_flag(Flag::DivideByZero, true);
                } else {
                    let (quotient, remainder) = (lhs / rhs, lhs % rhs);
                    self.registers[registers[0]] = quotient;
                    self.registers[registers[1]] = remainder;
                    self.set_flag(Flag::Zero, quotient == 0);
                    self.set_flag(Flag::DivideByZero, false);
                }
            }
            0x000C => {
                let lhs = self.registers[registers[1]];
                let rhs = self.registers[registers[2]];
                let result = lhs & rhs;
                self.registers[registers[0]] = result;
                self.set_flag(Flag::Zero, result == 0);
            }
            0x000D => {
                let lhs = self.registers[registers[1]];
                let rhs = self.registers[registers[2]];
                let result = lhs | rhs;
                self.registers[registers[0]] = result;
                self.set_flag(Flag::Zero, result == 0);
            }
            0x000E => {
                let lhs = self.registers[registers[1]];
                let rhs = self.registers[registers[2]];
                let result = lhs ^ rhs;
                self.registers[registers[0]] = result;
                self.set_flag(Flag::Zero, result == 0);
            }
            0x000F => {
                let source = self.registers[registers[1]];
                let result = !source;
                self.registers[registers[0]] = result;
                self.set_flag(Flag::Zero, result == 0);
            }
            0x0010 => {
                let lhs = self.registers[registers[1]];
                let rhs = self.registers[registers[2]];
                if rhs > Word::BITS {
                    self.registers[registers[0]] = 0;
                    self.set_flag(Flag::Zero, true);
                    self.set_flag(Flag::Carry, lhs > 0);
                } else {
                    let result = lhs << rhs;
                    self.registers[registers[0]] = result;
                    self.set_flag(Flag::Zero, result == 0);
                    self.set_flag(Flag::Carry, rhs > lhs.leading_zeros());
                }
            }
            0x0011 => {
                let lhs = self.registers[registers[1]];
                let rhs = self.registers[registers[2]];
                if rhs > Word::BITS {
                    self.registers[registers[0]] = 0;
                    self.set_flag(Flag::Zero, true);
                    self.set_flag(Flag::Carry, lhs > 0);
                } else {
                    let result = lhs >> rhs;
                    self.registers[registers[0]] = result;
                    self.set_flag(Flag::Zero, result == 0);
                    self.set_flag(Flag::Carry, rhs > lhs.trailing_zeros());
                }
            }
            0x0012 => {
                let lhs = self.registers[registers[1]];
                let (result, carry) = lhs.overflowing_add(constant);
                self.registers[registers[0]] = result;
                self.set_flag(Flag::Zero, result == 0);
                self.set_flag(Flag::Carry, carry);
            }
            0x0013 => {
                let lhs = self.registers[registers[1]];
                let result = lhs.wrapping_sub(constant);
                self.registers[registers[0]] = result;
                self.set_flag(Flag::Zero, result == 0);
                self.set_flag(Flag::Carry, constant > lhs);
            }
            0x0014 => {
                let lhs = self.registers[registers[1]];
                let rhs = self.registers[registers[2]];
                let result = match lhs.cmp(&rhs) {
                    std::cmp::Ordering::Less => Word::MAX,
                    std::cmp::Ordering::Equal => 0,
                    std::cmp::Ordering::Greater => 1,
                };
                self.registers[registers[0]] = result;
                self.set_flag(Flag::Zero, result == 0);
            }
            0x0015 => {
                let value = self.registers[registers[0]];
                memory.write_data(self.get_stack_pointer(), value);
                self.advance_stack_pointer(Word::SIZE, Direction::Forwards);
            }
            0x0016 => {
                self.advance_stack_pointer(Word::SIZE, Direction::Backwards);
                self.registers[registers[0]] = memory.read_data(self.get_stack_pointer());
            }
            _ => panic!("Unknown opcode!"),
        }
        self.advance_instruction_pointer(Direction::Forwards);
    }
}
