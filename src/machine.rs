use crate::{
    display::Render, memory::Memory, opcodes::Opcode, periphery::Periphery, processor::Processor,
    terminal, Instruction,
};
use raylib::prelude::*;

pub struct Machine<Display> {
    pub memory: Memory,
    pub processor: Processor,
    pub display: Display,
    pub periphery: Periphery,
    is_halted: bool,
}

impl<Display: Render> Machine<Display> {
    pub fn new(periphery: Periphery) -> Self {
        let mut memory = Memory::new();
        let display = Display::new(&mut memory);
        Self {
            memory,
            processor: Processor::new(),
            display,
            periphery,
            is_halted: false,
        }
    }

    pub fn render(&mut self, draw_handle: &mut RaylibDrawHandle, font: &Font) {
        self.display.render(&mut self.memory);
        terminal::render(&self.memory, draw_handle, Vector2::zero(), font, 20.0);
    }

    pub fn execute_next_instruction(&mut self) {
        use crate::processor::ExecutionResult::*;
        if let Halted = self
            .processor
            .execute_next_instruction(&mut self.memory, &mut self.periphery)
        {
            self.is_halted = true;
        }
    }

    #[must_use = "Am I a joke to you?"]
    pub fn is_halted(&self) -> bool {
        self.is_halted
    }
}

#[cfg(test)]
mod tests {
    use crate::display::MockDisplay;
    use crate::keyboard::{KeyState, Keyboard};
    use crate::processor::Flag;
    use crate::timer::Timer;
    use crate::{address_constants, Address, Instruction, Size, Word};
    use crate::{
        opcodes::Opcode::{self, *},
        Register,
    };

    use super::*;

    macro_rules! opcodes_to_machine {
        () => {
            Machine::new()
        };
        ($opcodes:expr) => {
            create_machine_with_opcodes($opcodes)
        };
    }

    macro_rules! create_test {
        (
            $test_name:ident,
            $( setup = { $($setup_tokens:tt)+ }, )?
            $( opcodes = $opcodes:expr, )?
            $( registers_pre = [$( $register_pre_value:expr => $register_pre:expr ),+], )?
            $( flags_pre = [ $( $flag_pre_value:expr => $flag_pre:ident ),+ ],)?
            $( memory_pre = [$( $memory_pre_value:expr => $memory_pre_address:expr ),+], )?
            $( registers_post = [$( ($register_post:expr, $register_post_value:expr) ),+], )?
            $( memory_post = [$( ( $memory_post_address:expr, $memory_post_value:expr ) ),+], )?
            $( flags_post = [ $( ( $flag_post:ident, $flag_post_value:expr ) ),+], )?
            $( eq_asserts = [ $( ( $eq_assert_lhs:expr, $eq_assert_rhs:expr ) ),+ ], )?
        ) => {
            #[test]
            fn $test_name() {
                #![allow(clippy::bool_assert_comparison)]
                $(
                    $(
                        $setup_tokens
                    )+
                )?
                let mut machine = opcodes_to_machine!($( $opcodes )?);
                $(
                    $(
                        machine.processor.registers[$register_pre.into()] = $register_pre_value;
                    )+
                )?
                $(
                    $(
                        machine.processor.set_flag(Flag::$flag_pre, $flag_pre_value);
                    )+
                )?
                $(
                    $(
                        machine.memory.write_data($memory_pre_address, $memory_pre_value);
                    )+
                )?
                $(
                    for _ in 0..$opcodes.len() {
                        machine.execute_next_instruction();
                    }
                )?
                $(
                    $(
                        assert_eq!(machine.processor.registers[$register_post], $register_post_value);
                    )+
                )?
                $(
                    $(
                        assert_eq!(machine.memory.read_data($memory_post_address), $memory_post_value);
                    )+
                )?
                $(
                    $(
                        assert_eq!(machine.processor.get_flag(Flag::$flag_post), $flag_post_value);
                    )+
                )?
                $(
                    $(
                        assert_eq!($eq_assert_lhs, $eq_assert_rhs);
                    )+
                )?
            }
        };
    }

    fn create_machine_with_opcodes(opcodes: &[Opcode]) -> Machine<MockDisplay> {
        let mut machine = Machine::new(create_mock_periphery());
        for (&opcode, address) in opcodes
            .iter()
            .zip((address_constants::ENTRY_POINT..).step_by(Instruction::SIZE))
        {
            machine.memory.write_opcode(address, opcode);
        }
        machine
    }

    fn execute_instruction_with_machine(
        mut machine: Machine<MockDisplay>,
        opcode: Opcode,
    ) -> Machine<MockDisplay> {
        let instruction_pointer = machine.processor.registers[Processor::INSTRUCTION_POINTER];
        machine.memory.write_opcode(instruction_pointer, opcode);
        machine
            .processor
            .execute_next_instruction(&mut machine.memory, &mut machine.periphery);
        assert_eq!(
            machine.processor.registers[Processor::INSTRUCTION_POINTER],
            instruction_pointer + Instruction::SIZE as u32
        );
        machine
    }

    fn create_mock_periphery() -> Periphery {
        let mut time = 0;
        Periphery {
            timer: Timer::new(move || {
                let old_value = time;
                time += 1;
                old_value
            }),
            keyboard: Keyboard::new(Box::new(|_| KeyState::Up)),
        }
    }

    create_test!(
        make_tick_increases_instruction_pointer,
        opcodes = &[Opcode::MoveRegisterImmediate {
            register: 0.into(),
            immediate: 0
        }],
        registers_post = [(
            Processor::INSTRUCTION_POINTER,
            address_constants::ENTRY_POINT + Instruction::SIZE as u32
        )],
    );

    create_test!(
        move_constant_into_register,
        setup = {
            let register = 0x0A.into();
            let value = 0xABCD_1234;
        },
        opcodes = &[MoveRegisterImmediate {
            register,
            immediate: value,
        }],
        registers_post = [(register, value)],
    );

    create_test!(
        move_from_address_into_register,
        setup = {
            let address = 0xF0;
            let data = 0xABCD_1234;
            let register = 0x0A.into();
        },
        opcodes = &[MoveRegisterAddress { register, address }],
        memory_pre = [data => address],
        registers_post = [(register, data)],
    );

