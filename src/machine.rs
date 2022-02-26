use crate::{memory::Memory, processor::Processor, terminal, Instruction, Size, OPCODE_LENGTH};
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

    #[must_use = "Am I a joke to you?"]
    pub fn is_halted(&self) -> bool {
        let instruction = self.read_instruction_at_instruction_pointer();
        let bitmask = !(Instruction::MAX >> OPCODE_LENGTH);
        (instruction & bitmask) >> (Instruction::SIZE * 8 - OPCODE_LENGTH) == 0x0006
    }

    fn read_instruction_at_instruction_pointer(&self) -> Instruction {
        self.memory
            .read_instruction(self.processor.registers[Processor::INSTRUCTION_POINTER])
    }
}

#[cfg(test)]
mod tests {
    use crate::opcodes::*;
    use crate::processor::Flag;
    use crate::{Address, Instruction, Size, Word};

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

    fn create_machine_with_instructions(instructions: &[Instruction]) -> Machine {
        let mut machine = Machine::new();
        for (&instruction, address) in instructions
            .iter()
            .zip((Processor::ENTRY_POINT..).step_by(Instruction::SIZE))
        {
            machine.memory.write_instruction(address, instruction);
        }
        machine
    }

    fn execute_instruction_with_machine(mut machine: Machine, instruction: Instruction) -> Machine {
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
        let register = 0x0A.into();
        let value = 0xABCD_1234;
        let machine = execute_instruction(move_register_immediate(register, value));
        assert_eq!(machine.processor.registers[register], value);
    }

    #[test]
    fn move_from_address_into_register() {
        // `0001 RR__ AAAA AAAA` | move the value at address A into register R
        let address = 0xF0;
        let data = 0xABCD_1234;
        let register = 0x0A.into();
        let machine = create_machine_with_data_at(address, data);
        let machine =
            execute_instruction_with_machine(machine, move_register_address(register, address));
        assert_eq!(machine.processor.registers[register], data);
    }

    #[test]
    fn move_from_one_register_to_another() {
        // `0002 TTSS ____ ____` | move the contents of register S into register T
        let mut machine = Machine::new();
        let source = 0x5.into();
        let target = 0x0A.into();
        let data = 0xCAFE;
        machine.processor.registers[source] = data;
        let machine = execute_instruction_with_machine(machine, move_target_source(target, source));
        assert_eq!(machine.processor.registers[target], data);
    }

    #[test]
    fn move_from_register_into_memory() {
        // `0003 RR__ AAAA AAAA` | move the contents of register R into memory at address A
        let mut machine = Machine::new();
        let register = 0x5.into();
        let data = 0xC0FFEE;
        let address = 0xF0;
        machine.processor.registers[register] = data;
        let machine =
            execute_instruction_with_machine(machine, move_address_register(address, register));
        assert_eq!(machine.memory.read_data(address), data);
    }

    #[test]
    fn move_from_memory_addressed_by_register_into_another_register() {
        // `0004 TTPP ____ ____` | move the contents addressed by the value of register P into register T
        let address = 0xF0;
        let data = 0xC0FFEE;
        let target = 0x0A.into();
        let pointer = 0x05.into();
        let mut machine = create_machine_with_data_at(address, data);
        machine.processor.registers[pointer] = address;
        let machine =
            execute_instruction_with_machine(machine, move_target_pointer(target, pointer));
        assert_eq!(machine.processor.registers[target], data);
    }

    #[test]
    fn move_from_memory_addressed_by_register_into_same_register() {
        // `0004 TTPP ____ ____` | move the contents addressed by the value of register P into register T
        let address = 0xF0;
        let data = 0xC0FFEE;
        let register = 0x05.into();
        let mut machine = create_machine_with_data_at(address, data);
        machine.processor.registers[register] = address;
        let machine =
            execute_instruction_with_machine(machine, move_target_pointer(register, register));
        assert_eq!(machine.processor.registers[register], data);
    }

    #[test]
    fn move_from_register_into_memory_addressed_by_another_register() {
        // `0005 PPSS ____ ____` | move the contents of register S into memory at address specified by register P
        let data = 0xC0FFEE;
        let address = 0xF0;
        let pointer = 0x0A.into();
        let source = 0x05.into();
        let mut machine = Machine::new();
        machine.processor.registers[source] = data;
        machine.processor.registers[pointer] = address;
        let machine =
            execute_instruction_with_machine(machine, move_pointer_source(pointer, source));
        assert_eq!(machine.memory.read_data(address), data);
    }

