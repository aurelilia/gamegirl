use crate::{
    numutil::{NumExt, U32Ext},
    psx::PlayStation,
};

macro_rules! check_cache {
    ($me:ident) => {
        if $me.cache_isolated() {
            return;
        }
    };
}

pub type InstructionHandler = fn(ps: &mut PlayStation, inst: Inst);
type Lut = [InstructionHandler; 64];

const PRIMARY: Lut = PlayStation::primary_table();
const SECONDARY: Lut = PlayStation::secondary_table();

impl PlayStation {
    pub fn run_inst(&mut self, inst: u32) {
        let primary = inst.bits(26, 6);
        let handler = PRIMARY[primary.us()];
        handler(self, Inst(inst));
    }

    const fn primary_table() -> Lut {
        let mut lut: Lut = [Self::unknown_instruction; 64];
        lut[0x00] = Self::secondary;
        lut[0x01] = Self::bcondz;
        lut[0x02] = Self::j;
        lut[0x03] = Self::jal;
        lut[0x04] = Self::beq;
        lut[0x05] = Self::bne;
        lut[0x06] = Self::blez;
        lut[0x07] = Self::bgtz;

        lut[0x08] = Self::addi;
        lut[0x09] = Self::addiu;
        lut[0x0A] = Self::slti;
        lut[0x0B] = Self::sltiu;
        lut[0x0C] = Self::andi;
        lut[0x0D] = Self::ori;
        lut[0x0E] = Self::xori;
        lut[0x0F] = Self::lui;

        lut[0x10] = Self::cop0;
        lut[0x11] = Self::cop1;
        lut[0x12] = Self::cop2;
        lut[0x13] = Self::cop3;

        lut[0x20] = Self::lb;
        lut[0x21] = Self::lh;
        lut[0x22] = Self::lwl;
        lut[0x23] = Self::lw;
        lut[0x24] = Self::lbu;
        lut[0x25] = Self::lhu;
        lut[0x26] = Self::lwr;

        lut[0x28] = Self::sb;
        lut[0x29] = Self::sh;
        lut[0x2A] = Self::swl;
        lut[0x2B] = Self::sw;
        lut[0x2E] = Self::swr;

        lut[0x30] = Self::lwc0;
        lut[0x31] = Self::lwc1;
        lut[0x32] = Self::lwc2;
        lut[0x33] = Self::lwc3;
        lut[0x38] = Self::swc0;
        lut[0x39] = Self::swc1;
        lut[0x3A] = Self::swc2;
        lut[0x3B] = Self::swc3;

        lut
    }

    const fn secondary_table() -> Lut {
        let mut lut: Lut = [Self::unknown_instruction; 64];
        lut[0x00] = Self::sll;
        lut[0x02] = Self::srl;
        lut[0x03] = Self::sra;
        lut[0x04] = Self::sllv;
        lut[0x06] = Self::srlv;
        lut[0x07] = Self::srav;

        lut[0x08] = Self::jr;
        lut[0x09] = Self::jalr;
        lut[0x0C] = Self::syscall;
        lut[0x0D] = Self::break_;

        lut[0x10] = Self::mfhi;
        lut[0x11] = Self::mthi;
        lut[0x12] = Self::mflo;
        lut[0x13] = Self::mtlo;
        lut[0x18] = Self::mult;
        lut[0x19] = Self::multu;
        lut[0x1A] = Self::div;
        lut[0x1B] = Self::divu;

        lut
    }
}

// Utility
impl PlayStation {
    fn branch(&mut self, imm: i32) {
        // Always word-aligned
        let offs = imm << 2;
        // Account for Pc increment after instruction
        self.set_pc(self.cpu.pc.wrapping_add_signed(offs).wrapping_sub(4));
    }

    fn cache_isolated(&self) -> bool {
        self.cpu.cop0.sr.is_bit(16)
    }

    fn addr_with_imm(&self, inst: Inst) -> u32 {
        self.cpu.reg(inst.rs()).wrapping_add_signed(inst.imm16s())
    }
}

// Primary
impl PlayStation {
    fn bcondz(&mut self, inst: Inst) {
        todo!();
    }

