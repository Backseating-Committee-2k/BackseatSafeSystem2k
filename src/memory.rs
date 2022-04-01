use crate::{opcodes::Opcode, Address, Instruction, Size, Word};

pub struct Memory {
    data: Vec<u8>,
}

impl Memory {
    pub const SIZE: usize = 16 * 1024 * 1024;

    pub fn new() -> Self {
        Self {
            data: vec![0; Self::SIZE],
        }
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn read_opcode(
        &self,
        address: Address,
    ) -> Result<Opcode, <Opcode as TryFrom<Instruction>>::Error> {
        debug_assert!(address as usize % Instruction::SIZE == 0);
        let slice = &self.data[address as usize..][..Instruction::SIZE];
        let instruction = Instruction::from_be_bytes(slice.try_into().unwrap());
        instruction.try_into()
    }

    pub fn read_data(&self, address: Address) -> Word {
        debug_assert!(address as usize % Word::SIZE == 0);
        let slice = &self.data[address as usize..][..Word::SIZE];
        Word::from_be_bytes(slice.try_into().unwrap())
    }

    pub fn write_opcode(&mut self, address: Address, opcode: Opcode) {
        debug_assert!(address as usize % Instruction::SIZE == 0);
        let instruction = opcode.as_instruction();

        self.data[address as usize..][..Instruction::SIZE]
            .copy_from_slice(&instruction.to_be_bytes());
    }

    pub fn write_data(&mut self, address: Address, data: Word) {
        debug_assert!(address as usize % Word::SIZE == 0);
        self.data[address as usize..][..Word::SIZE].copy_from_slice(&data.to_be_bytes());
    }
}

#[cfg(test)]
mod tests {
    use crate::Register;

    use super::*;

    #[test]
    fn write_instruction_read_back() {
        let mut memory = Memory::new();
        let address = 0x0;
        let opcode = Opcode::MoveRegisterImmediate {
            register: Register(0),
            immediate: 42,
        };
        memory.write_opcode(address, opcode);
        assert_eq!(memory.read_opcode(address), Ok(opcode));
    }

    #[test]
    fn write_data_read_back() {
        let mut memory = Memory::new();
        let data = 0xFFFFFFFF;
        let address = 0x0;
        memory.write_data(address, data);
        assert_eq!(memory.read_data(address), data);
    }

    #[test]
    fn fill_memory_with_instructions_read_back() {
        let mut memory = Memory::new();

        // fill memory
        let opcode = Opcode::MoveRegisterImmediate {
            register: Register(0),
            immediate: 42,
        };
        for address in (0..Memory::SIZE).step_by(Instruction::SIZE) {
            memory.write_opcode(address as Address, opcode);
        }

        for address in (0..Memory::SIZE).step_by(Instruction::SIZE) {
            assert_eq!(memory.read_opcode(address as Address), Ok(opcode));
        }
    }

    #[test]
    fn fill_memory_with_data_read_back() {
        let mut memory = Memory::new();

        // fill memory
        let mut data = 0x0;
        for address in (0..Memory::SIZE).step_by(Word::SIZE) {
            memory.write_data(address as Address, data);
            data = data.wrapping_add(1);
        }

        // read back memory
        data = 0x0;
        for address in (0..Memory::SIZE).step_by(Word::SIZE) {
            assert_eq!(memory.read_data(address as Address), data);
            data = data.wrapping_add(1);
        }
    }
}