    #[test]
    fn move_from_one_register_to_another() {
        let mut machine = Machine::new(create_mock_periphery());
        let source = 0x5.into();
        let target = 0x0A.into();
        let data = 0xCAFE;
        machine.processor.registers[source] = data;
        let machine =
            execute_instruction_with_machine(machine, MoveTargetSource { target, source });
        assert_eq!(machine.processor.registers[target], data);
    }

    create_test!(
        move_from_register_into_memory,
        setup = {
            let register = Register(5);
            let data = 0xC0FFEE;
            let address = 0xF0;
        },
        opcodes = &[MoveAddressRegister { address, register }],
        registers_pre = [data => register],
        memory_post = [(address, data)],
    );

    create_test!(
        move_from_memory_addressed_by_register_into_another_register,
        setup = {
            let address = 0xF0;
            let data = 0xC0FFEE;
            let target = 0x0A.into();
            let pointer = 0x05.into();
        },
        opcodes = &[MoveTargetPointer { target, pointer }],
        registers_pre = [address => pointer],
        memory_pre = [data => address],
        registers_post = [(target, data)],
    );

    create_test!(
        move_from_memory_addressed_by_register_into_same_register,
        setup = {
            let address = 0xF0;
            let data = 0xC0FFEE;
            let register = 0x05.into();
        },
        opcodes = &[MoveTargetPointer {
            target: register,
            pointer: register,
        }],
        registers_pre = [address => register],
        memory_pre = [data => address],
        registers_post = [(register, data)],
    );

    create_test!(
        move_from_register_into_memory_addressed_by_another_register,
        setup = {
            let data = 0xC0FFEE;
            let address = 0xF0;
            let pointer = 0x0A.into();
            let source = 0x05.into();
        },
        opcodes = &[MovePointerSource { pointer, source }],
        registers_pre = [data => source, address => pointer],
        memory_post = [(address, data)],
    );

    create_test!(
        move_from_register_into_memory_addressed_by_same_register,
        setup = {
            let address = 0xF0;
            let register = 0x05.into();
        },
        opcodes = &[MovePointerSource { pointer: register, source: register }],
        registers_pre = [address => register],
        memory_post = [(address, address)],
    );

    create_test!(
        halt_and_catch_fire_prevents_further_instructions,
        setup = {
            let register = 0x05.into();
            let value = 0x0000_0042;
        },
        opcodes = &[
            HaltAndCatchFire {},
            MoveRegisterImmediate {
                register,
                immediate: value,
            }
        ],
        registers_post = [
            (
                Processor::INSTRUCTION_POINTER,
                address_constants::ENTRY_POINT
            ),
            (register, 0x0)
        ],
    );

    macro_rules! create_addition_test{
        (
            $test_name:ident,
            $lhs:expr,
            $rhs:expr,
            zero = $zero:literal,
            carry = $carry:literal
        ) => {
            create_test!(
                $test_name,
                setup = {
                    let lhs_register = 0x42.into();
                    let rhs_register = 0x43.into();
                    let target_register = 0x0A.into();
                    let lhs: Word = $lhs;
                    let rhs = $rhs;
                    let expected = lhs.wrapping_add(rhs);
                },
                opcodes = &[AddTargetLhsRhs {
                    target: target_register,
                    lhs: lhs_register,
                    rhs: rhs_register,
                }],
                registers_pre = [lhs => lhs_register, rhs => rhs_register],
                registers_post = [
                    (lhs_register, lhs),
                    (rhs_register, rhs),
                    (target_register, expected)
                ],
                flags_post = [(Zero, $zero), (Carry, $carry)],
            );
        };
    }

    create_addition_test!(
        add_two_values_with_no_flags_set,
        10,
        12,
        zero = false,
        carry = false
    );

    create_addition_test!(
        add_two_values_with_only_zero_flag_set,
        0,
        0,
        zero = true,
        carry = false
    );

    create_addition_test!(
        add_two_values_with_only_carry_flag_set,
        Word::MAX,
        5,
        zero = false,
        carry = true
    );

    create_addition_test!(
        add_two_values_with_both_zero_and_carry_flags_set,
        Word::MAX,
        1,
        zero = true,
        carry = true
    );

    create_test!(
        add_two_values_with_carry_with_no_flags_set,
        opcodes = &[Opcode::AddWithCarryTargetLhsRhs {
            target: 2.into(),
            lhs: 0.into(),
            rhs: 1.into(),
        }],
        registers_pre = [5 => 0, 37 => 1],
        registers_post = [(0.into(), 5), (1.into(), 37), (2.into(), 42)],
        flags_post = [(Carry, false), (Zero, false)],
    );

    create_test!(
        add_two_values_with_carry_with_zero_flag_set,
        opcodes = &[Opcode::AddWithCarryTargetLhsRhs {
            target: 2.into(),
            lhs: 0.into(),
            rhs: 1.into(),
        }],
        registers_pre = [0 => 0, 0 => 1],
        registers_post = [(0.into(), 0), (1.into(), 0), (2.into(), 0)],
        flags_post = [(Carry, false), (Zero, true)],
    );

    create_test!(
        add_two_values_with_carry_with_carry_flag_set,
        opcodes = &[Opcode::AddWithCarryTargetLhsRhs {
            target: 2.into(),
            lhs: 0.into(),
            rhs: 1.into(),
        }],
        registers_pre = [Word::MAX => 0, 5 => 1],
        registers_post = [(0.into(), Word::MAX), (1.into(), 5), (2.into(), 4)],
        flags_post = [(Carry, true), (Zero, false)],
    );

    create_test!(
        add_two_values_with_carry_with_both_carry_and_zero_flags_set,
        opcodes = &[Opcode::AddWithCarryTargetLhsRhs {
            target: 2.into(),
            lhs: 0.into(),
            rhs: 1.into(),
        }],
        registers_pre = [Word::MAX => 0, 1 => 1],
        registers_post = [(0.into(), Word::MAX), (1.into(), 1), (2.into(), 0)],
        flags_post = [(Carry, true), (Zero, true)],
    );

