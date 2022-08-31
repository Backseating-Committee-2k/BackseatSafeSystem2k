use crate::{Address, AsHalfwords, AsWords, Instruction, Register, Word};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

macro_rules! type_to_abbreviation {
    (immediate) => {
        "cccc\u{00a0}cccc"
    };
    (source_address) => {
        "aaaa\u{00a0}aaaa"
    };
    (target_address) => {
        "aaaa\u{00a0}aaaa"
    };
}

macro_rules! stringify_registers {
    ( () ) => {
        "____\u{00a0}____\u{00a0}____"
    };
    ( (), $type:ident ) => {
        concat!("____\u{00a0}", type_to_abbreviation!($type))
    };
    ( ( $r0:ident ) ) => {
        concat!(
            stringify!($r0),
            stringify!($r0),
            "__\u{00a0}____\u{00a0}____"
        )
    };
    ( ( $r0:ident, $r1:ident ) ) => {
        concat!(
            stringify!($r0),
            stringify!($r0),
            stringify!($r1),
            stringify!($r1),
            "\u{00a0}____\u{00a0}____"
        )
    };
    ( ( $r0:ident ), $type:ident) => {
        concat!(
            stringify!($r0),
            stringify!($r0),
            "__\u{00a0}",
            type_to_abbreviation!($type)
        )
    };
    ( ( $r0:ident, $r1:ident ), $type:ident) => {
        concat!(
            stringify!($r0),
            stringify!($r0),
            stringify!($r1),
            stringify!($r1),
            "\u{00a0}",
            type_to_abbreviation!($type)
        )
    };
    ( ($r0:ident, $r1:ident, $r2:ident) ) => {
        concat!(
            stringify!($r0),
            stringify!($r0),
            stringify!($r1),
            stringify!($r1),
            "\u{00a0}",
            stringify!($r2),
            stringify!($r2),
            "__\u{00a0}____"
        )
    };
    ( ($r0:ident, $r1:ident, $r2:ident, $r3:ident) ) => {
        concat!(
            stringify!($r0),
            stringify!($r0),
            stringify!($r1),
            stringify!($r1),
            "\u{00a0}",
            stringify!($r2),
            stringify!($r2),
            stringify!($r3),
            stringify!($r3),
            "\u{00a0}____",
        )
    };
    ( ($r0:ident, $r1:ident, $r2:ident, $r3:ident, $r4:ident) ) => {
        concat!(
            stringify!($r0),
            stringify!($r0),
            stringify!($r1),
            stringify!($r1),
            "\u{00a0}",
            stringify!($r2),
            stringify!($r2),
            stringify!($r3),
            stringify!($r3),
            "\u{00a0}",
            stringify!($r4),
            stringify!($r4),
            "__",
        )
    };
    ( ($r0:ident, $r1:ident, $r2:ident, $r3:ident, $r4:ident, $r5:ident) ) => {
        concat!(
            stringify!($r0),
            stringify!($r0),
            stringify!($r1),
            stringify!($r1),
            "\u{00a0}",
            stringify!($r2),
            stringify!($r2),
            stringify!($r3),
            stringify!($r3),
            "\u{00a0}",
            stringify!($r4),
            stringify!($r4),
            stringify!($r5),
            stringify!($r5),
        )
    };
}

macro_rules! type_to_datatype {
    (immediate) => {
        Word
    };
    (source_address) => {
        Address
    };
    (target_address) => {
        Address
    };
}

macro_rules! registers_to_instruction {
    // entrypoint with at least one element
    ( $( $r:ident ),+ ) => {
        registers_to_instruction!(@ $( $r ),+ v 48)
    };
    // entrypoint with zero elements
    () => {
        0 as Instruction
    };
    // inner invocation with more then one element
    (@ $r:ident, $( $rest:ident ),+ v $v:expr ) => {
        ( ($r.0 as Instruction) << ($v-8) | registers_to_instruction!(@ $( $rest ),+ v $v - 8 ) )
    };
    // inner invocation with exactly one element
    (@ $r:ident v $v:expr ) => {
        ( ($r.0 as Instruction) << ($v-8) )
    };
}

macro_rules! type_to_opcode_type {
    () => {
        None
    };
    ($type:ident) => {
        Some(stringify!($type))
    };
}

