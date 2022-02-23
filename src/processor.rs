use std::mem::size_of;

use crate::{memory::Memory, Address, Instruction, Word};
use crate::{AsHalfWords, AsWords};

pub struct Processor {
    pub registers: [Word; Self::NUM_REGISTERS],
}

impl Processor {
    pub const NUM_REGISTERS: usize = 256;
    pub const FLAGS: usize = Self::NUM_REGISTERS - 3;
    pub const INSTRUCTION_POINTER: usize = Self::NUM_REGISTERS - 2;
    pub const STACK_POINTER: usize = Self::NUM_REGISTERS - 1;
    pub const ENTRY_POINT: Address = 0x1F48; // gonna change!

    pub fn new() -> Self {
        let mut result = Self {
            registers: [0; Self::NUM_REGISTERS],
        };
        result.registers[Self::INSTRUCTION_POINTER] = Self::ENTRY_POINT;
        result
    }

    pub fn make_tick(&mut self, memory: &mut Memory) {
        let instruction = memory.read_instruction(self.registers[Self::INSTRUCTION_POINTER]);
        let opcode = instruction.as_words().0.as_half_words().0;
        let registers = &instruction.to_be_bytes()[2..];
        let constant = instruction.as_words().1;
        let address = constant;
        match opcode {
            0x0000 => self.registers[registers[0] as usize] = constant,
            0x0001 => self.registers[registers[0] as usize] = memory.read_data(address),
            0x0002 => self.registers[registers[0] as usize] = self.registers[registers[1] as usize],
            0x0003 => memory.write_data(address, self.registers[registers[0] as usize]),
            0x0004 => {
                self.registers[registers[0] as usize] =
                    memory.read_data(self.registers[registers[1] as usize])
            }
            0x0005 => memory.write_data(
                self.registers[registers[0] as usize],
                self.registers[registers[1] as usize],
            ),
            _ => panic!("Unknown opcode!"),
        }
        self.registers[Self::INSTRUCTION_POINTER] += size_of::<Instruction>() as Address;
    }
}