    #[test]
    fn move_from_register_into_memory_addressed_by_same_register() {
        // `0005 PPSS ____ ____` | move the contents of register S into memory at address specified by register P
        let address = 0xF0;
        let register = 0x05.into();
        let mut machine = Machine::new();
        machine.processor.registers[register] = address;
        let machine =
            execute_instruction_with_machine(machine, move_pointer_source(register, register));
        assert_eq!(machine.memory.read_data(address), address);
    }

    #[test]
    fn halt_and_catch_fire_prevents_further_instructions() {
        // `0006 ____ ____ ____` | halt and catch fire
        let register = 0x05.into();
        let value = 0x0000_0042;
        let mut machine = create_machine_with_instructions(&[
            halt_and_catch_fire(),
            move_register_immediate(register, value),
        ]);
        for _ in 0..2 {
            machine.make_tick();
        }
        assert_eq!(
            machine.processor.registers[Processor::INSTRUCTION_POINTER],
            Processor::ENTRY_POINT
        );
        assert_eq!(machine.processor.registers[register], 0x0);
    }

    #[test]
    fn add_two_values_with_no_flags_set() {
        // `0007 TTLL RR__ ____` | add the values in registers LL and RR, store the result in TT, set zero and carry flags appropriately
        let lhs_register = 0x42.into();
        let rhs_register = 0x43.into();
        let target_register = 0x0A.into();
        let lhs = 10;
        let rhs = 12;
        let expected = lhs + rhs;
        let mut machine = Machine::new();
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let mut machine = execute_instruction_with_machine(
            machine,
            add_target_lhs_rhs(target_register, lhs_register, rhs_register),
        );
        machine.make_tick();
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(machine.processor.registers[target_register], expected);
        assert_eq!(machine.processor.get_flag(Flag::Zero), false);
        assert_eq!(machine.processor.get_flag(Flag::Carry), false);
    }

    #[test]
    fn add_two_values_with_only_zero_flag_set() {
        // `0007 TTLL RR__ ____` | add the values in registers LL and RR, store the result in TT, set zero and carry flags appropriately
        let lhs_register = 0x42.into();
        let rhs_register = 0x43.into();
        let target_register = 0x0A.into();
        let lhs = 0;
        let rhs = 0;
        let expected = lhs + rhs;
        let mut machine = Machine::new();
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let mut machine = execute_instruction_with_machine(
            machine,
            add_target_lhs_rhs(target_register, lhs_register, rhs_register),
        );
        machine.make_tick();
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(machine.processor.registers[target_register], expected);
        assert_eq!(machine.processor.get_flag(Flag::Zero), true);
        assert_eq!(machine.processor.get_flag(Flag::Carry), false);
    }

    #[test]
    fn add_two_values_with_only_carry_flag_set() {
        // `0007 TTLL RR__ ____` | add the values in registers LL and RR, store the result in TT, set zero and carry flags appropriately
        let lhs_register = 0x42.into();
        let rhs_register = 0x43.into();
        let target_register = 0x0A.into();
        let lhs = Word::MAX;
        let rhs = 5;
        let expected = lhs.wrapping_add(rhs);
        let mut machine = Machine::new();
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let mut machine = execute_instruction_with_machine(
            machine,
            add_target_lhs_rhs(target_register, lhs_register, rhs_register),
        );
        machine.make_tick();
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(machine.processor.registers[target_register], expected);
        assert_eq!(machine.processor.get_flag(Flag::Zero), false);
        assert_eq!(machine.processor.get_flag(Flag::Carry), true);
    }

    #[test]
    fn add_two_values_with_both_zero_and_carry_flags_set() {
        // `0007 TTLL RR__ ____` | add the values in registers LL and RR, store the result in TT, set zero and carry flags appropriately
        let lhs_register = 0x42.into();
        let rhs_register = 0x43.into();
        let target_register = 0x0A.into();
        let lhs = Word::MAX;
        let rhs = 1;
        let expected = lhs.wrapping_add(rhs);
        let mut machine = Machine::new();
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let mut machine = execute_instruction_with_machine(
            machine,
            add_target_lhs_rhs(target_register, lhs_register, rhs_register),
        );
        machine.make_tick();
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(machine.processor.registers[target_register], expected);
        assert_eq!(machine.processor.get_flag(Flag::Zero), true);
        assert_eq!(machine.processor.get_flag(Flag::Carry), true);
    }

