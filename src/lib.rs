/*!
# ELF Loader

A dead simple crate for ELF64 parsing and loading.

## Features

- This crate is `#[no_std]`, you do not even need the `alloc` crate to start having fun.
- It performs a lot of bounds, alignment and other sanity checks. Don't just blindly trust
  a buffer containing code. =)
- There is no recursion in the code and the call graph is rather flat. Put simply: This crate
  won't eat much stack space.
- Errors are very descriptive and very compact error codes. No wasted register space on your
  happy path.
- This crate does its job in a quite small amount of code, despite all the error checking.
- No dependencies, except for `libcore`.

## TODOs

- Currently, only re-locatable `x86_64` executables are supported. However, at least support for
  AArch64 and RISC-V is planned.
- An other "not yet implemented" feature is dynamic linking. This is required to eventually make
  this crate a minimal drop-in replacement for `dlopen`.
- Just `Ctrl`+`F` this crate for `TODO` and `FIXME`. ಥ‿ಥ
- Guarantee 100% that no `panic!`s will occur.

## Getting Started

To use it, you need an in-memory buffer containing valid ELF data. Parsing and loading an ELF
is as easy as following these few steps:

1. Have a byte slice containing all of your ELF data. This might originate from loading a
   file or from invoking `include_bytes!("path/to/elf")`. Note that shared objects (`.so`)
   are also ELF files.
2. Call `Elf::try_parse` with your ELF slice. On success it will return a small parsed `Elf`
   struct.
3. Call the `Elf`'s `num_pages` function and multiply the result by `PAGE_SIZE`. This is the
   amount of page-aligned memory, in bytes, that you have to allocate for the next steps.
4. Call `Elf::try_load` with the `Elf` struct and a mutable slice of your newly allocated
   memory. This will copy all necessary segments into the new memory region after zero-filling it
   first. On success, the result is a `LoadedElf` struct which holds a mutable borrow to your
   allocated memory.
5. Call `LoadedElf::try_reloc` with the `ParsedElf` struct and a chosen virtual base address. The
   base address is where the final running program will think its first memory page is located.
   This allows you to re-locate an ELF from within a different address space. If you don't
   change the memory mapping of the loaded ELF, then the base address is the pointer of your
   allocated memory block's slice. You can get this pointer from `LoadedElf::loader_base`.
6. The re-location function takes an additional parameter: An optional function pointer for a
   callback. This callback receives memory ranges and the requested memory protection levels. You
   can use this callback to actually apply memory protection flags as specified by the ELF data.
   Do not assume that protection regions won't overlap and just blindly handle each request in
   order.
7. On success, the `LoadedElf::try_reloc` function returns a `ReadyElf`. This struct provides
   functions needed to run the ELF or grab its memory range.

### Examples

```
# use elf_loader::*;
# use std::mem;
# fn get_buffer() -> &'static [u8] { &[][..] }
# fn alloc_aligned(_: usize, _: usize) -> &'static mut [u8] { &mut [][..] }
# fn dealloc(_: &[u8]) {}
# fn main() {
#     fn sub() -> Result<(), ElfError> {
#         let protection_fn = protect_noop;
// Grab a buffer containing ELF data.
let elf_data = get_buffer();

// Try parsing the buffer as ELF data.
let elf = Elf::try_parse(elf_data)?;

// For the next step, we need to allocate a bunch of page-aligned memory.
// You might as well use a pre-allocated buffer from your `.bss` section.
let align = PAGE_SIZE as usize;
let size  = (elf.num_pages() * PAGE_SIZE) as usize;
let mem   = alloc_aligned(size, align);

// Now, load the ELF into our allocated memory. After that, you are free to throw
// `elf_data` and `elf` out of the window.
let loaded_elf = elf.try_load(mem)?;

// The next step is to re-locate and memory-protect our ELF. To do that we first need
// a base address. If you intend to run the loaded ELF as a plugin in your own address
// space, you can use `loader_base` as a base address, which is just `mem.as_ptr()`.
// Otherwise, you need a base address within the loaded ELF's address space.
let base  = loaded_elf.loader_base();
let ready = match loaded_elf.try_reloc(base, Some(protection_fn)) {
    Ok(r) => r,

    // In case of an error, you get back your memory slice to de-allocate or inspect
    // it. You don't get a `LoadedElf` back for a retry, that one is consumed. The
    // reason for this is that `m` might already have been partially modified until
    // the error occurred. You can, however, just re-load `elf` if you did not yet
    // throw it away.
    Err((m, e)) => {
        dealloc(m);
        return Err(e.into());
    },
};

// Now you can grab an entry function pointer for whichever address space.
// Go on and have fun!
let main: fn() = unsafe { mem::transmute(ready.p_entry()) };
unsafe { (main)() };

// Done? Better not leak all the precious memory. Only you have control
// over all the allocations.
dealloc(ready.p_mem());
#         Ok(())
#     }
#     let _ = sub();
# }
```
 */

#![no_std]

// TODO IMPORTANT guarantee 100% that this can't `panic!`, at all, not counting Debug/Display
// TODO add thread-local storage (TLS) support

