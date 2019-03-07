#![allow(missing_docs)]



pub const EI_CLASS:    usize   =   4;
pub const EI_DATA:     usize   =   5;
pub const ET_DYN:      u16     =   3;
pub const ELFMAG:      [u8; 4] = [b'\x7F', b'E', b'L', b'F'];
pub const SELFMAG:     usize   =   4;
pub const ELFCLASS64:  u8      =   2;
pub const ELFDATA2LSB: u8      =   1;
pub const ELFDATA2MSB: u8      =   2;
pub const EM_X86_64:   u16     =  62;
pub const EM_AARCH64:  u16     = 183;
pub const EM_RISCV:    u16     = 243;

pub const PF_X:   u32 = 0b001;
pub const PF_W:   u32 = 0b010;
pub const PF_R:   u32 = 0b100;
pub const PF_RW:  u32 = 0b110;
pub const PF_RX:  u32 = 0b101;

pub const PT_NULL:      u32 = 0;
pub const PT_LOAD:      u32 = 1;
pub const PT_DYNAMIC:   u32 = 2;
pub const PT_GNU_STACK: u32 = 0x6474E551;
pub const PT_GNU_RELRO: u32 = 0x6474E552;

pub const DT_REL:     u64 = 17;
pub const DT_RELSZ:   u64 = 18;
pub const DT_RELENT:  u64 = 19;
pub const DT_RELA:    u64 =  7;
pub const DT_RELASZ:  u64 =  8;
pub const DT_RELAENT: u64 =  9;

pub const R_X86_64_NONE:     u32 = 0;
pub const R_X86_64_COPY:     u32 = 5;
pub const R_X86_64_RELATIVE: u32 = 8;



#[derive(Copy, Clone)]
#[repr(C)]
pub struct ElfFileHeader {
    pub e_ident:     [u8; 16],
    pub e_type:      u16,
    pub e_machine:   u16,
    pub e_version:   u32,
    pub e_entry:     u64,
    pub e_phoff:     u64,
    pub e_shoff:     u64,
    pub e_flags:     u32,
    pub e_ehsize:    u16,
    pub e_phentsize: u16,
    pub e_phnum:     u16,
    pub e_shentsize: u16,
    pub e_shnum:     u16,
    pub e_shstrndx:  u16,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct ElfProgramHeader {
    pub p_type:   u32,
    pub p_flags:  u32,
    pub p_offset: u64,
    pub p_vaddr:  u64,
    pub p_paddr:  u64,
    pub p_filesz: u64,
    pub p_memsz:  u64,
    pub p_align:  u64,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct ElfDyn {
    pub d_tag: u64,
    pub d_val: u64,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct ElfRel {
    pub r_offset: u64,
    pub r_info:   u64,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct ElfRela {
    pub r_offset: u64,
    pub r_info:   u64,
    pub r_addend: i64,
}



#[inline(always)]
pub fn r_type(info: u64) -> u32 {
    (info & 0xFFFFFFFF) as u32
}
