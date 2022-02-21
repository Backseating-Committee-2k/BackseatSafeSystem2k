use crate::{memory::Memory, processor::Processor, terminal};
use raylib::prelude::*;

pub struct Machine {
    pub memory: Memory,
    pub processor: Processor,
}

impl Machine {
    pub fn new() -> Self {
        Self {
            memory: Memory::new(),
            processor: Processor::new(),
        }
    }

    pub fn render(&mut self, draw_handle: &mut RaylibDrawHandle, font: &Font) {
        terminal::render(&mut self.memory, draw_handle, Vector2::zero(), font, 20.0);
    }

    pub fn make_tick(&mut self) {
        self.processor.make_tick(&mut self.memory);
    }
}

#[cfg(test)]
mod tests {
    use crate::{Address, Instruction, Word};

    use super::*;

    #[test]
    fn make_tick_increases_instruction_pointer() {
        use crate::Size;
        let mut machine = Machine::new();
        assert_eq!(
            machine.processor.registers[Processor::INSTRUCTION_POINTER],
            Processor::ENTRY_POINT
        );
        machine.processor.make_tick(&mut machine.memory);
        assert_eq!(
            machine.processor.registers[Processor::INSTRUCTION_POINTER],
            Processor::ENTRY_POINT + Instruction::SIZE as u32
        );
    }

    fn create_machine_with_data_at(address: Address, data: Word) -> Machine {
        let mut machine = Machine::new();
        machine.memory.write_data(address, data);
        machine
    }

    fn execute_instruction_with_machine(mut machine: Machine, instruction: Instruction) -> Machine {
        use crate::Size;
        machine
            .memory
            .write_instruction(Processor::ENTRY_POINT, instruction);
        machine.processor.make_tick(&mut machine.memory);
        assert_eq!(
            machine.processor.registers[Processor::INSTRUCTION_POINTER],
            Processor::ENTRY_POINT + Instruction::SIZE as u32
        );
        machine
    }

    fn execute_instruction(instruction: Instruction) -> Machine {
        execute_instruction_with_machine(Machine::new(), instruction)
    }

    #[test]
    fn move_constant_into_register() {
        // 0000 RR__ CCCC CCCC => move the value C into register R
        let machine = execute_instruction(0x0000_0A00_ABCD_1234);
        assert_eq!(machine.processor.registers[10], 0xABCD_1234);
    }

    #[test]
    fn move_from_address_into_register() {
        // `0001 RR__ AAAA AAAA` | move the value at address A into register R
        let machine = create_machine_with_data_at(0xF0, 0xABCD_1234);
        let machine = execute_instruction_with_machine(machine, 0x0001_0A00_0000_00F0);
        assert_eq!(machine.processor.registers[10], 0xABCD_1234);
    }

    #[test]
    fn move_from_one_register_to_another() {
        // `0002 TTSS ____ ____` | move the contents of register S into register T
        let mut machine = Machine::new();
        machine.processor.registers[5] = 0xCAFE;
        let machine = execute_instruction_with_machine(machine, 0x0002_0A_05_0000_0000);
        assert_eq!(machine.processor.registers[10], 0xCAFE);
    }

    #[test]
    fn move_from_register_into_memory() {
        // `0003 RR__ AAAA AAAA` | move the contents of register R into memory at address A
        let mut machine = Machine::new();
        machine.processor.registers[5] = 0xC0FFEE;
        let machine = execute_instruction_with_machine(machine, 0x0003_0500_0000_00F0);
        assert_eq!(machine.memory.read_data(0xF0), 0xC0FFEE);
    }

    #[test]
    fn move_from_memory_addressed_by_register_into_another_register() {
        // `0004 TTPP ____ ____` | move the contents addressed by the value of register P into register T
        let mut machine = create_machine_with_data_at(0xF0, 0xC0FFEE);
        machine.processor.registers[5] = 0xF0;
        let machine = execute_instruction_with_machine(machine, 0x0004_0A05_0000_0000);
        assert_eq!(machine.processor.registers[10], 0xC0FFEE);
    }

    #[test]
    fn move_from_memory_addressed_by_register_into_same_register() {
        // `0004 TTPP ____ ____` | move the contents addressed by the value of register P into register T
        let mut machine = create_machine_with_data_at(0xF0, 0xC0FFEE);
        machine.processor.registers[5] = 0xF0;
        let machine = execute_instruction_with_machine(machine, 0x0004_0505_0000_0000);
        assert_eq!(machine.processor.registers[5], 0xC0FFEE);
    }

    #[test]
    fn move_from_register_into_memory_addressed_by_another_register() {
        // `0005 PPSS ____ ____` | move the contents of register S into memory at address specified by register P
        let mut machine = Machine::new();
        machine.processor.registers[5] = 0xC0FFEE;
        machine.processor.registers[10] = 0xF0;
        let machine = execute_instruction_with_machine(machine, 0x0005_0A05_0000_0000);
        assert_eq!(machine.memory.read_data(0xF0), 0xC0FFEE);
    }

    #[test]
    fn move_from_register_into_memory_addressed_by_same_register() {
        // `0005 PPSS ____ ____` | move the contents of register S into memory at address specified by register P
        let mut machine = Machine::new();
        machine.processor.registers[5] = 0xF0;
        let machine = execute_instruction_with_machine(machine, 0x0005_0505_0000_0000);
        assert_eq!(machine.memory.read_data(0xF0), 0xF0);
    }
}