    create_test!(
        add_two_values_with_carry_when_carry_flag_is_already_set_with_no_flags_set,
        opcodes = &[Opcode::AddWithCarryTargetLhsRhs {
            target: 2.into(),
            lhs: 0.into(),
            rhs: 1.into(),
        }],
        registers_pre = [5 => 0, 36 => 1],
        flags_pre = [true => Carry],
        registers_post = [(0.into(), 5), (1.into(), 36), (2.into(), 42)],
        flags_post = [(Carry, false), (Zero, false)],
    );

    create_test!(
        add_two_values_with_carry_when_carry_flag_is_already_set_with_carry_flag_set,
        opcodes = &[Opcode::AddWithCarryTargetLhsRhs {
            target: 2.into(),
            lhs: 0.into(),
            rhs: 1.into(),
        }],
        registers_pre = [Word::MAX => 0, 1 => 1],
        flags_pre = [true => Carry],
        registers_post = [(0.into(), Word::MAX), (1.into(), 1), (2.into(), 1)],
        flags_post = [(Carry, true), (Zero, false)],
    );

    create_test!(
        add_two_values_with_carry_when_carry_flag_is_already_set_with_both_carry_and_zero_flags_set,
        opcodes = &[Opcode::AddWithCarryTargetLhsRhs {
            target: 2.into(),
            lhs: 0.into(),
            rhs: 1.into(),
        }],
        registers_pre = [Word::MAX => 0, 0 => 1],
        flags_pre = [true => Carry],
        registers_post = [(0.into(), Word::MAX), (1.into(), 0), (2.into(), 0)],
        flags_post = [(Carry, true), (Zero, true)],
    );

    macro_rules! create_subtraction_test{
        (
            $test_name:ident,
            $lhs:expr,
            $rhs:expr,
            zero = $zero:literal,
            carry = $carry:literal
        ) => {
            create_test!(
                $test_name,
                setup = {
                    let lhs_register = 0x42.into();
                    let rhs_register = 0x43.into();
                    let target_register = 0x0A.into();
                    let lhs: Word = $lhs;
                    let rhs = $rhs;
                    let expected = lhs.wrapping_sub(rhs);
                },
                opcodes = &[SubtractTargetLhsRhs {
                    target: target_register,
                    lhs: lhs_register,
                    rhs: rhs_register,
                }],
                registers_pre = [lhs => lhs_register, rhs => rhs_register],
                registers_post = [
                    (lhs_register, lhs),
                    (rhs_register, rhs),
                    (target_register, expected)
                ],
                flags_post = [(Zero, $zero), (Carry, $carry)],
            );
        };
    }

    create_subtraction_test!(
        subtract_two_values_with_no_flags_set,
        10,
        8,
        zero = false,
        carry = false
    );

    create_subtraction_test!(
        subtract_two_values_with_only_zero_flag_set,
        10,
        10,
        zero = true,
        carry = false
    );

    create_subtraction_test!(
        subtract_two_values_with_only_carry_flag_set,
        10,
        12,
        zero = false,
        carry = true
    );

    create_test!(
        subtract_two_values_with_carry_with_no_flags_set,
        setup = {
            let lhs_register = 0x42.into();
            let rhs_register = 0x43.into();
            let target_register = 0x0A.into();
            let lhs: Word = 14;
            let rhs = 12;
            let expected = lhs.wrapping_sub(rhs + 1 /* carry */);
        },
        opcodes = &[SubtractWithCarryTargetLhsRhs {
            target: target_register,
            lhs: lhs_register,
            rhs: rhs_register,
        }],
        registers_pre = [lhs => lhs_register, rhs => rhs_register],
        flags_pre = [true => Carry],
        registers_post = [(lhs_register, lhs), (rhs_register, rhs), (target_register, expected)],
        flags_post = [(Zero, false), (Carry, false)],
    );

    create_test!(
        subtract_two_values_with_carry_with_zero_flag_set,
        setup = {
            let lhs_register = 0x42.into();
            let rhs_register = 0x43.into();
            let target_register = 0x0A.into();
            let lhs: Word = 14;
            let rhs = 13;
            let expected = lhs.wrapping_sub(rhs + 1 /* carry */);
        },
        opcodes = &[SubtractWithCarryTargetLhsRhs {
            target: target_register,
            lhs: lhs_register,
            rhs: rhs_register,
        }],
        registers_pre = [lhs => lhs_register, rhs => rhs_register],
        flags_pre = [true => Carry],
        registers_post = [(lhs_register, lhs), (rhs_register, rhs), (target_register, expected)],
        flags_post = [(Zero, true), (Carry, false)],
    );

    create_test!(
        subtract_two_values_with_carry_with_both_carry_and_zero_flags_set,
        setup = {
            let lhs_register = 0x42.into();
            let rhs_register = 0x43.into();
            let target_register = 0x0A.into();
            let lhs: Word = 0;
            let rhs = Word::MAX;
            let expected = lhs.wrapping_sub(rhs).wrapping_sub(1);
        },
        opcodes = &[SubtractWithCarryTargetLhsRhs {
            target: target_register,
            lhs: lhs_register,
            rhs: rhs_register,
        }],
        registers_pre = [lhs => lhs_register, rhs => rhs_register],
        flags_pre = [true => Carry],
        registers_post = [(lhs_register, lhs), (rhs_register, rhs), (target_register, expected)],
        flags_post = [(Zero, true), (Carry, true)],
    );

    create_test!(
        multiply_two_values_without_any_flags_set,
        setup = {
            let lhs_register = 0x42.into();
            let rhs_register = 0x43.into();
            let target_high = 0x09.into();
            let target_low = 0x0A.into();
            let lhs: Word = 3;
            let rhs = 4;
            let expected = lhs * rhs;
        },
        opcodes = &[MultiplyHighLowLhsRhs {
            high: target_high,
            low: target_low,
            lhs: lhs_register,
            rhs: rhs_register,
        }],
        registers_pre = [lhs => lhs_register, rhs => rhs_register],
        registers_post = [(lhs_register, lhs), (rhs_register, rhs), (target_high, 0), (target_low, expected)],
        flags_post = [(Carry, false), (Zero, false)],
    );

