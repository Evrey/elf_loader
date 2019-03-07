
use crate::{ LoadedElf, RelocElfError, ProtectFn, SegmentProtection };
use crate::elf::{
    ElfDyn, ElfRel, ElfRela,
    DT_REL, DT_RELSZ, DT_RELENT, DT_RELA, DT_RELASZ, DT_RELAENT,
    R_X86_64_NONE, R_X86_64_COPY, R_X86_64_RELATIVE,
    r_type,
};
use core::{ mem, slice };



pub fn try_reloc_elf(elf: &mut LoadedElf<'_>, base: *mut u8, prot: Option<ProtectFn>)
-> Result<(), RelocElfError> {
    let base_off = base_to_offset(elf.mem_align(), base)?;

    relocate_segments(elf, base_off)?;

    protect_segments(elf, base, prot)
}

fn protect_segments(elf: &mut LoadedElf<'_>, v_base: *mut u8, prot: Option<ProtectFn>)
-> Result<(), RelocElfError> {
    if let Some(prot) = prot {
        let p_base  = elf.mem.as_mut_ptr();
        let mem_len = elf.mem.len();

        // Initial protection request to make everything read-only. This way no unused memory
        // is left with undefined, at worst executable, rights.
        (prot)(
            SegmentProtection::RO,
            p_base, v_base, mem_len,
            0_usize .. elf.mem.len()
        ).map_err(|_| RelocElfError::MemProtectFailed)?;

        for seg in &elf.protect.data[..(elf.protect.len as usize)] {
            (prot)(
                seg.protect,
                p_base, v_base, mem_len,
                seg.range.to_byte_range()
            ).map_err(|_| RelocElfError::MemProtectFailed)?;
        }
    }

    Ok(())
}

fn base_to_offset(align: u32, base: *mut u8) -> Result<usize, RelocElfError> {
    let off = base as usize;

    match off % (align as usize) {
        0 =>  Ok(off),
        _ => Err(RelocElfError::BadBaseAddressAlignment),
    }
}

fn relocate_segments(elf: &mut LoadedElf<'_>, off: usize)
-> Result<(), RelocElfError> {
    use self::RelocElfError::*;

    let mem_base      = elf.mem.as_mut_ptr();
    let mem_len       = elf.mem.len();
    let dyns          = elf.dyns.try_slice(elf.mem, BadDynAlignment)?;
    let (rels, relas) = find_rels_and_relas(elf.mem, dyns)?;

    // FIXME Does the ELF spec say something about "either, or"? Where even is the ELF spec?!
    for rel  in rels  { apply_rel( rel , mem_base, mem_len, off)?; }
    for rela in relas { apply_rela(rela, mem_base, mem_len, off)?; }

    Ok(())
}

fn find_rels_and_relas<'a>(mem: &'a [u8], dyns: &'a [ElfDyn])
-> Result<(&'a [ElfRel], &'a [ElfRela]), RelocElfError> {
    // FIXME move to load?
    let mut  rel_table_off = 0_u64;
    let mut  rel_table_len = 0_u64;

    let mut rela_table_off = 0_u64;
    let mut rela_table_len = 0_u64;

    for d in dyns {
        match d.d_tag {
            DT_REL     =>  rel_table_off = d.d_val,
            DT_RELSZ   =>  rel_table_len = d.d_val,
            DT_RELENT  => if (mem::size_of::<ElfRel >() as u64) != d.d_val {
                return Err(RelocElfError::BadRelSize );
            },
            DT_RELA    => rela_table_off = d.d_val,
            DT_RELASZ  => rela_table_len = d.d_val,
            DT_RELAENT => if (mem::size_of::<ElfRela>() as u64) != d.d_val {
                return Err(RelocElfError::BadRelaSize);
            },
            _ => (), // Other `DT_DYNAMIC` entries are of no interest to us.
        }
    }

    slice_rel_rela(mem, rel_table_off, rel_table_len, rela_table_off, rela_table_len)
}

fn slice_rel_rela(
    mem: &[u8],
    rel_off: u64, rel_len: u64,
    rela_off: u64, rela_len: u64
)
-> Result<(&[ElfRel], &[ElfRela]), RelocElfError> {
    let  rel_mem = slice_rel(mem,  rel_off,  rel_len)?;
    let rela_mem = slice_rel(mem, rela_off, rela_len)?;

    Ok((rel_mem, rela_mem))
}

fn slice_rel<T: Sized>(mem: &[u8], off: u64, len: u64) -> Result<&[T], RelocElfError> {
    if off == 0 { return Ok(&[]); }

    if off.checked_add(len).map(|end| end >= (mem.len() as u64)).unwrap_or(true) {
        return Err(RelocElfError::BadRelRelaTableRange);
    }

    let addr = (&mem[(off as usize)..]).as_ptr() as *const T;

    if 0 != ((addr as usize) % mem::align_of::<T>()) {
        return Err(RelocElfError::BadRelRelaTableAlignment);
    }

    Ok(unsafe { slice::from_raw_parts(
        addr,
        (len as usize) / mem::size_of::<T>()
    )})
}

// In case you stumble upon relocation formulae, and - like me - have no
// idea what the fuck to do:
// - S:        ? Value of "symbol", symbol index in re-location entry
// - A:        `rela.r_addend`
// - B:        `base`
// - P:        ? "place" somehow calculated from `rela.r_offset`
// - G:        ? Offset into GOT where the address of the reloc symbol is
// - L:        ? Address of Procedure Linkage Table for a symbol
// - GOT:      ? Address of Global Offset Table
// - Z:        ?
// - indirect: ?

fn apply_rel(rel: &ElfRel, mem_base: *mut u8, mem_len: usize, base: usize)
-> Result<(), RelocElfError> {
    // Pretty much TODO here.
    let _ = (rel, mem_base, mem_len, base); // shut up, linter
    Err(RelocElfError::UnsupportedRelArch)
}

fn apply_rela(rela: &ElfRela, mem_base: *mut u8, mem_len: usize, base: usize)
-> Result<(), RelocElfError> {
    if rela.r_offset >= (mem_len as u64) {
        return Err(RelocElfError::BadRelaOffset);
    }

    let reloc_this = mem_base.wrapping_add(rela.r_offset as usize) as *mut u64;
    let reloc_ty   = r_type(rela.r_info);
    let a          = rela.r_addend as u64;
    let b          = base as u64;

    if cfg!(target_arch = "x86_64") { apply_rela_x86_64(reloc_this, reloc_ty, a, b) }
    else { Err(RelocElfError::UnsupportedRelaArch) }
}

#[cfg(target_arch = "x86_64")]
fn apply_rela_x86_64(r: *mut u64, ty: u32, a: u64, b: u64) -> Result<(), RelocElfError> {
    match ty {
        | R_X86_64_COPY
        | R_X86_64_NONE => (),

        | R_X86_64_RELATIVE => unsafe { r.write_unaligned(a.wrapping_add(b)) },

        _ => return Err(RelocElfError::UnsupportedRelaType),
    }

    Ok(())
}