use core::slice::{ self, Iter };
use core::ops::Range;



mod elf;
mod error;
mod parse;
mod load;
mod reloc;

pub use self::error::{ ElfError, ParseElfError, LoadElfError, RelocElfError };

use self::elf::{
    ElfProgramHeader,
    PF_R, PF_W, PF_X, PF_RW, PF_RX,
    PT_DYNAMIC, PT_GNU_RELRO, PT_GNU_STACK, PT_LOAD, PT_NULL,
};

use self::parse::try_parse_elf;
use self::load::try_load_elf;
use self::reloc::try_reloc_elf;



/// Smallest virtual memory page size on the target platform, in bytes.
///
/// ELF segments are usually loaded and modified in page-sized chunks, and to allow some
/// optimisations not done by this loader, ELF segment data is therefore typically even
/// page-aligned within the ELF file. (Memory-map and mutate in-place.)
// FIXME conditional for supported archs? crate if notable differences? Halpz!
// FIXME or maybe kill that and use byte lengths and offsets?
pub const PAGE_SIZE: u32 = 4096;



/// Represents a parsed and partially verified ELF binary with easy access
/// to the program headers required for loading an ELF.
///
/// This struct only accepts ELF data that fits within a 4GiB address range if
/// loaded at address zero.
#[derive(Clone)]
pub struct Elf<'a> {
    program_headers: ProgramHeaders<'a>,
    num_pages: u32,
    entry: u32,
}

impl<'a> Elf<'a> {
    /// Tries parsing a buffer as an ELF binary and partially verifies ELF headers.
    pub fn try_parse(raw: &'a [u8]) -> Result<Self, ParseElfError> {
        try_parse_elf(raw)
    }

    /// Tries loading the ELF into some page-aligned buffer.
    ///
    /// This does not yet re-locate or memory-protect the loaded ELF, in case you want to
    /// delay those steps or handle them in another process or thread.
    ///
    /// The given buffer must be page-aligned and big enough to hold at least `num_pages`
    /// pages of ELF segments.
    pub fn try_load<'b>(&self, mem: &'b mut [u8]) -> Result<LoadedElf<'b>, LoadElfError> {
        try_load_elf(self, mem)
    }

    /// Provides an iterator over the ELF's program headers.
    pub fn program_headers(&self) -> ProgramHeaders<'a> {
        self.program_headers.clone()
    }

    /// Number of contiguous memory pages to allocate in order to load the ELF.
    pub fn num_pages(&self) -> u32 {
        self.num_pages
    }
}



/// Represents a loaded, but not yet memory-protected and re-located ELF.
// TODO serialisability, possibly MessagePack, Binn?
pub struct LoadedElf<'a> {
    mem:       &'a mut [u8],
    dyn_start: usize,
    dyn_len:   usize,
    entry:     u32,
    protect:   SegmentStack,
}

impl<'a> LoadedElf<'a> {
    /// Try re-locating and memory-protecting the loaded ELF.
    ///
    /// - `base` is the base address of the re-located ELF's address space. If you run the ELF
    ///   in the loader's address space, then use the address from `loader_base`.
    /// - `prot` is an optional function to be called to restrict access to specific ranges of
    ///   memory. It is possible that overlapping regions of memory request distinct protection
    ///   levels. In such cases newer protection requests overrule older ones. This argument is
    ///   optional, as for some systems, like for UEFI, there is no proper way of restricting
    ///   memory access rights.
    pub fn try_reloc(mut self, base: *const u8, prot: Option<ProtectFn>)
    -> Result<ReadyElf<'a>, (&'a mut [u8], RelocElfError)> {
        let res   = try_reloc_elf(&mut self, base, prot);
        let mem   = self.mem;
        let entry = self.entry;

        match res {
            Ok( _) =>  Ok(ReadyElf { mem, base, entry }),
            Err(e) => Err((mem, e)),
        }
    }

    /// The final re-located ELF's base address within the ELF loader's address space.
    pub fn loader_base(&self) -> *const u8 {
        self.mem.as_ptr()
    }
}

/// Type of a memory-protecting callback.
///
/// - `prot` is the protection level to apply to the given range of memory.
/// - `p_base` is the base address within the ELF loader's address space.
/// - `v_base` is the base address within the re-located ELF's address space.
/// - `mem_len` is the size of the memory region pointed at by the base addresses.
/// - `range` is the region of memory to protect within the slice of memory
///   defined by one of the base addresses and `mem_len`.
pub type ProtectFn = extern "C" fn(
    prot:    SegmentProtection,
    p_base:  *const u8,
    v_base:  *const u8,
    mem_len: usize,
    range:   Range<usize>,
) -> Result<(), ()>;

/// A memory-protecting callback that does absolutely nothing.
///
/// Useful for systems like UEFI where there either is no way of protecting memory,
/// or where the system's API does not provide any methods to do such a thing.
pub extern "C" fn protect_noop(
    _: SegmentProtection, _: *const u8, _: *const u8, _: usize, _: Range<usize>
) -> Result<(), ()> {
    Ok(())
}