    create_test!(
        multiply_two_values_with_zero_flag_set,
        setup = {
            let lhs_register = 0x42.into();
            let rhs_register = 0x43.into();
            let target_high = 0x09.into();
            let target_low = 0x0A.into();
            let lhs: Word = 3;
            let rhs = 0;
            let expected = lhs * rhs;
        },
        opcodes = &[MultiplyHighLowLhsRhs {
            high: target_high,
            low: target_low,
            lhs: lhs_register,
            rhs: rhs_register,
        }],
        registers_pre = [lhs => lhs_register, rhs => rhs_register],
        registers_post = [(lhs_register, lhs), (rhs_register, rhs), (target_high, 0), (target_low, expected)],
        flags_post = [(Carry, false), (Zero, true)],
    );

    create_test!(
        multiply_two_values_with_overflow,
        setup = {
            let lhs_register = 0x42.into();
            let rhs_register = 0x43.into();
            let target_high = 0x09.into();
            let target_low = 0x0A.into();
            let lhs: Word = Word::MAX;
            let rhs = 5;
            let result = lhs as u64 * rhs as u64;
            let high_expected = (result >> 32) as u32;
            let low_expected = result as u32;
        },
        opcodes = &[MultiplyHighLowLhsRhs {
            high: target_high,
            low: target_low,
            lhs: lhs_register,
            rhs: rhs_register,
        }],
        registers_pre = [lhs => lhs_register, rhs => rhs_register],
        registers_post = [(lhs_register, lhs), (rhs_register, rhs), (target_high, high_expected), (target_low, low_expected)],
        flags_post = [(Carry, true), (Zero, false)],
    );

    create_test!(
        multiply_two_values_with_overflow_and_zero_flags_set,
        setup = {
            let lhs_register = 0x42.into();
            let rhs_register = 0x43.into();
            let target_high = 0x09.into();
            let target_low = 0x0A.into();
            let lhs: Word = 1 << (Word::BITS - 1);
            let rhs = 2;
            let result = lhs as u64 * rhs as u64;
            let high_expected = (result >> 32) as u32;
            let low_expected = result as u32;
        },
        opcodes = &[MultiplyHighLowLhsRhs {
            high: target_high,
            low: target_low,
            lhs: lhs_register,
            rhs: rhs_register,
        }],
        registers_pre = [lhs => lhs_register, rhs => rhs_register],
        registers_post = [(lhs_register, lhs), (rhs_register, rhs), (target_high, high_expected), (target_low, low_expected)],
        flags_post = [(Carry, true), (Zero, true)],
    );

    macro_rules! create_divmod_test{
        (
            $test_name:ident,
            $lhs:expr,
            $rhs:expr,
            $quotient:expr,
            $remainder:expr,
            divide_by_zero = $divide_by_zero:literal,
            zero = $zero:literal
        ) => {
            create_test!(
                $test_name,
                setup = {
                    let lhs_register = 0x42.into();
                    let rhs_register = 0x43.into();
                    let target_quotient = 0x09.into();
                    let target_remainder = 0x0A.into();
                    let lhs: Word = $lhs;
                    let rhs = $rhs;
                    let expected_quotient = $quotient;
                    let expected_remainder = $remainder;
                },
                opcodes = &[DivmodTargetModLhsRhs {
                    result: target_quotient,
                    remainder: target_remainder,
                    lhs: lhs_register,
                    rhs: rhs_register,
                }],
                registers_pre = [lhs => lhs_register, rhs => rhs_register],
                registers_post = [
                    (lhs_register, lhs),
                    (rhs_register, rhs),
                    (target_quotient, expected_quotient),
                    (target_remainder, expected_remainder)],
                flags_post = [(DivideByZero, $divide_by_zero), (Zero, $zero)],
            );
        }
    }

    create_divmod_test!(
        divmod_two_values_with_no_flags_set,
        15,
        6,
        2,
        3,
        divide_by_zero = false,
        zero = false
    );

    create_divmod_test!(
        divmod_two_values_with_zero_flag_set,
        0,
        6,
        0,
        0,
        divide_by_zero = false,
        zero = true
    );

    create_divmod_test!(
        divmod_two_values_divide_by_zero,
        15,
        0,
        0,
        15,
        divide_by_zero = true,
        zero = true
    );

    macro_rules! create_bitwise_test{
        (
            $test_name:ident,
            $bitwise_instruction:ident,
            $lhs:expr,
            $rhs:expr,
            $expected:expr,
            zero = $zero:literal
        ) => {
            create_test!(
                $test_name,
                setup = {
                    let lhs_register = 0x42.into();
                    let rhs_register = 0x43.into();
                    let target_register = 0x0A.into();
                    let lhs: Word = $lhs;
                    let rhs = $rhs;
                    let expected = $expected;
                },
                opcodes = &[$bitwise_instruction {
                    target: target_register,
                    lhs: lhs_register,
                    rhs: rhs_register,
                }],
                registers_pre = [lhs => lhs_register, rhs => rhs_register],
                registers_post = [(lhs_register, lhs), (rhs_register, rhs), (target_register, expected)],
                flags_post = [(Zero, $zero)],
            );
        }
    }

    create_bitwise_test!(
        bitwise_and_two_values_with_no_flags_set,
        AndTargetLhsRhs,
        0b0110_1110_1001_1010_0110_1110_1001_1010,
        0b1011_1010_0101_1001_1011_1010_0101_1001,
        0b0010_1010_0001_1000_0010_1010_0001_1000,
        zero = false
    );

    create_bitwise_test!(
        bitwise_and_two_values_with_zero_flag_set,
        AndTargetLhsRhs,
        0b0100_0100_1000_0110_0100_0100_1000_0010,
        0b1011_1010_0101_1001_1011_1010_0101_1001,
        0,
        zero = true
    );

