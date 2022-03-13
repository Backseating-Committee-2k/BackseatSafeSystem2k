use crate::{opcodes::Opcode, Address, Instruction, Size, Word};

pub struct Memory {
    data: Vec<Word>,
}

impl Memory {
    pub const SIZE: usize = 16 * 1024 * 1024;

    pub fn new() -> Self {
        Self {
            data: vec![0; Self::SIZE],
        }
    }

    fn address_to_word_index(address: Address) -> usize {
        debug_assert!(address as usize % Address::SIZE == 0);
        address as usize / Address::SIZE
    }

    pub fn read_opcode(
        &self,
        address: Address,
    ) -> Result<Opcode, <Opcode as TryFrom<Instruction>>::Error> {
        debug_assert!(address as usize % Instruction::SIZE == 0);
        let word_index = Self::address_to_word_index(address);
        let slice = &self.data[word_index..][..Instruction::SIZE / Word::SIZE];
        let mut instruction = 0;
        for &word in slice {
            instruction = (instruction << (8 * Word::SIZE)) | word as Instruction;
        }
        instruction.try_into()
    }

    pub fn read_data(&self, address: Address) -> Word {
        self.data[Self::address_to_word_index(address)]
    }

    pub fn write_opcode(&mut self, address: Address, opcode: Opcode) {
        debug_assert!(address as usize % Instruction::SIZE == 0);
        let mut instruction = opcode.as_instruction();
        let word_index = Self::address_to_word_index(address);
        let bit_mask = Word::MAX as Instruction;
        for index in (word_index..word_index + Instruction::SIZE / Word::SIZE).rev() {
            self.data[index] = (instruction & bit_mask) as Word;
            instruction >>= 8 * Word::SIZE;
        }
    }

    pub fn write_data(&mut self, address: Address, data: Word) {
        self.data[Self::address_to_word_index(address)] = data;
    }
}

impl<Index> std::ops::Index<Index> for Memory
where
    Index: std::slice::SliceIndex<[Address]>,
{
    type Output = Index::Output;

    fn index(&self, index: Index) -> &Self::Output {
        &self.data[index]
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