    #[test]
    fn subtract_two_values_with_no_flags_set() {
        // `0008 TTLL RR__ ____` | subtract (without carry) the values in registers LL and RR, store the result in TT, set zero and carry flags appropriately
        let lhs_register = 0x42.into();
        let rhs_register = 0x43.into();
        let target_register = 0x0A.into();
        let lhs = 10;
        let rhs = 8;
        let expected = lhs - rhs;
        let mut machine = Machine::new();
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let mut machine = execute_instruction_with_machine(
            machine,
            subtract_target_lhs_rhs(target_register, lhs_register, rhs_register),
        );
        machine.make_tick();
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(machine.processor.registers[target_register], expected);
        assert_eq!(machine.processor.get_flag(Flag::Zero), false);
        assert_eq!(machine.processor.get_flag(Flag::Carry), false);
    }

    #[test]
    fn subtract_two_values_with_only_zero_flag_set() {
        // `0008 TTLL RR__ ____` | subtract (without carry) the values in registers LL and RR, store the result in TT, set zero and carry flags appropriately
        let lhs_register = 0x42.into();
        let rhs_register = 0x43.into();
        let target_register = 0x0A.into();
        let lhs = 10;
        let rhs = 10;
        let expected = lhs - rhs;
        let mut machine = Machine::new();
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let mut machine = execute_instruction_with_machine(
            machine,
            subtract_target_lhs_rhs(target_register, lhs_register, rhs_register),
        );
        machine.make_tick();
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(machine.processor.registers[target_register], expected);
        assert_eq!(machine.processor.get_flag(Flag::Zero), true);
        assert_eq!(machine.processor.get_flag(Flag::Carry), false);
    }

    #[test]
    fn subtract_two_values_with_only_carry_flag_set() {
        // `0008 TTLL RR__ ____` | subtract (without carry) the values in registers LL and RR, store the result in TT, set zero and carry flags appropriately
        let lhs_register = 0x42.into();
        let rhs_register = 0x43.into();
        let target_register = 0x0A.into();
        let lhs: Word = 10;
        let rhs = 12;
        let expected = lhs.wrapping_sub(rhs);
        let mut machine = Machine::new();
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let mut machine = execute_instruction_with_machine(
            machine,
            subtract_target_lhs_rhs(target_register, lhs_register, rhs_register),
        );
        machine.make_tick();
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(machine.processor.registers[target_register], expected);
        assert_eq!(machine.processor.get_flag(Flag::Zero), false);
        assert_eq!(machine.processor.get_flag(Flag::Carry), true);
    }

    #[test]
    fn subtract_two_values_with_carry_with_no_flags_set() {
        // `0009 TTLL RR__ ____` | subtract (with carry) the values in registers LL and RR, store the result in TT, set zero and carry flags appropriately
        let lhs_register = 0x42.into();
        let rhs_register = 0x43.into();
        let target_register = 0x0A.into();
        let lhs: Word = 14;
        let rhs = 12;
        let expected = lhs.wrapping_sub(rhs + 1 /* carry */);
        let mut machine = Machine::new();
        machine.processor.set_flag(Flag::Carry, true);
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let mut machine = execute_instruction_with_machine(
            machine,
            subtract_with_carry_target_lhs_rhs(target_register, lhs_register, rhs_register),
        );
        machine.make_tick();
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(machine.processor.registers[target_register], expected);
        assert_eq!(machine.processor.get_flag(Flag::Zero), false);
        assert_eq!(machine.processor.get_flag(Flag::Carry), false);
    }

    #[test]
    fn subtract_two_values_with_carry_with_zero_flag_set() {
        // `0009 TTLL RR__ ____` | subtract (with carry) the values in registers LL and RR, store the result in TT, set zero and carry flags appropriately
        let lhs_register = 0x42.into();
        let rhs_register = 0x43.into();
        let target_register = 0x0A.into();
        let lhs: Word = 14;
        let rhs = 13;
        let expected = lhs.wrapping_sub(rhs + 1 /* carry */);
        let mut machine = Machine::new();
        machine.processor.set_flag(Flag::Carry, true);
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let mut machine = execute_instruction_with_machine(
            machine,
            subtract_with_carry_target_lhs_rhs(target_register, lhs_register, rhs_register),
        );
        machine.make_tick();
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(machine.processor.registers[target_register], expected);
        assert_eq!(machine.processor.get_flag(Flag::Zero), true);
        assert_eq!(machine.processor.get_flag(Flag::Carry), false);
    }

