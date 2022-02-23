use crate::Size;
use crate::{memory::Memory, Address, Instruction, Word};
use crate::{static_assert, AsHalfWords, AsWords};

pub struct Processor {
    pub registers: [Word; Self::NUM_REGISTERS],
}

const _: () = static_assert(Processor::ENTRY_POINT as usize % Instruction::SIZE == 0);

enum Direction {
    Forwards,
    Backwards,
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
            0x0006 => return,
            _ => panic!("Unknown opcode!"),
        }
        self.advance_instruction_pointer(Direction::Forwards);
    }
}