macro_rules! opcodes {
    ( $({
        $identifier:ident,
        $code:literal,
        registers( $($register_usage:ident $register_letter:ident $register_name:ident ),* ) $(, $type:ident )? ;
        cycles = $num_cycles:literal,
        Increment::$should_increment:ident,
        $comment:literal
    },)+ ) => {
        enum Increment {
            Yes,
            No,
        }

        #[derive(Serialize)]
        pub enum RegisterUsage {
            Target,
            Source,
        }

        /// ## Opcodes
        /// | Opcode                | Name | Meaning                                   |
        /// |-----------------------|------|-------------------------------------------|
        $(
            #[doc = concat!(" | `", stringify!($code), "\u{00a0}", stringify_registers!(($( $register_letter ),*) $(, $type)?), "` | ", stringify!($identifier), " | ", $comment, " |")]
        )+

        #[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
        #[repr(u32)] // for the speeds (blame: slartibart)
        pub enum Opcode {
            $(
                $identifier{ $( $register_name : Register, )* $($type : type_to_datatype!($type))? },
            )+
        }

        #[derive(Serialize)]
        pub enum Argument {
            Register(RegisterUsage, &'static str, &'static str),
            Address,
            Immediate,
        }

        #[derive(Serialize)]
        pub struct OpcodeDescription {
            opcode: u16,
            arguments: Vec<Argument>,
            opcode_type: Option<&'static str>,
            cycles: usize,
            should_increment: bool,
            docstring: &'static str,
        }

        impl Opcode {
            pub fn as_hashmap() -> HashMap<&'static str, OpcodeDescription> {
                let mut result = HashMap::new();
                $(
                    {
                        #[allow(unused_mut)]
                        let mut arguments = Vec::<Argument>::new();

                        macro_rules! push_target {
                            () => {};
                            (target_address) => {
                                arguments.push(Argument::Address);
                            };
                            (source_address) => {};
                            (immediate) => {};
                        }
                        push_target!($($type)?);

                        $(
                            arguments.push(Argument::Register(
                                RegisterUsage::$register_usage,
                                stringify!($register_letter),
                                stringify!($register_name)
                            ));
                        )*

                        macro_rules! push_source {
                            () => {};
                            (source_address) => {
                                arguments.push(Argument::Address);
                            };
                            (immediate) => {
                                arguments.push(Argument::Immediate);
                            };
                            (target_address) => {};
                        }
                        push_source!($($type)?);

                        result.insert(stringify!($identifier), OpcodeDescription{
                            opcode: $code,
                            arguments,
                            opcode_type: type_to_opcode_type!($($type)?),
                            cycles: $num_cycles,
                            should_increment: matches!(Increment::$should_increment, Increment::Yes),
                            docstring: $comment,
                        });
                    }
                )+
                result
            }

            pub fn as_instruction(self) -> Instruction {
                match self {
                    $(
                        Self::$identifier{ $( $register_name, )* $($type)?} => (($code as Instruction) << Instruction::BITS - u16::BITS) | registers_to_instruction!($( $register_name ),*) $(| $type as Instruction)?,
                    )+
                }
            }

            pub fn should_increment_instruction_pointer(self) -> bool {
                match self {
                    $(
                        Self::$identifier{ .. } => matches!(Increment::$should_increment, Increment::Yes),
                    )+
                }
            }

            pub fn get_num_cycles(self) -> u8 {
                match self {
                    $(
                        Self::$identifier{ .. } => $num_cycles,
                    )+
                }
            }
        }

        impl TryFrom<Instruction> for Opcode {
            type Error = &'static str;

            fn try_from(value: Instruction) -> Result<Self, Self::Error> {
                #![allow(clippy::mixed_read_write_in_expression)]
                let opcode = value.as_words().0.as_halfwords().0;
                let register_values = &value.to_be_bytes()[2..];
                let mut registers = [Register(0); 6];
                for (i, register) in registers.iter_mut().enumerate() {
                    *register = Register(register_values[i]);
                }
                let immediate = value.as_words().1;
                let address = immediate;
                macro_rules! address_or_immediate {
                    ( immediate ) => { immediate };
                    ( source_address ) => { address };
                    ( target_address ) => { address };
                }
                #[deny(unreachable_patterns)]
                match opcode {
                    $(
                        $code => {
                            let mut _register_index = 0;
                            Ok(Self::$identifier{
                            $(
                                $register_name: registers[{
                                    let old_index = _register_index;
                                    _register_index += 1;
                                    old_index
                                }],
                            )*
                            $( $type: address_or_immediate!($type) )?
                        })},
                    )*
                    _ => Err("Invalid opcode")
                }
            }
        }
    };
}