    create_bitwise_test!(
        bitwise_or_two_values_with_no_flags_set,
        OrTargetLhsRhs,
        0b0110_1110_1001_1010_0110_1110_1001_1010,
        0b1011_1010_0101_1001_1011_1010_0101_1001,
        0b1111_1110_1101_1011_1111_1110_1101_1011,
        zero = false
    );

    create_bitwise_test!(
        bitwise_or_two_values_with_zero_flag_set,
        OrTargetLhsRhs,
        0,
        0,
        0,
        zero = true
    );

    create_bitwise_test!(
        bitwise_xor_two_values_with_no_flags_set,
        XorTargetLhsRhs,
        0b0110_1110_1001_1010_0110_1110_1001_1010,
        0b1011_1010_0101_1001_1011_1010_0101_1001,
        0b1101_0100_1100_0011_1101_0100_1100_0011,
        zero = false
    );

    create_bitwise_test!(
        bitwise_xor_two_values_with_zero_flag_set,
        XorTargetLhsRhs,
        0b1011_1010_1001_0010_0100_0100_1001_0010,
        0b1011_1010_1001_0010_0100_0100_1001_0010,
        0,
        zero = true
    );

    create_test!(
        bitwise_not_value_with_no_flags_set,
        setup = {
            let source = 0x5.into();
            let target = 0x0A.into();
            let data = 0b0010_1010_0001_1000_0010_1010_0001_1000;
            let expected = 0b1101_0101_1110_0111_1101_0101_1110_0111;
        },
        opcodes = &[NotTargetSource { target, source }],
        registers_pre = [data => source],
        registers_post = [(target, expected)],
        flags_post = [(Zero, false)],
    );

    create_test!(
        bitwise_not_value_with_zero_flag_set,
        setup = {
            let source = 0x5.into();
            let target = 0x0A.into();
            let data = 0xFFFFFFFF;
            let expected = 0;
        },
        opcodes = &[NotTargetSource { target, source }],
        registers_pre = [data => source],
        registers_post = [(target, expected)],
        flags_post = [(Zero, true)],
    );

    macro_rules! create_shift_test{
        (
            $test_name:ident,
            $shift_instruction:ident,
            $lhs:expr,
            $rhs:expr,
            $expected:expr,
            zero = $zero:literal,
            carry = $carry:literal
        ) => {
            create_test!(
                $test_name,
                opcodes = &[$shift_instruction {
                    target: 0x0A.into(),
                    lhs: 0x5.into(),
                    rhs: 0x6.into(),
                }],
                registers_pre = [$lhs => Register(0x5), $rhs => Register(0x6)],
                registers_post = [(0x5.into(), $lhs), (0x6.into(), $rhs), (0x0A.into(), $expected)],
                flags_post = [(Zero, $zero), (Carry, $carry)],
            );
        }
    }

    create_shift_test!(
        left_shift_without_any_flags_set,
        LeftShiftTargetLhsRhs,
        0b1,
        2,
        0b100,
        zero = false,
        carry = false
    );

    create_shift_test!(
        left_shift_with_carry_flag_set,
        LeftShiftTargetLhsRhs,
        0b11 << 30,
        1,
        0b1 << 31,
        zero = false,
        carry = true
    );

    create_shift_test!(
        left_shift_with_carry_and_zero_flags_set,
        LeftShiftTargetLhsRhs,
        0b1 << 31,
        1,
        0,
        zero = true,
        carry = true
    );

    create_shift_test!(
        left_shift_way_too_far,
        LeftShiftTargetLhsRhs,
        0xFFFF_FFFF,
        123,
        0,
        zero = true,
        carry = true
    );

    create_shift_test!(
        left_shift_zero_way_too_far,
        LeftShiftTargetLhsRhs,
        0,
        123,
        0,
        zero = true,
        carry = false
    );

    create_shift_test!(
        right_shift_without_any_flags_set,
        RightShiftTargetLhsRhs,
        0b10,
        1,
        0b1,
        zero = false,
        carry = false
    );

    create_shift_test!(
        right_shift_with_carry_flag_set,
        RightShiftTargetLhsRhs,
        0b11,
        1,
        0b1,
        zero = false,
        carry = true
    );

    create_shift_test!(
        right_shift_with_zero_flag_set,
        RightShiftTargetLhsRhs,
        0b0,
        1,
        0,
        zero = true,
        carry = false
    );

    create_shift_test!(
        right_shift_with_carry_and_zero_flags_set,
        RightShiftTargetLhsRhs,
        0b1,
        1,
        0,
        zero = true,
        carry = true
    );

    create_shift_test!(
        right_shift_way_too_far,
        RightShiftTargetLhsRhs,
        0xFFFF_FFFF,
        123,
        0b0,
        zero = true,
        carry = true
    );

    create_shift_test!(
        right_shift_zero_way_too_far,
        RightShiftTargetLhsRhs,
        0,
        123,
        0b0,
        zero = true,
        carry = false
    );

    macro_rules! create_add_immediate_test{
        (
            $test_name:ident,
            $immediate:expr,
            $source_value:expr,
            $expected_value:expr,
            zero = $zero:literal,
            carry = $carry:literal
        ) => {
            create_test!(
                $test_name,
                opcodes = &[AddTargetSourceImmediate {
                    target: Register(0xAB),
                    source: Register(0x07),
                    immediate: $immediate,
                }],
                registers_pre = [$source_value => Register(0x07)],
                registers_post = [(Register(0x07), $source_value), (Register(0xAB), $expected_value)],
                flags_post = [(Zero, $zero), (Carry, $carry)],
            );
        }
    }

    create_add_immediate_test!(
        add_immediate_with_no_flags_set,
        2,
        40,
        42,
        zero = false,
        carry = false
    );

    create_add_immediate_test!(
        add_immediate_with_zero_flag_set,
        0,
        0,
        0,
        zero = true,
        carry = false
    );

    create_add_immediate_test!(
        add_immediate_with_carry_flag_set,
        5,
        Word::MAX,
        4,
        zero = false,
        carry = true
    );

    create_add_immediate_test!(
        add_immediate_with_both_flags_set,
        1,
        Word::MAX,
        0,
        zero = true,
        carry = true
    );

