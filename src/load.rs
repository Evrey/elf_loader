
use crate::elf::ElfDyn ;
use crate::{
    PAGE_SIZE,
    LoadElfError, Elf, LoadedElf,
    SegmentKind, SegmentStack,
    ProgramHeader,
};
use core::{ ptr, mem };



/// Tries loading a parsed ELF binary into a pre-allocated memory region and relocating
/// it to the specified base address.
pub fn try_load_elf<'a>(elf: &Elf<'_>, mem: &'a mut [u8])
-> Result<LoadedElf<'a>, LoadElfError> {
    check_buffer_requirements_and_zerofill(elf, mem)?;

    let mut segs = SegmentStack::new();
    let mut dyns = None;

    for ph in elf.program_headers() {
        match ph.kind {
            SegmentKind::Load => {
                segs.try_push(&ph)?;
                load_segment(&ph, mem)
            },
            SegmentKind::Dynamic => match dyns.take() {
                Some(_) => return Err(LoadElfError::MultipleDynamicSegments),
                None    => {
                    segs.try_push(&ph)?;
                    load_segment(&ph, mem);

                    let start = (ph.page_offset * PAGE_SIZE) as usize;
                    let len   = ph.copy_from.len() / mem::size_of::<ElfDyn>();
                    dyns = Some((start, len));
                },
            },
            SegmentKind::Relro       => segs.try_push(&ph)?,
            SegmentKind::Unsupported => (),
        }
    }

    let (dyn_start, dyn_len) = dyns.ok_or(LoadElfError::NoDynamicSegments)?;

    Ok(LoadedElf {
        mem, dyn_start, dyn_len,
        entry:     elf.entry,
        protect:   segs,
    })
}



fn check_buffer_requirements_and_zerofill(elf: &Elf<'_>, mem: &mut [u8])
-> Result<(), LoadElfError> {
    if mem.len() < ((elf.num_pages() as usize).wrapping_mul(PAGE_SIZE as usize)) {
        return Err(LoadElfError::BadBufferSize);
    }

    // FIXME store min alignment in `Elf`?
    if 0 != ((mem.as_ptr() as usize) % (PAGE_SIZE as usize)) {
        return Err(LoadElfError::BadBufferAlignment);
    }

    // Don't you fucking dare, compiler!
    unsafe { ptr::write_bytes(mem.as_mut_ptr(), 0_u8, mem.len()) };

    Ok(())
}

fn load_segment(ph: &ProgramHeader, mem: &mut [u8]) {
    let start = (ph.page_offset * PAGE_SIZE) as usize;
    let end   = ph.copy_from.len() + start;
    let dst   = &mut mem[start..end];

    dst.copy_from_slice(ph.copy_from);
}