opcodes!(
    // move instructions
    { MoveRegisterImmediate, 0x0000, registers(Target R register), immediate; cycles = 1, Increment::Yes, "move the value C into register R" },
    { MoveRegisterAddress, 0x0001, registers(Target R register), source_address; cycles = 1, Increment::Yes, "move the value at address A into register R" },
    { MoveTargetSource, 0x0002, registers(Target T target, Source S source); cycles = 1, Increment::Yes, "move the contents of register S into register T" },
    { MoveAddressRegister, 0x0003, registers(Source R register), target_address; cycles = 1, Increment::Yes, "move the contents of register R into memory at address A" },
    { MoveTargetPointer, 0x0004, registers(Target T target, Source P pointer); cycles = 1, Increment::Yes, "move the contents addressed by the value of register P into register T" },
    { MovePointerSource, 0x0005, registers(Target P pointer, Source S source); cycles = 1, Increment::Yes, "move the contents of register S into memory at address specified by register P" },
    // move instructions for byte-sized access
    { MoveByteRegisterAddress, 0x0041, registers(Target R register), source_address; cycles = 1, Increment::Yes, "move the value at address A into register R (1 byte)"},
    { MoveByteAddressRegister, 0x0042, registers(Source R register), target_address; cycles = 1, Increment::Yes, "move the contents of register R into memory at address A (1 byte)" },
    { MoveByteTargetPointer, 0x0043, registers(Target T target, Source P pointer); cycles = 1, Increment::Yes, "move the contents addressed by the value of register P into register T (1 byte)" },
    { MoveBytePointerSource, 0x0044, registers(Target P pointer, Source S source); cycles = 1, Increment::Yes, "move the contents of register S into memory at address specified by register P (1 byte)" },
    // move instructions for halfword-sized access
    { MoveHalfwordRegisterAddress, 0x0045, registers(Target R register), source_address; cycles = 1, Increment::Yes, "move the value at address A into register R (2 bytes)"},
    { MoveHalfwordAddressRegister, 0x0046, registers(Source R register), target_address; cycles = 1, Increment::Yes, "move the contents of register R into memory at address A (2 bytes)" },
    { MoveHalfwordTargetPointer, 0x0047, registers(Target T target, Source P pointer); cycles = 1, Increment::Yes, "move the contents addressed by the value of register P into register T (2 bytes)" },
    { MoveHalfwordPointerSource, 0x0048, registers(Target P pointer, Source S source); cycles = 1, Increment::Yes, "move the contents of register S into memory at address specified by register P (2 bytes)" },
    // offset move-instructions
    { MovePointerSourceOffset, 0x0049, registers(Target P pointer, Source S source), immediate; cycles = 1, Increment::Yes, "move the value in register S into memory at address pointer + immediate" },
    { MoveBytePointerSourceOffset, 0x004A, registers(Target P pointer, Source S source), immediate; cycles = 1, Increment::Yes, "move the value in register S into memory at address pointer + immediate (1 byte)" },
    { MoveHalfwordPointerSourceOffset, 0x004B, registers(Target P pointer, Source S source), immediate; cycles = 1, Increment::Yes, "move the value in register S into memory at address pointer + immediate (2 bytes)" },
    { MoveTargetPointerOffset, 0x004C, registers(Target T target, Source P pointer), immediate; cycles = 1, Increment::Yes, "move the contents addressed by the sum of the pointer and the immediate into the register T" },
    { MoveByteTargetPointerOffset, 0x004D, registers(Target T target, Source P pointer), immediate; cycles = 1, Increment::Yes, "move the contents addressed by the sum of the pointer and the immediate into the register T" },
    { MoveHalfwordTargetPointerOffset, 0x004E, registers(Target T target, Source P pointer), immediate; cycles = 1, Increment::Yes, "move the contents addressed by the sum of the pointer and the immediate into the register T" },

    // halt and catch fire
    { HaltAndCatchFire, 0x0006, registers(); cycles = 1, Increment::No, "halt and catch fire" },

    // artimetic (sic!) instructions
    { AddTargetLhsRhs, 0x0007, registers(Target T target, Source L lhs, Source R rhs); cycles = 1, Increment::Yes, "add the values in registers L and R, store the result in T, set zero and carry flags appropriately" },
    { AddWithCarryTargetLhsRhs, 0x0034, registers(Target T target, Source L lhs, Source R rhs); cycles = 1, Increment::Yes, "add (with carry) the values in registers L and R, store the result in T, set zero and carry flags appropriately" },
    { SubtractTargetLhsRhs, 0x0008, registers(Target T target, Source L lhs, Source R rhs); cycles = 1, Increment::Yes, "subtract (without carry) the values in registers L and R, store the result in T, set zero and carry flags appropriately" },
    { SubtractWithCarryTargetLhsRhs, 0x0009, registers(Target T target, Source L lhs, Source R rhs); cycles = 1, Increment::Yes, "subtract (with carry) the values in registers L and R, store the result in T, set zero and carry flags appropriately" },
    { MultiplyHighLowLhsRhs, 0x000A, registers(Target H high, Target T low, Source L lhs, Source R rhs); cycles = 1, Increment::Yes, "multiply the values in registers L and R, store the low part of the result in T, the high part in H, set zero and carry flags appropriately" },
    { DivmodTargetModLhsRhs, 0x000B, registers(Target D result, Target M remainder, Source L lhs, Source R rhs); cycles = 1, Increment::Yes, "divmod the values in registers L and R, store the result in D and the remainder in M set zero and divide-by-zero flags appropriately" },

    // bitwise instructions
    { AndTargetLhsRhs, 0x000C, registers(Target T target, Source L lhs, Source R rhs); cycles = 1, Increment::Yes, "and the values in registers LL and RR, store the result in TT, set zero flag appropriately" },
    { OrTargetLhsRhs, 0x000D, registers(Target T target, Source L lhs, Source R rhs); cycles = 1, Increment::Yes, "or the values in registers LL and RR, store the result in TT, set zero flag appropriately" },
    { XorTargetLhsRhs, 0x000E, registers(Target T target, Source L lhs, Source R rhs); cycles = 1, Increment::Yes, "xor the values in registers LL and RR, store the result in TT, set zero flag appropriately" },
    { NotTargetSource, 0x000F, registers(Target T target, Source S source); cycles = 1, Increment::Yes, "not the value in register SS, store the result in TT, set zero flag appropriately" },
    { LeftShiftTargetLhsRhs, 0x0010, registers(Target T target, Source L lhs, Source R rhs); cycles = 1, Increment::Yes, "left shift the value in register LL by RR bits, store the result in TT, set zero and carry flags appropriately" },
    { RightShiftTargetLhsRhs, 0x0011, registers(Target T target, Source L lhs, Source R rhs); cycles = 1, Increment::Yes, "right shift the value in register LL by RR bits, store the result in TT, set zero and carry flags appropriately" },
    { AddTargetSourceImmediate, 0x0012, registers(Target T target, Source S source), immediate; cycles = 1, Increment::Yes, "add the constant CC to the value in register SS and store the result in TT, set zero and carry flags appropriately" },
    { SubtractTargetSourceImmediate, 0x0013, registers(Target T target, Source S source), immediate; cycles = 1, Increment::Yes, "subtract the constant CC from the value in register SS and store the result in TT, set zero and carry flags appropriately" },

    // comparison
    { CompareTargetLhsRhs, 0x0014, registers(Target T target, Source L lhs, Source R rhs); cycles = 1, Increment::Yes, "compare the values in registers LL and RR, store the result (Word::MAX, 0, 1) in TT, set zero flag appropriately" },
    { BoolCompareEquals, 0x003A, registers(Target T target, Source L lhs, Source R rhs); cycles = 1, Increment::Yes, "checks whether the values in registers L and R are equal and stores the result as boolean (0 or 1) in T" },
    { BoolCompareNotEquals, 0x003B, registers(Target T target, Source L lhs, Source R rhs); cycles = 1, Increment::Yes, "checks whether the values in registers L and R are not equal and stores the result as boolean (0 or 1) in T" },
    { BoolCompareGreater, 0x003C, registers(Target T target, Source L lhs, Source R rhs); cycles = 1, Increment::Yes, "checks whether the value in registers L is greater than the value in regsiter R and stores the result as boolean (0 or 1) in T" },
    { BoolCompareGreaterOrEquals, 0x003D, registers(Target T target, Source L lhs, Source R rhs); cycles = 1, Increment::Yes, "checks whether the value in registers L is greater than or equals the value in regsiter R and stores the result as boolean (0 or 1) in T" },
    { BoolCompareLess, 0x003E, registers(Target T target, Source L lhs, Source R rhs); cycles = 1, Increment::Yes, "checks whether the value in registers L is less than the value in regsiter R and stores the result as boolean (0 or 1) in T" },
    { BoolCompareLessOrEquals, 0x003F, registers(Target T target, Source L lhs, Source R rhs); cycles = 1, Increment::Yes, "checks whether the value in registers L is less than or equals the value in regsiter R and stores the result as boolean (0 or 1) in T" },

    // stack instructions
    { PushRegister, 0x0015, registers(Source R register); cycles = 1, Increment::Yes, "pushes the value of register RR onto the stack" },
    { PopRegister, 0x0016, registers(Target R register); cycles = 1, Increment::Yes, "pops from the stack and stores the value in register RR" },
    { Pop, 0x0040, registers(); cycles = 1, Increment::Yes, "pops from the stack and discards the value" },
    { CallAddress, 0x0017, registers(), source_address; cycles = 1, Increment::No, "push the current instruction pointer onto the stack and jump to the specified address" },
    { CallRegister, 0x0036, registers(Source R register); cycles = 1, Increment::No, "push the current instruction pointer onto the stack and jump to the address stored in register R" },
    { CallPointer, 0x0037, registers(Source P pointer); cycles = 1, Increment::No, "push the current instruction pointer onto the stack and jump to the address stored in memory at the location specified by the value in register P" },
    { Return, 0x0018, registers(); cycles = 1, Increment::No, "pop the return address from the stack and jump to it" },

    // unconditional jumps
    { JumpImmediate, 0x0019, registers(), immediate; cycles = 1, Increment::No, "jump to the given address" },
    { JumpRegister, 0x001A, registers(Source R register); cycles = 1, Increment::No, "jump to the address stored in register R" },

    // conditional jumps, address given as immediate
    { JumpImmediateIfEqual, 0x001B, registers(Source C comparison), immediate; cycles = 1, Increment::No, "jump to the specified address if the comparison result in register C corresponds to \"equality\"" },
    { JumpImmediateIfGreaterThan, 0x001C, registers(Source C comparison), immediate; cycles = 1, Increment::No, "jump to the specified address if the comparison result in register C corresponds to \"greater than\"" },
    { JumpImmediateIfLessThan, 0x001D, registers(Source C comparison), immediate; cycles = 1, Increment::No, "jump to the specified address if the comparison result in register C corresponds to \"less than\"" },
    { JumpImmediateIfGreaterThanOrEqual, 0x001E, registers(Source C comparison), immediate; cycles = 1, Increment::No, "jump to the specified address if the comparison result in register C corresponds to \"greater than\" or \"equal\"" },
    { JumpImmediateIfLessThanOrEqual, 0x001F, registers(Source C comparison), immediate; cycles = 1, Increment::No, "jump to the specified address if the comparison result in register C corresponds to \"less than\" or \"equal\"" },
    { JumpImmediateIfZero, 0x0020, registers(), immediate; cycles = 1, Increment::No, "jump to the specified address if the zero flag is set" },
    { JumpImmediateIfNotZero, 0x0021, registers(), immediate; cycles = 1, Increment::No, "jump to the specified address if the zero flag is not set" },
    { JumpImmediateIfCarry, 0x0022, registers(), immediate; cycles = 1, Increment::No, "jump to the specified address if the carry flag is set" },
    { JumpImmediateIfNotCarry, 0x0023, registers(), immediate; cycles = 1, Increment::No, "jump to the specified address if the carry flag is not set" },
    { JumpImmediateIfDivideByZero, 0x0024, registers(), immediate; cycles = 1, Increment::No, "jump to the specified address if the divide by zero flag is set" },
    { JumpImmediateIfNotDivideByZero, 0x0025, registers(), immediate; cycles = 1, Increment::No, "jump to the specified address if the divide by zero flag is not set" },

    // conditional jumps, address given as register
    { JumpRegisterIfEqual, 0x0026, registers(Source P pointer, Source C comparison); cycles = 1, Increment::No, "jump to the address specified in register P if the comparison result in register C corresponds to \"equality\"" },
    { JumpRegisterIfGreaterThan, 0x0027, registers(Source P pointer, Source C comparison); cycles = 1, Increment::No, "jump to the address specified in register P if the comparison result in register C corresponds to \"greater than\"" },
    { JumpRegisterIfLessThan, 0x0028, registers(Source P pointer, Source C comparison); cycles = 1, Increment::No, "jump to the address specified in register P if the comparison result in register C corresponds to \"less than\"" },
    { JumpRegisterIfGreaterThanOrEqual, 0x0029, registers(Source P pointer, Source C comparison); cycles = 1, Increment::No, "jump to the address specified in register P if the comparison result in register C corresponds to \"greater than\" or \"equal\"" },
    { JumpRegisterIfLessThanOrEqual, 0x002A, registers(Source P pointer, Source C comparison); cycles = 1, Increment::No, "jump to the address specified in register P if the comparison result in register C corresponds to \"less than\" or \"equal\"" },
    { JumpRegisterIfZero, 0x002B, registers(Source P pointer); cycles = 1, Increment::No, "jump to the address specified in register P if the zero flag is set" },
    { JumpRegisterIfNotZero, 0x002C, registers(Source P pointer); cycles = 1, Increment::No, "jump to the address specified in register P if the zero flag is not set" },
    { JumpRegisterIfCarry, 0x002D, registers(Source P pointer); cycles = 1, Increment::No, "jump to the address specified in register P if the carry flag is set" },
    { JumpRegisterIfNotCarry, 0x002E, registers(Source P pointer); cycles = 1, Increment::No, "jump to the address specified in register P if the carry flag is not set" },
    { JumpRegisterIfDivideByZero, 0x002F, registers(Source P pointer); cycles = 1, Increment::No, "jump to the address specified in register P if the divide by zero flag is set" },
    { JumpRegisterIfNotDivideByZero, 0x0030, registers(Source P pointer); cycles = 1, Increment::No, "jump to the address specified in register P if the divide by zero flag is not set" },

    // no-op
    { NoOp, 0x0031, registers(); cycles = 1, Increment::Yes, "does nothing" },

    // input
    { GetKeyState, 0x0032, registers(Target T target, Source K keycode); cycles = 1, Increment::Yes, "store the keystate (1 = held down, 0 = not held down) of the key specified by register K into register T and set the zero flag appropriately" },

    // Timing
    { PollTime, 0x0033, registers(Target H high, Target L low); cycles = 1, Increment::Yes, "store the number of milliseconds since the UNIX epoch into registers high and low" },

    // Rendering
    { SwapFramebuffers, 0x0035, registers(); cycles = 1, Increment::Yes, "swap the display buffers" },
    { InvisibleFramebufferAddress, 0x0038, registers(Target T target); cycles = 1, Increment::Yes, "get the start address of the framebuffer that's currently invisible (use the address to draw without tearing)" },

    // Debugging and profiling
    { PollCycleCountHighLow, 0x0039, registers(Target H high, Target L low); cycles = 1, Increment::Yes, "store the current cycle (64 bit value) count into registers H and L (H: most significant bytes, L: least significant bytes)" },
    { DumpRegisters, 0xFFFF, registers(); cycles = 1, Increment::Yes, "dump the contents of all registers into the file 'registers_YYYY-MM-DD_X.bin' where YYYY-MM-DD is the current date and X is an increasing number" },
    { DumpMemory, 0xFFFE, registers(); cycles = 1, Increment::Yes, "dump the contents of the whole memory into the file 'memory_YYYY-MM-DD_X.bin' where YYYY-MM-DD is the current date and X is an increasing number" },
    { AssertRegisterRegister, 0xFFFD, registers(Source E expected, Source A actual); cycles = 1, Increment::Yes, "assert that the expected register value equals the actual register value (behavior of the VM on a failed assertion is implementation defined)" },
    { AssertRegisterImmediate, 0xFFFC, registers(Source A actual), immediate; cycles = 1, Increment::Yes, "assert that the actual register value equals the immediate (behavior of the VM on a failed assertion is implementation defined)"},
    { AssertPointerImmediate, 0xFFFB, registers(Source P pointer), immediate; cycles = 1, Increment::Yes, "assert that the value in memory pointed at by P equals the immediate (behavior of the VM on a failed assertion is implementation defined)"},
    { DebugBreak, 0xFFFA, registers(); cycles = 1, Increment::Yes, "behavior is implementation defined" },
    { PrintRegister, 0xFFF9, registers(Source R register); cycles = 1, Increment::Yes, "prints the value of the register as debug output"},
    { Checkpoint, 0xFFF8, registers(), immediate; cycles = 1, Increment::Yes, "makes the emulator check the value of the internal checkpoint counter, fails on mismatch" },
);