    macro_rules! create_subtract_immediate_test{
        (
            $test_name:ident,
            $immediate:expr,
            $source_value:expr,
            $expected_value:expr,
            zero = $zero:literal,
            carry = $carry:literal
        ) => {
            create_test!(
                $test_name,
                opcodes = &[SubtractTargetSourceImmediate {
                    target: Register(0xAB),
                    source: Register(0x07),
                    immediate: $immediate,
                }],
                registers_pre = [$source_value => Register(0x07)],
                registers_post = [(Register(0x07), $source_value), (Register(0xAB), $expected_value)],
                flags_post = [(Zero, $zero), (Carry, $carry)],
            );
        }
    }

    create_subtract_immediate_test!(
        subtract_immediate_with_no_flags_set,
        2,
        44,
        42,
        zero = false,
        carry = false
    );

    create_subtract_immediate_test!(
        subtract_immediate_with_zero_flag_set,
        42,
        42,
        0,
        zero = true,
        carry = false
    );

    create_subtract_immediate_test!(
        subtract_immediate_with_carry_flag_set,
        2,
        1,
        Word::MAX,
        zero = false,
        carry = true
    );

    macro_rules! create_comparison_test{
        (
            $test_name:ident,
            $lhs:expr,
            $rhs:expr,
            $expected:expr,
            zero = $zero:literal
        ) => {
            create_test!(
                $test_name,
                opcodes = &[CompareTargetLhsRhs {
                    target: Register(0x0A),
                    lhs: Register(0x42),
                    rhs: Register(0x43),
                }],
                registers_pre = [$lhs => Register(0x42), $rhs => Register(0x43)],
                registers_post = [
                    (Register(0x42), $lhs),
                    (Register(0x43), $rhs),
                    (Register(0x0A), $expected)
                ],
                flags_post = [(Zero, $zero)],
            );
        }
    }

    create_comparison_test!(
        compare_lower_value_against_higher_value,
        10,
        12,
        Word::MAX,
        zero = false
    );

    create_comparison_test!(
        compare_higher_value_against_lower_value,
        14,
        12,
        1,
        zero = false
    );

    create_comparison_test!(compare_equal_values, 12, 12, 0, zero = true);

    #[test]
    fn push_and_pop_stack_value() {
        let mut machine = Machine::new(create_mock_periphery());
        let source_register = 0xAB.into();
        let target_register = 0x06.into();
        let data = 42;
        machine.processor.registers[source_register] = data;
        assert_eq!(
            machine.processor.get_stack_pointer(),
            address_constants::STACK_START
        );
        let machine = execute_instruction_with_machine(
            machine,
            PushRegister {
                register: source_register,
            },
        );
        assert_eq!(
            machine.processor.get_stack_pointer(),
            address_constants::STACK_START + Word::SIZE as Address
        );
        assert_eq!(
            machine.memory.read_data(address_constants::STACK_START),
            data
        );
        let machine = execute_instruction_with_machine(
            machine,
            PopRegister {
                register: target_register,
            },
        );
        assert_eq!(
            machine.processor.get_stack_pointer(),
            address_constants::STACK_START
        );
        assert_eq!(machine.processor.registers[target_register], data);
    }

    #[test]
    fn push_and_pop_multiple_stack_values() {
        let values = [1, 4, 5, 42, 2, 3];
        let mut machine = Machine::new(create_mock_periphery());
        for (register, value) in (0..).map(Register).zip(values) {
            machine.processor.registers[register] = value;
            machine = execute_instruction_with_machine(machine, PushRegister { register });
            assert_eq!(
                machine.processor.get_stack_pointer(),
                address_constants::STACK_START
                    + (register.0 as Address + 1) * Word::SIZE as Address
            );
            assert_eq!(
                machine.memory.read_data(
                    address_constants::STACK_START + register.0 as Address * Word::SIZE as Address
                ),
                value
            );
        }
        for &value in values.iter().rev() {
            let target = 0xAB.into();
            machine = execute_instruction_with_machine(machine, PopRegister { register: target });
            assert_eq!(machine.processor.registers[target], value);
        }
        assert_eq!(
            machine.processor.get_stack_pointer(),
            address_constants::STACK_START
        );
    }

    #[test]
    fn call_and_return() {
        let mut machine: Machine<MockDisplay> = Machine::new(create_mock_periphery());
        let call_address = address_constants::ENTRY_POINT + 200 * Instruction::SIZE as Address;
        machine.memory.write_opcode(
            address_constants::ENTRY_POINT,
            Opcode::CallAddress {
                address: call_address,
            },
        );
        let target_register = Register(0xAB);
        let value = 42;
        machine.memory.write_opcode(
            call_address,
            Opcode::MoveRegisterImmediate {
                register: target_register,
                immediate: value,
            },
        );
        machine.memory.write_opcode(
            call_address + Instruction::SIZE as Address,
            Opcode::Return {},
        );

        machine.execute_next_instruction(); // jump into subroutine
        assert_eq!(
            machine.memory.read_data(address_constants::STACK_START),
            address_constants::ENTRY_POINT + Instruction::SIZE as Address
        );
        assert_eq!(
            machine.processor.registers[Processor::INSTRUCTION_POINTER],
            call_address
        );

        machine.execute_next_instruction(); // write value into register
        assert_eq!(machine.processor.registers[target_register], value);
        assert_eq!(
            machine.processor.registers[Processor::INSTRUCTION_POINTER],
            call_address + Instruction::SIZE as Address
        );

        machine.execute_next_instruction(); // jump back from subroutine
        assert_eq!(
            machine.processor.registers[Processor::INSTRUCTION_POINTER],
            address_constants::ENTRY_POINT + Instruction::SIZE as Address
        );
    }

    create_test!(
        jump_to_address,
        setup = {
            let address = address_constants::ENTRY_POINT as Address + 42;
        },
        opcodes = &[Opcode::JumpAddress { address }],
        registers_post = [(Processor::INSTRUCTION_POINTER, address)],
    );

