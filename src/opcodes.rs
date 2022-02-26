//! ## Opcodes
//! | Opcode                | Meaning                                   |
//! |-----------------------|-------------------------------------------|
//! | `0000 RR__ CCCC CCCC` | move the value C into register R |
//! | `0001 RR__ AAAA AAAA` | move the value at address A into register R |
//! | `0002 TTSS ____ ____` | move the contents of register S into register T |
//! | `0003 RR__ AAAA AAAA` | move the contents of register R into memory at address A |
//! | `0004 TTPP ____ ____` | move the contents addressed by the value of register P into register T |
//! | `0005 PPSS ____ ____` | move the contents of register S into memory at address specified by register P |
//! | `0006 ____ ____ ____` | halt and catch fire |
//! | `0007 TTLL RR__ ____` | add the values in registers LL and RR, store the result in TT, set zero and carry flags appropriately |
//! | `0008 TTLL RR__ ____` | subtract (without carry) the values in registers LL and RR, store the result in TT, set zero and carry flags appropriately |
//! | `0009 TTLL RR__ ____` | subtract (with carry) the values in registers LL and RR, store the result in TT, set zero and carry flags appropriately |
//! | `000A HHTT LLRR ____` | multiply the values in registers LL and RR, store the low part of the result in TT, the high part in HH, set zero and carry flags appropriately |
//! | `000B DDMM LLRR ____` | divmod the values in registers LL and RR, store the result in DD and the remainder in MM set zero and carry flags appropriately |
//! | `000C TTLL RR__ ____` | and the values in registers LL and RR, store the result in TT, set zero and carry flags appropriately |
//! | `000D TTLL RR__ ____` | or the values in registers LL and RR, store the result in TT, set zero and carry flags appropriately |
//! | `000E TTLL RR__ ____` | xor the values in registers LL and RR, store the result in TT, set zero and carry flags appropriately |
//! | `000F TTSS ____ ____` | not the values in registers LL and RR, store the result in TT, set zero and carry flags appropriately |
//! | `0010 TTLL RR__ ____` | left shift the values in registers LL and RR, store the result in TT, set zero and carry flags appropriately |
//! | `0011 TTLL RR__ ____` | right shift the values in registers LL and RR, store the result in TT, set zero and carry flags appropriately |
//! | `0012 TTSS CCCC CCCC` | add the constant CC to the value in register SS and store the result in TT, set zero and carry flags appropriately |
//! | `0013 TTSS CCCC CCCC` | subtract the constant CC from the value in register SS ans store the result in TT, set zero and carry flags appropriately |
//! | `0014 TTLL RR__ ____` | compare shift the values in registers LL and RR, store the result in TT, set zero and carry flags appropriately |

use crate::{Address, Instruction, Register, Word};

pub fn move_register_immediate(register: Register, value: Word) -> Instruction {
    (register.0 as Instruction) << 40 | value as Instruction
}

pub fn move_register_address(register: Register, address: Address) -> Instruction {
    0x0001_0000_0000_0000 | (register.0 as Instruction) << 40 | address as Instruction
}

pub fn move_target_source(target: Register, source: Register) -> Instruction {
    0x0002_0000_0000_0000 | (target.0 as Instruction) << 40 | (source.0 as Instruction) << 32
}

pub fn move_address_register(address: Address, register: Register) -> Instruction {
    0x0003_0000_0000_0000 | (register.0 as Instruction) << 40 | address as Instruction
}

pub fn move_target_pointer(target: Register, pointer: Register) -> Instruction {
    0x0004_0000_0000_0000 | (target.0 as Instruction) << 40 | (pointer.0 as Instruction) << 32
}

pub fn move_pointer_source(pointer: Register, source: Register) -> Instruction {
    0x0005_0000_0000_0000 | (pointer.0 as Instruction) << 40 | (source.0 as Instruction) << 32
}

pub fn halt_and_catch_fire() -> Instruction {
    0x0006_0000_0000_0000
}

