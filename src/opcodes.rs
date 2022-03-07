use crate::{Address, Instruction, Register, Word};

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
    ( $($register:ident),*) => {
        {
            let mut _shift = 40;
            #[allow(unused_mut)]
            let mut result = 0;
            $(
                result |= ($register.0 as Instruction) << _shift;
                _shift -= 8;
            )*
            result
        }
    };
}

macro_rules! opcodes {
    ( $({
        $identifier:ident,
        $code:literal,
        register( $( $register_name:ident ),* ) $(, $type:ident )?,
        $comment:literal
    },)+ ) => {
        /// ## Opcodes
        /// | Opcode                | Meaning                                   |
        /// |-----------------------|-------------------------------------------|
        $(
            #[doc = concat!(" | `", stringify!($code), "\u{00a0}", stringify_registers!(($( $register_name ),*) $(, $type)?), "` | ", $comment, " |")]
        )+
        #[derive(Clone, Copy, Debug)]
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
        }
    };
}

opcodes!(
    { MoveRegisterImmediate, 0x0000,  register(r), immediate, "move the value C into register R" },
    { MoveRegisterAddress, 0x0001,  register(r), address, "move the value at address A into register R" },
    { MoveTargetSource, 0x0002, register(t, s), "move the contents of register S into register T" },
    { MoveAddressRegister, 0x0003, register(r), address, "move the contents of register R into memory at address A" },
    { MoveTargetPointer, 0x0004, register(t, p), "move the contents addressed by the value of register P into register T" },
    { MovePointerSource, 0x0005, register(p, s), "move the contents of register S into memory at address specified by register P" },
    { HaltAndCatchFire, 0x0006, register(), "halt and catch fire" },
    { AddTargetLhsRhs, 0x0007, register(t, l, r), "add the values in registers LL and RR, store the result in TT, set zero and carry flags appropriately" },
    { SubtractTargetLhsRhs, 0x0008, register(t, l, r), "subtract (without carry) the values in registers LL and RR, store the result in TT, set zero and carry flags appropriately" },
    { SubtractWithCarryTargetLhsRhs, 0x0009, register(t, l, r), "subtract (with carry) the values in registers LL and RR, store the result in TT, set zero and carry flags appropriately" },
    { MultiplyHighLowLhsRhs, 0x000A, register(h, t, l, r), "multiply the values in registers LL and RR, store the low part of the result in TT, the high part in HH, set zero and carry flags appropriately" },
    { DivmodTargetModLhsRhs, 0x000B, register(d, m, l, r), "divmod the values in registers LL and RR, store the result in DD and the remainder in MM set zero and divide-by-zero flags appropriately" },
    { AndTargetLhsRhs, 0x000C, register(t, l, r), "and the values in registers LL and RR, store the result in TT, set zero flag appropriately" },
    { OrTargetLhsRhs, 0x000D, register(t, l, r), "or the values in registers LL and RR, store the result in TT, set zero flag appropriately" },
    { XorTargetLhsRhs, 0x000E, register(t, l, r), "xor the values in registers LL and RR, store the result in TT, set zero flag appropriately" },
    { NotTargetSource, 0x000F, register(t, s), "not the value in register SS, store the result in TT, set zero flag appropriately" },
    { LeftShiftTargetLhsRhs, 0x0010, register(t, l, r), "left shift the value in register LL by RR bits, store the result in TT, set zero and carry flags appropriately" },
    { RightShiftTargetLhsRhs, 0x0011, register(t, l, r), "right shift the value in register LL by RR bits, store the result in TT, set zero and carry flags appropriately" },
    { AddTargetSourceImmediate, 0x0012, register(t, s), immediate, "add the constant CC to the value in register SS and store the result in TT, set zero and carry flags appropriately" },
    { SubtractTargetSourceImmediate, 0x0013, register(t, s), immediate, "subtract the constant CC from the value in register SS and store the result in TT, set zero and carry flags appropriately" },
    { CompareTargetLhsRhs, 0x0014, register(t, l, r), "compare the values in registers LL and RR, store the result (Word::MAX, 0, 1) in TT, set zero flag appropriately" },
    { PushRegister, 0x0015, register(r), "push the value of register RR onto the stack" },
    { PopRegister, 0x0016, register(r), "pop from the stack and store the value in register RR" },
);