    create_test!(
        jump_to_pointer,
        setup = {
            let register = Register(0xAB);
            let address = address_constants::ENTRY_POINT as Address + 42;
        },
        opcodes = &[Opcode::JumpRegister { register }],
        registers_pre = [address => register],
        registers_post = [(Processor::INSTRUCTION_POINTER, address)],
    );

    macro_rules! create_jump_tests {
        (
            $address_test_name:ident,
            $pointer_test_name:ident,
            $jump_address_instruction:ident,
            $jump_register_instruction:ident,
            $lhs:literal,
            $rhs:literal,
            $should_jump:literal
        ) => {
            // create test for "jump address"
            create_test!(
                $address_test_name,
                setup = {
                    let target_address = address_constants::ENTRY_POINT + 42 * Instruction::SIZE as Address;
                    let target_register = 0.into();
                },
                opcodes = &[
                    Opcode::CompareTargetLhsRhs {
                        target: target_register,
                        lhs: 1.into(),
                        rhs: 2.into(),
                    },
                    Opcode::$jump_address_instruction {
                        comparison: target_register,
                        address: target_address,
                    },
                ],
                registers_pre = [$lhs => 1, $rhs => 2],
                registers_post = [(Processor::INSTRUCTION_POINTER, if $should_jump { target_address } else {
                    address_constants::ENTRY_POINT + 2 * Instruction::SIZE as Address
                })],
            );

            // create test for "jump register"
            create_test!(
                $pointer_test_name,
                setup = {
                    let target_address = address_constants::ENTRY_POINT + 42 * Instruction::SIZE as Address;
                    let pointer_register = 0xA.into();
                    let comparison_register = 0.into();
                },
                opcodes = &[
                    Opcode::CompareTargetLhsRhs {
                        target: comparison_register,
                        lhs: 1.into(),
                        rhs: 2.into(),
                    },
                    Opcode::$jump_register_instruction {
                        pointer: pointer_register,
                        comparison: comparison_register,
                    },
                ],
                registers_pre = [$lhs => 1, $rhs => 2, target_address => pointer_register],
                registers_post = [(Processor::INSTRUCTION_POINTER, if $should_jump { target_address } else {
                    address_constants::ENTRY_POINT + 2 * Instruction::SIZE as Address
                })],
            );
        };
    }

    create_jump_tests!(
        jump_to_address_if_equal_that_jumps,
        jump_to_register_if_equal_that_jumps,
        JumpAddressIfEqual,
        JumpRegisterIfEqual,
        42,
        42,
        true
    );

    create_jump_tests!(
        jump_to_address_if_equal_that_does_not_jump,
        jump_to_register_if_equal_that_does_not_jump,
        JumpAddressIfEqual,
        JumpRegisterIfEqual,
        42,
        43,
        false
    );

    create_jump_tests!(
        jump_to_address_if_greater_than_that_jumps,
        jump_to_register_if_greater_than_that_jumps,
        JumpAddressIfGreaterThan,
        JumpRegisterIfGreaterThan,
        43,
        42,
        true
    );

    create_jump_tests!(
        jump_to_address_if_greater_than_that_does_not_jump_01,
        jump_to_register_if_greater_than_that_does_not_jump_01,
        JumpAddressIfGreaterThan,
        JumpRegisterIfGreaterThan,
        42,
        43,
        false
    );

    create_jump_tests!(
        jump_to_address_if_greater_than_that_does_not_jump_02,
        jump_to_register_if_greater_than_that_does_not_jump_02,
        JumpAddressIfGreaterThan,
        JumpRegisterIfGreaterThan,
        42,
        42,
        false
    );

    create_jump_tests!(
        jump_to_address_if_less_than_that_jumps,
        jump_to_register_if_less_than_that_jumps,
        JumpAddressIfLessThan,
        JumpRegisterIfLessThan,
        41,
        42,
        true
    );

    create_jump_tests!(
        jump_to_address_if_less_than_that_does_not_jump_01,
        jump_to_register_if_less_than_that_does_not_jump_01,
        JumpAddressIfLessThan,
        JumpRegisterIfLessThan,
        43,
        42,
        false
    );

    create_jump_tests!(
        jump_to_address_if_less_than_that_does_not_jump_02,
        jump_to_register_if_less_than_that_does_not_jump_02,
        JumpAddressIfLessThan,
        JumpRegisterIfLessThan,
        42,
        42,
        false
    );

    create_jump_tests!(
        jump_to_address_if_less_than_or_equal_that_jumps_01,
        jump_to_register_if_less_than_or_equal_that_jumps_01,
        JumpAddressIfLessThanOrEqual,
        JumpRegisterIfLessThanOrEqual,
        41,
        42,
        true
    );

    create_jump_tests!(
        jump_to_address_if_less_than_or_equal_that_jumps_02,
        jump_to_register_if_less_than_or_equal_that_jumps_02,
        JumpAddressIfLessThanOrEqual,
        JumpRegisterIfLessThanOrEqual,
        42,
        42,
        true
    );

    create_jump_tests!(
        jump_to_address_if_less_than_or_equal_that_does_not_jump,
        jump_to_register_if_less_than_or_equal_that_does_not_jump,
        JumpAddressIfLessThanOrEqual,
        JumpRegisterIfLessThanOrEqual,
        43,
        42,
        false
    );

    create_jump_tests!(
        jump_to_address_if_greater_than_or_equal_that_jumps_01,
        jump_to_register_if_greater_than_or_equal_that_jumps_01,
        JumpAddressIfGreaterThanOrEqual,
        JumpRegisterIfGreaterThanOrEqual,
        43,
        42,
        true
    );

    create_jump_tests!(
        jump_to_address_if_greater_than_or_equal_that_jumps_02,
        jump_to_register_if_greater_than_or_equal_that_jumps_02,
        JumpAddressIfGreaterThanOrEqual,
        JumpRegisterIfGreaterThanOrEqual,
        42,
        42,
        true
    );

    create_jump_tests!(
        jump_to_address_if_greater_than_or_equal_that_does_not_jump,
        jump_to_register_if_greater_than_or_equal_that_does_not_jump,
        JumpAddressIfGreaterThanOrEqual,
        JumpRegisterIfGreaterThanOrEqual,
        41,
        42,
        false
    );

