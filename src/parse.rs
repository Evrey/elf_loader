
use crate::elf::{
    ElfFileHeader, ElfProgramHeader,
    EI_CLASS, EI_DATA, ET_DYN,
    ELFMAG, SELFMAG, ELFCLASS64, ELFDATA2LSB, ELFDATA2MSB,
    EM_AARCH64, EM_RISCV, EM_X86_64,
    PF_X, PT_LOAD,
};
use crate::{ ParseElfError, Elf, ProgramHeaders };
use core::slice::{ self, Iter };
use core::mem;



pub fn try_parse_elf<'a>(raw: &'a [u8]) -> Result<Elf<'a>, ParseElfError> {
    let  header                                      = try_load_header(raw)?;
    let (mem_len, mem_align, entry, program_headers) = try_load_program_headers(header, raw)?;

    Ok(Elf { program_headers, mem_len, mem_align, entry })
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

    // FIXME maybe allow ELF32 one day
    if header.e_ident[EI_CLASS] != ELFCLASS64 {
        return Err(ParseElfError::NotElf64);
    }

    check_is_native_endian(header.e_ident[EI_DATA ])?;

    if header.e_type != ET_DYN {
        return Err(ParseElfError::NotPic);
    }

    check_isa(header.e_machine)?; // TODO ? header.e_flags

    Ok(header)
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
        EM_RISCV   => false, // FIXME wait for `rustc` to target RV64
        EM_X86_64  => cfg!(target_arch = "x86_64"),
        // FIXME more archs?

        _ => false,
    };

    if wat {  Ok(()) }
    else   { Err(ParseElfError::BadIsa) }
}



fn try_load_program_headers<'a>(hdr: &'a ElfFileHeader, raw: &'a [u8])
-> Result<(u32, u32, u32, ProgramHeaders<'a>), ParseElfError> {
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
    let (mem_len, mem_align) = check_ph_ranges(hdrs.iter(), raw, hdr.e_entry)?;

    Ok((mem_len, mem_align, hdr.e_entry as u32, ProgramHeaders {
        inner: hdrs.iter(),
        elf:   raw,
    }))
}

fn check_ph_ranges<'a>(hdrs: Iter<'a, ElfProgramHeader>, raw: &'a [u8], ent: u64)
-> Result<(u32, u32), ParseElfError> {
    let mut end_offset   = 0;
    let mut max_align    = 1;
    let mut entry_in_exe = false;

    // FIXME Bail out on too high header count?
    for ph in hdrs {
        // `p_offset` and `p_filesz` implicitly checked against a 4GiB limit,
        // as `raw.len()` has already checked to be at most that.
        if ph.p_offset.checked_add(ph.p_filesz)
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

        if ph.p_memsz < ph.p_filesz {
            return Err(ParseElfError::PhSmallerThanVmem);
        }

        if ent != 0 {
            if ((ph.p_type, ph.p_flags & PF_X) == (PT_LOAD, PF_X))
            & ((ent >= ph.p_vaddr) & (ent < ph.p_vaddr.wrapping_add(ph.p_memsz))) {
                // In case there are - for whatever reason - valid ELF files with many
                // executable segments, delaying the error return allows us to check
                // the entry address against all of them.
                entry_in_exe = true;
            }
        }

        let end   = (ph.p_vaddr.wrapping_add(ph.p_memsz)) as u32;
        let align = if ph.p_align <= (u32::max_value() as u64) { ph.p_align as u32 }
                    else { return Err(ParseElfError::ExcessiveAlignment); };

        if end   > end_offset { end_offset = end;   }
        if align > max_align  { max_align  = align; }
    }

    // FIXME For shared objects, it seems to be the case that `ent==0` means no entry. Check this.
    if (ent != 0) & (!entry_in_exe) {
        return Err(ParseElfError::BadEntry);
    }

    Ok((end_offset, max_align))
}