    #[test]
    fn subtract_two_values_with_carry_with_both_carry_and_zero_flags_set() {
        // `0009 TTLL RR__ ____` | subtract (with carry) the values in registers LL and RR, store the result in TT, set zero and carry flags appropriately
        let lhs_register = 0x42.into();
        let rhs_register = 0x43.into();
        let target_register = 0x0A.into();
        let lhs: Word = 0;
        let rhs = Word::MAX;
        let expected = lhs.wrapping_sub(rhs).wrapping_sub(1);
        let mut machine = Machine::new();
        machine.processor.set_flag(Flag::Carry, true);
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let mut machine = execute_instruction_with_machine(
            machine,
            subtract_with_carry_target_lhs_rhs(target_register, lhs_register, rhs_register),
        );
        machine.make_tick();
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(machine.processor.registers[target_register], expected);
        assert_eq!(machine.processor.get_flag(Flag::Zero), true);
        assert_eq!(machine.processor.get_flag(Flag::Carry), true);
    }

    #[test]
    fn multiply_two_values_without_any_flags_set() {
        // `000A HHTT LLRR ____` | multiply the values in registers LL and RR, store the low part of the result in TT, the high part in HH, set zero and carry flags appropriately
        let lhs_register = 0x42.into();
        let rhs_register = 0x43.into();
        let target_high = 0x09.into();
        let target_low = 0x0A.into();
        let lhs: Word = 3;
        let rhs = 4;
        let expected = lhs * rhs;
        let mut machine = Machine::new();
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let mut machine = execute_instruction_with_machine(
            machine,
            multiply_target_lhs_rhs(target_high, target_low, lhs_register, rhs_register),
        );
        machine.make_tick();
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(machine.processor.registers[target_high], 0);
        assert_eq!(machine.processor.registers[target_low], expected);
        assert_eq!(machine.processor.get_flag(Flag::Zero), false);
        assert_eq!(machine.processor.get_flag(Flag::Carry), false);
    }

    #[test]
    fn multiply_two_values_with_zero_flag_set() {
        // `000A HHTT LLRR ____` | multiply the values in registers LL and RR, store the low part of the result in TT, the high part in HH, set zero and carry flags appropriately
        let lhs_register = 0x42.into();
        let rhs_register = 0x43.into();
        let target_high = 0x09.into();
        let target_low = 0x0A.into();
        let lhs: Word = 3;
        let rhs = 0;
        let expected = lhs * rhs;
        let mut machine = Machine::new();
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let mut machine = execute_instruction_with_machine(
            machine,
            multiply_target_lhs_rhs(target_high, target_low, lhs_register, rhs_register),
        );
        machine.make_tick();
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(machine.processor.registers[target_high], 0);
        assert_eq!(machine.processor.registers[target_low], expected);
        assert_eq!(machine.processor.get_flag(Flag::Zero), true);
        assert_eq!(machine.processor.get_flag(Flag::Carry), false);
    }

    #[test]
    fn multiply_two_values_with_overflow() {
        // `000A HHTT LLRR ____` | multiply the values in registers LL and RR, store the low part of the result in TT, the high part in HH, set zero and carry flags appropriately
        let lhs_register = 0x42.into();
        let rhs_register = 0x43.into();
        let target_high = 0x09.into();
        let target_low = 0x0A.into();
        let lhs: Word = Word::MAX;
        let rhs = 5;
        let result = lhs as u64 * rhs as u64;
        let high_expected = (result >> 32) as u32;
        let low_expected = result as u32;
        let mut machine = Machine::new();
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let mut machine = execute_instruction_with_machine(
            machine,
            multiply_target_lhs_rhs(target_high, target_low, lhs_register, rhs_register),
        );
        machine.make_tick();
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(machine.processor.registers[target_high], high_expected);
        assert_eq!(machine.processor.registers[target_low], low_expected);
        assert_eq!(machine.processor.get_flag(Flag::Zero), false);
        assert_eq!(machine.processor.get_flag(Flag::Carry), true);
    }
}