struct SegmentStack {
    data: [Segment; 8], // TODO more needed? 4 to 6 seems typical
    len:  u8,
}

impl SegmentStack {
    pub fn new() -> Self {
        Self {
            len:  0,
            data: [Segment {
                page_off: 0,
                page_num: 0,
                protect:  SegmentProtection::RO,
            }; 8],
        }
    }

    pub fn try_push(&mut self, ph: &ProgramHeader<'_>) -> Result<(), LoadElfError> {
        if (self.len as usize) >= self.data.len() {
            return Err(LoadElfError::TooManySegments);
        }

        self.data[self.len as usize] = Segment {
            page_off: ph.page_offset,
            page_num: ph.page_num,
            protect:  ph.protection,
        };
        self.len += 1;

        Ok(())
    }
}

#[derive(Copy, Clone)]
struct Segment {
    page_off: u32,
    page_num: u32,
    protect:  SegmentProtection,
}



/// An iterator over the ELF data's program headers.
#[derive(Clone)]
pub struct ProgramHeaders<'a> {
    inner: Iter<'a, ElfProgramHeader>,
    elf:   &'a [u8],
}

impl<'a> Iterator for ProgramHeaders<'a> {
    type Item = ProgramHeader<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match ProgramHeader::from_elf(self.inner.next()?, self.elf) {
                None     => continue, // a program header we don't give a fuck about
                Some(ph) => return Some(ph),
            }
        }
    }
}



/// The kind of memory protection to apply to a loaded segment.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u8)]
pub enum SegmentProtection {
    /// Segment is read-only.
    RO,

    /// Segment is read-write.
    RW,

    /// Segment is read-execute.
    RX,
}

impl SegmentProtection {
    fn from_flags(flags: u32) -> Self {
        match flags & (PF_R | PF_W | PF_X) {
            PF_R         => SegmentProtection::RO,
            PF_W | PF_RW => SegmentProtection::RW,
            PF_X | PF_RX => SegmentProtection::RX,

            // Attempted RWX.
            _ => SegmentProtection::RX,
        }
    }
}



/// Determines what an ELF loader should do.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u8)]
pub enum SegmentKind {
    /// Copy ELF data into program memory.
    Load,

    /// Dynamic linking information.
    Dynamic,

    /// Relocate and then change the memory protection.
    Relro,

    /// Some other program header we don't care about.
    Unsupported,
}

impl SegmentKind {
    fn from_kind(kind: u32) -> Option<Self> {
        match kind {
            PT_DYNAMIC   => Some(SegmentKind::Dynamic),
            PT_GNU_RELRO => Some(SegmentKind::Relro  ),
            PT_GNU_STACK => None, // We don't give a fuck. Stack is always RW, never RWX.
            PT_LOAD      => Some(SegmentKind::Load   ),
            PT_NULL      => None,
            _            => Some(SegmentKind::Unsupported),
        }
    }
}



/// An ELF program header, which is basically an instruction an ELF loader executes.
#[derive(Copy, Clone, Debug)]
pub struct ProgramHeader<'a> {
    /// What the current header wants us to do.
    pub kind: SegmentKind,

    /// What kind of memory protection to apply.
    pub protection: SegmentProtection,

    /// Offset of this segment, in multiples of 4KiB.
    pub page_offset: u32,

    /// Number of 4KiB pages in this segment.
    pub page_num: u32,

    /// Source of the data to copy.
    pub copy_from: &'a [u8],
}

impl<'a> ProgramHeader<'a> {
    fn from_elf(ph: &ElfProgramHeader, elf: &'a [u8]) -> Option<Self> {
        Some(ProgramHeader {
            kind:        SegmentKind      ::from_kind( ph.p_type )?,
            protection:  SegmentProtection::from_flags(ph.p_flags),
            page_offset: (ph.p_vaddr as u32) / PAGE_SIZE,
            page_num:    (ph.p_memsz as u32 + (PAGE_SIZE - 1)) / PAGE_SIZE,
            copy_from:   &elf[
                (ph.p_offset as usize) .. (ph.p_offset as usize).wrapping_add(ph.p_filesz as usize)
            ],
        })
    }
}



/// A readily loaded and re-located ELF. You can run this as a program now.
pub struct ReadyElf<'a> {
    mem:   &'a mut [u8],
    base:  *const u8,
    entry: u32,
}

impl<'a> ReadyElf<'a> {
    /// The range of the ready ELF's memory, in the ELF loader's address space.
    pub fn p_mem(&self) -> &[u8] {
        self.mem
    }

    /// The range of the ready ELF's memory, in its own address space.
    pub fn v_mem(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.base, self.mem.len()) }
    }

    /// Pointer to the entry function, in the ELF loader's address space.
    // FIXME return generic function pointer if variadic generics
    pub fn p_entry(&self) -> *const () {
        (&self.mem[(self.entry as usize)..]).as_ptr() as *const ()
    }

    /// Pointer to the entry function, in the ready ELF's address space.
    // FIXME return generic function pointer if variadic generics
    pub fn v_entry(&self) -> *const () {
        unsafe { self.base.add(self.entry as usize) as *const () }
    }
}