    fn j(&mut self, inst: Inst) {
        let pc = (self.cpu.pc & 0xF000_0000) | (inst.imm26() << 2);
        self.set_pc(pc);
    }

    fn jal(&mut self, inst: Inst) {
        todo!();
    }

    fn beq(&mut self, inst: Inst) {
        if self.cpu.reg(inst.rs()) == self.cpu.reg(inst.rt()) {
            self.branch(inst.imm16s());
        }
    }

    fn bne(&mut self, inst: Inst) {
        if self.cpu.reg(inst.rs()) != self.cpu.reg(inst.rt()) {
            self.branch(inst.imm16s());
        }
    }

    fn blez(&mut self, inst: Inst) {
        todo!();
    }

    fn bgtz(&mut self, inst: Inst) {
        todo!();
    }

    fn addi(&mut self, inst: Inst) {
        let rt = self.cpu.reg(inst.rt()) as i32;
        let rs = self.cpu.reg(inst.rs()) as i32;
        let value = match rs.checked_add(rt) {
            Some(value) => value as u32,
            None => todo!("exceptions"),
        };
        self.cpu.set_reg(inst.rt(), value);
    }

    fn addiu(&mut self, inst: Inst) {
        let value = self.cpu.reg(inst.rs()).wrapping_add_signed(inst.imm16s());
        self.cpu.set_reg(inst.rt(), value);
    }

    fn slti(&mut self, inst: Inst) {
        todo!();
    }

    fn sltiu(&mut self, inst: Inst) {
        todo!();
    }

    fn andi(&mut self, inst: Inst) {
        todo!();
    }

    fn ori(&mut self, inst: Inst) {
        let rs = self.cpu.reg(inst.rs());
        self.cpu.set_reg(inst.rt(), rs | inst.imm16());
    }

    fn xori(&mut self, inst: Inst) {
        todo!();
    }

    fn lui(&mut self, inst: Inst) {
        self.cpu.set_reg(inst.rt(), inst.imm16() << 16);
    }

    fn cop1(&mut self, inst: Inst) {
        todo!();
    }

    fn cop2(&mut self, inst: Inst) {
        todo!();
    }

    fn cop3(&mut self, inst: Inst) {
        todo!();
    }

    fn lb(&mut self, inst: Inst) {
        check_cache!(self);
        let addr = self.addr_with_imm(inst);
        let value = self.read_byte(addr);
        self.cpu.set_reg(inst.rt(), value.u32());
    }

    fn lh(&mut self, inst: Inst) {
        check_cache!(self);
        let addr = self.addr_with_imm(inst);
        let value = self.read_hword(addr);
        self.cpu.set_reg(inst.rt(), value.u32());
    }

    fn lwl(&mut self, inst: Inst) {
        todo!();
    }

    fn lw(&mut self, inst: Inst) {
        check_cache!(self);
        let addr = self.addr_with_imm(inst);
        let value = self.read_word(addr);
        self.cpu.set_reg(inst.rt(), value);
    }

    fn lbu(&mut self, inst: Inst) {
        todo!();
    }

    fn lhu(&mut self, inst: Inst) {
        todo!();
    }

    fn lwr(&mut self, inst: Inst) {
        todo!();
    }

    fn sb(&mut self, inst: Inst) {
        check_cache!(self);
        let addr = self.addr_with_imm(inst);
        self.write_byte(addr, self.cpu.reg(inst.rt()).u8());
    }

    fn sh(&mut self, inst: Inst) {
        check_cache!(self);
        let addr = self.addr_with_imm(inst);
        self.write_hword(addr, self.cpu.reg(inst.rt()).u16());
    }

    fn swl(&mut self, inst: Inst) {
        todo!();
    }

    fn sw(&mut self, inst: Inst) {
        check_cache!(self);
        let addr = self.addr_with_imm(inst);
        self.write_word(addr, self.cpu.reg(inst.rt()));
    }

    fn swr(&mut self, inst: Inst) {
        todo!();
    }

    fn lwc0(&mut self, inst: Inst) {
        todo!();
    }

    fn lwc1(&mut self, inst: Inst) {
        todo!();
    }

