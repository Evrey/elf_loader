
use crate::elf::{
    ElfFileHeader, ElfProgramHeader,
    EI_CLASS, EI_DATA, ET_DYN,
    ELFMAG, SELFMAG, ELFCLASS64, ELFDATA2LSB, ELFDATA2MSB,
    EM_AARCH64, EM_RISCV, EM_X86_64,
    PF_X, PT_LOAD,
};
use crate::{ PAGE_SIZE, ParseElfError, Elf, ProgramHeaders };
use core::slice::{ self, Iter };
use core::mem;



pub fn try_parse_elf<'a>(raw: &'a [u8]) -> Result<Elf<'a>, ParseElfError> {
    let  header                             = try_load_header(raw)?;
    let (num_pages, entry, program_headers) = try_load_program_headers(header, raw)?;

    Ok(Elf { program_headers, num_pages, entry })
}



fn try_load_header(raw: &[u8]) -> Result<&ElfFileHeader, ParseElfError> {
    if (raw.len() < mem::size_of::<ElfFileHeader>())
     | (raw.len() > (u32::max_value() as usize)) {
        return Err(ParseElfError::BadBufferSize);
    }

    if 0 != ((raw.as_ptr() as usize) % mem::align_of::<ElfFileHeader>()) {
        return Err(ParseElfError::BadBufferAlignment);
    }

    let header: &ElfFileHeader = unsafe { mem::transmute(raw.as_ptr()) };

    if &header.e_ident[..SELFMAG] != &ELFMAG[..] {
        return Err(ParseElfError::BufferNotElf);
    }

    if (header.e_ehsize as usize) != mem::size_of::<ElfFileHeader>() {
        return Err(ParseElfError::BadHeaderSize);
    }

    check_is_elf64(        header.e_ident[EI_CLASS])?;
    check_is_native_endian(header.e_ident[EI_DATA ])?;

    if header.e_type != ET_DYN {
        return Err(ParseElfError::NotPic);
    }

    check_isa(header.e_machine)?; // TODO ? header.e_flags

    Ok(header)
}

fn check_is_elf64(tag: u8) -> Result<(), ParseElfError> {
    if tag == ELFCLASS64 {       Ok(()) }
    else { Err(ParseElfError::NotElf64) }
}

fn check_is_native_endian(tag: u8) -> Result<(), ParseElfError> {
    match tag {
        ELFDATA2LSB if cfg!(target_endian = "little") => Ok(()),
        ELFDATA2MSB if cfg!(target_endian = "big"   ) => Ok(()),

        _ => Err(ParseElfError::BadEndian),
    }
}

fn check_isa(tag: u16) -> Result<(), ParseElfError> {
    let wat = match tag {
        EM_AARCH64 => cfg!(target_arch = "aarch64"),
        EM_RISCV   => cfg!(target_arch = "riscv32imac"), // TODO this is wrong? maybe?
        EM_X86_64  => cfg!(target_arch = "x86_64"),
        // FIXME more archs?

        _ => false,
    };

    if wat {  Ok(()) }
    else   { Err(ParseElfError::BadIsa) }
}



fn try_load_program_headers<'a>(hdr: &'a ElfFileHeader, raw: &'a [u8])
-> Result<(u32, u32, ProgramHeaders<'a>), ParseElfError> {
    if (hdr.e_phentsize as usize) != mem::size_of::<ElfProgramHeader>() {
        return Err(ParseElfError::BadProgramHeaderSize);
    }

    let hoff = hdr.e_phoff;
    let ptr  = unsafe { raw.as_ptr().add(hoff as usize) as *const ElfProgramHeader };
    let len  = hdr.e_phnum as usize;
    let l    = raw.len() as u64;

    if (mem::size_of::<ElfProgramHeader>() as u64).checked_mul(len as u64)
            .and_then(|x| x.checked_add(hoff))
            .map(|x| x >= l)
            .unwrap_or(true) {
        return Err(ParseElfError::ProgramHeaderOverflow);
    }

    if 0 != ((ptr as usize) % mem::align_of::<ElfProgramHeader>()) {
        return Err(ParseElfError::BadBufferAlignment);
    }

    let hdrs: &[ElfProgramHeader] = unsafe { slice::from_raw_parts(ptr, len) };

    // Bounds-check here, so we can blindly slice the ELF buffer later.
    let n_pages = check_ph_ranges(hdrs.iter(), raw, hdr.e_entry)?;

    Ok((n_pages, hdr.e_entry as u32, ProgramHeaders {
        inner: hdrs.iter(),
        elf:   raw,
    }))
}

fn check_ph_ranges<'a>(hdrs: Iter<'a, ElfProgramHeader>, raw: &'a [u8], ent: u64)
-> Result<u32, ParseElfError> {
    let mut e = 0;

    for ph in hdrs {
        if  ph.p_offset.checked_add(ph.p_filesz)
                       .map(|x| x >= (raw.len() as u64))
                       .unwrap_or(true) {
            return Err(ParseElfError::BadPhRange);
        }

        if (ph.p_vaddr.checked_add(ph.p_memsz)
                      .map(|x| x > (u32::max_value() as u64))
                      .unwrap_or(true))
         | (ph.p_memsz > (u32::max_value() as u64)) {
            return Err(ParseElfError::BadVmemRange);
        }

        if ((ph.p_type, ph.p_flags & PF_X) == (PT_LOAD, PF_X))
         & ((ent < ph.p_vaddr) | (ent > ph.p_vaddr.wrapping_add(ph.p_memsz))) {
            // FIXME In case there are - for whatever reason - valid ELFs with multiple
            //       executable segments, change this code to instead check whether the
            //       entry point lies within a non-executable segment.
            return Err(ParseElfError::BadEntry);
        }

        let end = (
            ((ph.p_vaddr + ph.p_memsz) + ((PAGE_SIZE - 1) as u64)) / (PAGE_SIZE as u64)
        ) as u32;

        if end > e { e = end; }
    }

    Ok(e)
}