pub fn add_target_lhs_rhs(target: Register, lhs: Register, rhs: Register) -> Instruction {
    0x0007_0000_0000_0000
        | (target.0 as Instruction) << 40
        | (lhs.0 as Instruction) << 32
        | (rhs.0 as Instruction) << 24
}

pub fn subtract_target_lhs_rhs(target: Register, lhs: Register, rhs: Register) -> Instruction {
    0x0008_0000_0000_0000
        | (target.0 as Instruction) << 40
        | (lhs.0 as Instruction) << 32
        | (rhs.0 as Instruction) << 24
}

pub fn subtract_with_carry_target_lhs_rhs(
    target: Register,
    lhs: Register,
    rhs: Register,
) -> Instruction {
    0x0009_0000_0000_0000
        | (target.0 as Instruction) << 40
        | (lhs.0 as Instruction) << 32
        | (rhs.0 as Instruction) << 24
}

pub fn multiply_target_lhs_rhs(
    target_high: Register,
    target_low: Register,
    lhs: Register,
    rhs: Register,
) -> Instruction {
    0x000A_0000_0000_0000
        | (target_high.0 as Instruction) << 40
        | (target_low.0 as Instruction) << 32
        | (lhs.0 as Instruction) << 24
        | (rhs.0 as Instruction) << 16
}

pub fn divmod_target_mod_lhs_rhs(
    target: Register,
    mod_: Register,
    lhs: Register,
    rhs: Register,
) -> Instruction {
    0x000B_0000_0000_0000
        | (target.0 as Instruction) << 40
        | (mod_.0 as Instruction) << 32
        | (lhs.0 as Instruction) << 24
        | (rhs.0 as Instruction) << 16
}

pub fn and_target_lhs_rhs(target: Register, lhs: Register, rhs: Register) -> Instruction {
    0x000C_0000_0000_0000
        | (target.0 as Instruction) << 40
        | (lhs.0 as Instruction) << 32
        | (rhs.0 as Instruction) << 24
}

pub fn or_target_lhs_rhs(target: Register, lhs: Register, rhs: Register) -> Instruction {
    0x000D_0000_0000_0000
        | (target.0 as Instruction) << 40
        | (lhs.0 as Instruction) << 32
        | (rhs.0 as Instruction) << 24
}

pub fn xor_target_lhs_rhs(target: Register, lhs: Register, rhs: Register) -> Instruction {
    0x000E_0000_0000_0000
        | (target.0 as Instruction) << 40
        | (lhs.0 as Instruction) << 32
        | (rhs.0 as Instruction) << 24
}

pub fn not_target_lhs_rhs(target: Register, source: Register) -> Instruction {
    0x000F_0000_0000_0000 | (target.0 as Instruction) << 40 | (source.0 as Instruction) << 32
}

pub fn left_shift_target_lhs_rhs(target: Register, lhs: Register, rhs: Register) -> Instruction {
    0x0010_0000_0000_0000
        | (target.0 as Instruction) << 40
        | (lhs.0 as Instruction) << 32
        | (rhs.0 as Instruction) << 24
}

pub fn right_shift_target_lhs_rhs(target: Register, lhs: Register, rhs: Register) -> Instruction {
    0x0011_0000_0000_0000
        | (target.0 as Instruction) << 40
        | (lhs.0 as Instruction) << 32
        | (rhs.0 as Instruction) << 24
}

pub fn add_target_source_immediate(target: Register, source: Register, value: Word) -> Instruction {
    0x0012_0000_0000_0000
        | (target.0 as Instruction) << 40
        | (source.0 as Instruction) << 32
        | value as Instruction
}

pub fn subtract_target_source_immediate(
    target: Register,
    source: Register,
    value: Word,
) -> Instruction {
    0x0013_0000_0000_0000
        | (target.0 as Instruction) << 40
        | (source.0 as Instruction) << 32
        | value as Instruction
}

pub fn compare_target_lhs_rhs(target: Register, lhs: Register, rhs: Register) -> Instruction {
    0x0014_0000_0000_0000
        | (target.0 as Instruction) << 40
        | (lhs.0 as Instruction) << 32
        | (rhs.0 as Instruction) << 24
}