    fn lwc2(&mut self, inst: Inst) {
        todo!();
    }

    fn lwc3(&mut self, inst: Inst) {
        todo!();
    }

    fn swc0(&mut self, inst: Inst) {
        todo!();
    }

    fn swc1(&mut self, inst: Inst) {
        todo!();
    }

    fn swc2(&mut self, inst: Inst) {
        todo!();
    }

    fn swc3(&mut self, inst: Inst) {
        todo!();
    }

    fn unknown_instruction(&mut self, inst: Inst) {
        eprintln!("Unknown opcode 0x{:08X}", inst.0);
    }
}

// Secondary
impl PlayStation {
    fn secondary(&mut self, inst: Inst) {
        let secondary = inst.0.bits(0, 6);
        let handler = SECONDARY[secondary.us()];
        handler(self, inst);
    }

    fn sll(&mut self, inst: Inst) {
        let value = self.cpu.reg(inst.rt()) << inst.imm5();
        self.cpu.set_reg(inst.rd(), value);
    }

    fn srl(&mut self, inst: Inst) {
        todo!();
    }

    fn sra(&mut self, inst: Inst) {
        todo!();
    }

    fn sllv(&mut self, inst: Inst) {
        todo!();
    }

    fn srlv(&mut self, inst: Inst) {
        todo!();
    }

    fn srav(&mut self, inst: Inst) {
        todo!();
    }

    fn jr(&mut self, inst: Inst) {
        todo!();
    }

    fn jalr(&mut self, inst: Inst) {
        todo!();
    }

    fn syscall(&mut self, inst: Inst) {
        todo!();
    }

    fn break_(&mut self, inst: Inst) {
        todo!();
    }

    fn mfhi(&mut self, inst: Inst) {
        todo!();
    }

    fn mthi(&mut self, inst: Inst) {
        todo!();
    }

    fn mflo(&mut self, inst: Inst) {
        todo!();
    }

    fn mtlo(&mut self, inst: Inst) {
        todo!();
    }

    fn mult(&mut self, inst: Inst) {
        todo!();
    }

    fn multu(&mut self, inst: Inst) {
        todo!();
    }

    fn div(&mut self, inst: Inst) {
        todo!();
    }

    fn divu(&mut self, inst: Inst) {
        todo!();
    }

    fn add(&mut self, inst: Inst) {
        todo!();
    }

    fn addu(&mut self, inst: Inst) {
        todo!();
    }

    fn sub(&mut self, inst: Inst) {
        todo!();
    }

    fn subu(&mut self, inst: Inst) {
        todo!();
    }

    fn and(&mut self, inst: Inst) {
        let value = self.cpu.reg(inst.rs()) & self.cpu.reg(inst.rt());
        self.cpu.set_reg(inst.rd(), value);
    }

    fn or(&mut self, inst: Inst) {
        let value = self.cpu.reg(inst.rs()) | self.cpu.reg(inst.rt());
        self.cpu.set_reg(inst.rd(), value);
    }

    fn xor(&mut self, inst: Inst) {
        let value = self.cpu.reg(inst.rs()) ^ self.cpu.reg(inst.rt());
        self.cpu.set_reg(inst.rd(), value);
    }

    fn nor(&mut self, inst: Inst) {
        todo!();
    }

    fn slt(&mut self, inst: Inst) {
        todo!();
    }

    fn sltu(&mut self, inst: Inst) {
        todo!();
    }
}

pub struct Inst(u32);

impl Inst {
    pub fn rs(&self) -> u32 {
        self.0.bits(21, 5)
    }

    pub fn rt(&self) -> u32 {
        self.0.bits(16, 5)
    }

    pub fn rd(&self) -> u32 {
        self.0.bits(11, 5)
    }

    pub fn imm5(&self) -> u32 {
        self.0.bits(6, 5)
    }

    pub fn imm16(&self) -> u32 {
        self.0.low().u32()
    }

    pub fn imm16s(&self) -> i32 {
        self.0.low() as i16 as i32
    }

    pub fn imm26(&self) -> u32 {
        self.0.bits(0, 26)
    }
}