    macro_rules! create_jump_flag_test(
        (
            $test_name:ident,
            $jump_instruction:ident,
            $lhs:expr,
            $rhs:expr,
            $should_jump:literal
        ) => {
            create_test!(
                $test_name,
                setup = {
                    let target_address = address_constants::ENTRY_POINT + 42 * Instruction::SIZE as Address;
                    let high_register = 3.into();
                    let target_register = 0.into();
                },
                opcodes = &[
                    Opcode::MultiplyHighLowLhsRhs {
                        high: high_register,
                        low: target_register,
                        lhs: 1.into(),
                        rhs: 2.into(),
                    },
                    Opcode::$jump_instruction {
                        address: target_address,
                    },
                ],
                registers_pre = [$lhs => 1, $rhs => 2],
                registers_post = [(Processor::INSTRUCTION_POINTER, if $should_jump { target_address } else {
                    address_constants::ENTRY_POINT + 2 * Instruction::SIZE as Address
                })],
            );
        }
    );

    create_jump_flag_test!(
        jump_to_address_if_zero_flag_set_that_jumps,
        JumpAddressIfZero,
        5,
        0,
        true
    );

    create_jump_flag_test!(
        jump_to_address_if_zero_flag_set_that_does_not_jump,
        JumpAddressIfZero,
        5,
        2,
        false
    );

    create_jump_flag_test!(
        jump_to_address_if_zero_flag_not_set_that_jumps,
        JumpAddressIfNotZero,
        5,
        3,
        true
    );

    create_jump_flag_test!(
        jump_to_address_if_zero_flag_not_set_that_does_not_jump,
        JumpAddressIfNotZero,
        5,
        0,
        false
    );

    create_jump_flag_test!(
        jump_to_address_if_carry_flag_set_that_jumps,
        JumpAddressIfCarry,
        Word::MAX,
        2,
        true
    );

    create_jump_flag_test!(
        jump_to_address_if_carry_flag_set_that_does_not_jump,
        JumpAddressIfCarry,
        5,
        2,
        false
    );

    create_jump_flag_test!(
        jump_to_address_if_carry_flag_not_set_that_jumps,
        JumpAddressIfNotCarry,
        5,
        3,
        true
    );

    create_jump_flag_test!(
        jump_to_address_if_carry_flag_not_set_that_does_not_jump,
        JumpAddressIfNotCarry,
        2,
        Word::MAX,
        false
    );

    macro_rules! create_jump_divmod_test {
        (
            $test_name:ident,
            $jump_instruction:ident,
            $lhs:expr,
            $rhs:expr,
            $should_jump:literal
        ) => {
            create_test!(
                $test_name,
                setup = {
                    let target_address = address_constants::ENTRY_POINT + 42 * Instruction::SIZE as Address;
                    let remainder_register = 3.into();
                    let target_register = 0.into();
                },
                opcodes = &[
                    Opcode::DivmodTargetModLhsRhs {
                        result: target_register,
                        remainder: remainder_register,
                        lhs: 1.into(),
                        rhs: 2.into(),
                    },
                    Opcode::$jump_instruction {
                        address: target_address,
                    },
                ],
                registers_pre = [$lhs => 1, $rhs => 2],
                registers_post = [(Processor::INSTRUCTION_POINTER, if $should_jump { target_address } else {
                    address_constants::ENTRY_POINT + 2 * Instruction::SIZE as Address
                })],
            );
        };
    }

    create_jump_divmod_test!(
        jump_to_address_if_divide_by_zero_flag_set_that_jumps,
        JumpAddressIfDivideByZero,
        5,
        0,
        true
    );

    create_jump_divmod_test!(
        jump_to_address_if_divide_by_zero_flag_set_that_does_not_jump,
        JumpAddressIfDivideByZero,
        5,
        2,
        false
    );

    create_jump_divmod_test!(
        jump_to_address_if_divide_by_zero_flag_not_set_that_jumps,
        JumpAddressIfNotDivideByZero,
        5,
        3,
        true
    );

    create_jump_divmod_test!(
        jump_to_address_if_divide_by_zero_flag_not_set_that_does_not_jump,
        JumpAddressIfNotDivideByZero,
        2,
        0,
        false
    );

    create_test!(
        no_op_does_advance_the_instruction_pointer,
        opcodes = &[NoOp {}],
        registers_post = [(
            Processor::INSTRUCTION_POINTER,
            address_constants::ENTRY_POINT + Instruction::SIZE as Address
        )],
    );

    #[test]
    fn get_keystate() {
        let keycode_register = 0.into();
        let target_register = 1.into();
        let mut machine = create_machine_with_opcodes(&[
            Opcode::GetKeyState {
                target: target_register,
                keycode: keycode_register,
            },
            Opcode::GetKeyState {
                target: target_register,
                keycode: keycode_register,
            },
        ]);
        machine.periphery.keyboard = Keyboard::new(Box::new(|keycode| {
            if raylib::input::key_from_i32(keycode.try_into().expect("keycode out of range"))
                .expect("invalid keycode")
                == raylib::consts::KeyboardKey::KEY_A
            {
                KeyState::Down
            } else {
                KeyState::Up
            }
        }));
        machine.processor.registers[keycode_register] = raylib::consts::KeyboardKey::KEY_A as Word;
        machine.execute_next_instruction();
        assert_eq!(machine.processor.registers[target_register], 1);
        assert!(!machine.processor.get_flag(Flag::Zero));

        machine.processor.registers[keycode_register] = raylib::consts::KeyboardKey::KEY_B as Word;
        machine.execute_next_instruction();
        assert_eq!(machine.processor.registers[target_register], 0);
        assert!(machine.processor.get_flag(Flag::Zero));
    }

    create_test!(
        poll_time_twice,
        opcodes = &[
            Opcode::PollTime {
                high: 0.into(),
                low: 1.into()
            },
            Opcode::PollTime {
                high: 0.into(),
                low: 1.into()
            },
        ],
        registers_post = [(0.into(), 0), (1.into(), 1)],
    );
}
