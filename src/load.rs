
use crate::{
    LoadElfError, Elf, LoadedElf,
    SegmentKind, SegmentStack,
    ProgramHeader,
};
use core::ptr;



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
                    // TODO make offset relative to load base?
                    segs.try_push(&ph)?;
                    load_segment(&ph, mem);

                    dyns = Some(ph.load_range.convert());
                },
            },
            SegmentKind::Relro       => segs.try_push(&ph)?,
            SegmentKind::Unsupported => (),
        }
    }

    Ok(LoadedElf {
        mem, dyns: dyns.ok_or(LoadElfError::NoDynamicSegments)?,
        mem_align: elf.mem_align(),
        entry:     elf.entry,
        protect:   segs,
    })
}



fn check_buffer_requirements_and_zerofill(elf: &Elf<'_>, mem: &mut [u8])
-> Result<(), LoadElfError> {
    if mem.len() < (elf.mem_len() as usize) {
        return Err(LoadElfError::BadBufferSize);
    }

    // FIXME Store log2 alignment in `elf`?
    if 0 != ((mem.as_ptr() as usize) % (elf.mem_align() as usize)) {
        return Err(LoadElfError::BadBufferAlignment);
    }

    // Don't you fucking dare, compiler!
    unsafe { ptr::write_bytes(mem.as_mut_ptr(), 0_u8, mem.len()) };

    Ok(())
}

fn load_segment(ph: &ProgramHeader, mem: &mut [u8]) {
    // We already bounds-checked `load_range` while parsing, and we already ensured that
    // this invariant holds as well. This prevents the compiler from inserting `panic!`s
    // when generating optimised code, due to slice bounds checks.
    let dst = unsafe { ph.load_range.as_slice_mut(mem) };

    if dst.len() < ph.copy_from.len() {
        unsafe { ::core::hint::unreachable_unchecked() }
    }

    (&mut dst[..ph.copy_from.len()])
        .copy_from_slice(ph.copy_from);
}
