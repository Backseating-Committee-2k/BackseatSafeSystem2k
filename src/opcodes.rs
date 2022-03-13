use crate::{Address, AsHalfWords, AsWords, Instruction, Register, Word};

macro_rules! type_to_abbreviation {
    (immediate) => {
        "cccc\u{00a0}cccc"
    };
    (address) => {
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
    (address) => {
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

macro_rules! opcodes {
    ( $({
        $identifier:ident,
        $code:literal,
        register( $( $register_letter:ident $register_name:ident ),* ) $(, $type:ident )? ;
        Increment::$should_increment:ident,
        $comment:literal
    },)+ ) => {
        enum Increment {
            Yes,
            No,
        }

        /// ## Opcodes
        /// | Opcode                | Meaning                                   |
        /// |-----------------------|-------------------------------------------|
        $(
            #[doc = concat!(" | `", stringify!($code), "\u{00a0}", stringify_registers!(($( $register_letter ),*) $(, $type)?), "` | ", $comment, " |")]
        )+
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #[repr(u32)] // for the speeds (blame: slartibart)
        pub enum Opcode {
            $(
                $identifier{ $( $register_name : Register, )* $($type : type_to_datatype!($type))? },
            )+
        }

        impl Opcode {
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
        }

        impl TryFrom<Instruction> for Opcode {
            type Error = &'static str;

            fn try_from(value: Instruction) -> Result<Self, Self::Error> {
                #![allow(clippy::eval_order_dependence)]
                let opcode = value.as_words().0.as_half_words().0;
                let register_values = &value.to_be_bytes()[2..];
                let mut registers = [Register(0); 6];
                for (i, register) in registers.iter_mut().enumerate() {
                    *register = Register(register_values[i]);
                }
                let immediate = value.as_words().1;
                let address = immediate;
                macro_rules! address_or_immediate {
                    ( immediate ) => { immediate };
                    ( address ) => { address };
                }
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
    { MoveRegisterImmediate, 0x0000, register(R register), immediate; Increment::Yes, "move the value C into register R" },
    { MoveRegisterAddress, 0x0001, register(R register), address; Increment::Yes, "move the value at address A into register R" },
    { MoveTargetSource, 0x0002, register(T target, S source); Increment::Yes, "move the contents of register S into register T" },
    { MoveAddressRegister, 0x0003, register(R register), address; Increment::Yes, "move the contents of register R into memory at address A" },
    { MoveTargetPointer, 0x0004, register(T target, P pointer); Increment::Yes, "move the contents addressed by the value of register P into register T" },
    { MovePointerSource, 0x0005, register(P pointer, S source); Increment::Yes, "move the contents of register S into memory at address specified by register P" },

    // halt and catch fire
    { HaltAndCatchFire, 0x0006, register(); Increment::No, "halt and catch fire" },

    // arithmetic instructions
    { AddTargetLhsRhs, 0x0007, register(T target, L lhs, R rhs); Increment::Yes, "add the values in registers LL and RR, store the result in TT, set zero and carry flags appropriately" },
    { SubtractTargetLhsRhs, 0x0008, register(T target, L lhs, R rhs); Increment::Yes, "subtract (without carry) the values in registers LL and RR, store the result in TT, set zero and carry flags appropriately" },
    { SubtractWithCarryTargetLhsRhs, 0x0009, register(T target, L lhs, R rhs); Increment::Yes, "subtract (with carry) the values in registers LL and RR, store the result in TT, set zero and carry flags appropriately" },
    { MultiplyHighLowLhsRhs, 0x000A, register(H high, T low, L lhs, R rhs); Increment::Yes, "multiply the values in registers LL and RR, store the low part of the result in TT, the high part in HH, set zero and carry flags appropriately" },
    { DivmodTargetModLhsRhs, 0x000B, register(D result, M remainder, L lhs, R rhs); Increment::Yes, "divmod the values in registers LL and RR, store the result in DD and the remainder in MM set zero and divide-by-zero flags appropriately" },

    // bitwise instructions
    { AndTargetLhsRhs, 0x000C, register(T target, L lhs, R rhs); Increment::Yes, "and the values in registers LL and RR, store the result in TT, set zero flag appropriately" },
    { OrTargetLhsRhs, 0x000D, register(T target, L lhs, R rhs); Increment::Yes, "or the values in registers LL and RR, store the result in TT, set zero flag appropriately" },
    { XorTargetLhsRhs, 0x000E, register(T target, L lhs, R rhs); Increment::Yes, "xor the values in registers LL and RR, store the result in TT, set zero flag appropriately" },
    { NotTargetSource, 0x000F, register(T target, S source); Increment::Yes, "not the value in register SS, store the result in TT, set zero flag appropriately" },
    { LeftShiftTargetLhsRhs, 0x0010, register(T target, L lhs, R rhs); Increment::Yes, "left shift the value in register LL by RR bits, store the result in TT, set zero and carry flags appropriately" },
    { RightShiftTargetLhsRhs, 0x0011, register(T target, L lhs, R rhs); Increment::Yes, "right shift the value in register LL by RR bits, store the result in TT, set zero and carry flags appropriately" },
    { AddTargetSourceImmediate, 0x0012, register(T target, S source), immediate; Increment::Yes, "add the constant CC to the value in register SS and store the result in TT, set zero and carry flags appropriately" },
    { SubtractTargetSourceImmediate, 0x0013, register(T target, S source), immediate; Increment::Yes, "subtract the constant CC from the value in register SS and store the result in TT, set zero and carry flags appropriately" },

    // comparison
    { CompareTargetLhsRhs, 0x0014, register(T target, L lhs, R rhs); Increment::Yes, "compare the values in registers LL and RR, store the result (Word::MAX, 0, 1) in TT, set zero flag appropriately" },

    // stack instructions
    { PushRegister, 0x0015, register(R register); Increment::Yes, "push the value of register RR onto the stack" },
    { PopRegister, 0x0016, register(R register); Increment::Yes, "pop from the stack and store the value in register RR" },
    { CallAddress, 0x0017, register(), address; Increment::No, "push the current instruction pointer onto the stack and jump to the specified address" },
    { Return, 0x0018, register(); Increment::No, "pop the return address from the stack and jump to it" },

    // unconditional jumps
    { JumpAddress, 0x0019, register(), address; Increment::No, "jump to the given address" },
    { JumpRegister, 0x001A, register(R register); Increment::No, "jump to the address stored in register R" },

    // conditional jumps, address given as immediate
    { JumpAddressIfEqual, 0x001B, register(C comparison), address; Increment::No, "jump to the specified address if the comparison result in register C corresponds to \"equality\"" },
    { JumpAddressIfGreaterThan, 0x001C, register(C comparison), address; Increment::No, "jump to the specified address if the comparison result in register C corresponds to \"greater than\"" },
    { JumpAddressIfLessThan, 0x001D, register(C comparison), address; Increment::No, "jump to the specified address if the comparison result in register C corresponds to \"less than\"" },
    { JumpAddressIfGreaterThanOrEqual, 0x001E, register(C comparison), address; Increment::No, "jump to the specified address if the comparison result in register C corresponds to \"greater than\" or \"equal\"" },
    { JumpAddressIfLessThanOrEqual, 0x001F, register(C comparison), address; Increment::No, "jump to the specified address if the comparison result in register C corresponds to \"less than\" or \"equal\"" },
    { JumpAddressIfZero, 0x0020, register(), address; Increment::No, "jump to the specified address if the zero flag is set" },
    { JumpAddressIfNotZero, 0x0021, register(), address; Increment::No, "jump to the specified address if the zero flag is not set" },
    { JumpAddressIfCarry, 0x0022, register(), address; Increment::No, "jump to the specified address if the carry flag is set" },
    { JumpAddressIfNotCarry, 0x0023, register(), address; Increment::No, "jump to the specified address if the carry flag is not set" },
    { JumpAddressIfDivideByZero, 0x0024, register(), address; Increment::No, "jump to the specified address if the divide by zero flag is set" },
    { JumpAddressIfNotDivideByZero, 0x0025, register(), address; Increment::No, "jump to the specified address if the divide by zero flag is not set" },

    // conditional jumps, address given as register
    { JumpRegisterIfEqual, 0x0026, register(P pointer, C comparison); Increment::No, "jump to the address specified in register P if the comparison result in register C corresponds to \"equality\"" },
    { JumpRegisterIfGreaterThan, 0x0027, register(P pointer, C comparison); Increment::No, "jump to the address specified in register P if the comparison result in register C corresponds to \"greater than\"" },
    { JumpRegisterIfLessThan, 0x0028, register(P pointer, C comparison); Increment::No, "jump to the address specified in register P if the comparison result in register C corresponds to \"less than\"" },
    { JumpRegisterIfGreaterThanOrEqual, 0x0029, register(P pointer, C comparison); Increment::No, "jump to the address specified in register P if the comparison result in register C corresponds to \"greater than\" or \"equal\"" },
    { JumpRegisterIfLessThanOrEqual, 0x002A, register(P pointer, C comparison); Increment::No, "jump to the address specified in register P if the comparison result in register C corresponds to \"less than\" or \"equal\"" },
    { JumpRegisterIfZero, 0x002B, register(P pointer); Increment::No, "jump to the address specified in register P if the zero flag is set" },
    { JumpRegisterIfNotZero, 0x002C, register(P pointer); Increment::No, "jump to the address specified in register P if the zero flag is not set" },
    { JumpRegisterIfCarry, 0x002D, register(P pointer); Increment::No, "jump to the address specified in register P if the carry flag is set" },
    { JumpRegisterIfNotCarry, 0x002E, register(P pointer); Increment::No, "jump to the address specified in register P if the carry flag is not set" },
    { JumpRegisterIfDivideByZero, 0x002F, register(P pointer); Increment::No, "jump to the address specified in register P if the divide by zero flag is set" },
    { JumpRegisterIfNotDivideByZero, 0x0030, register(P pointer); Increment::No, "jump to the address specified in register P if the divide by zero flag is not set" },

    // no-op
    { NoOp, 0x0031, register(); Increment::Yes, "does nothing" },
);
