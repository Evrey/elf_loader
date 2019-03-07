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

- Currently, only page-aligned re-locatable `x86_64` executables are supported. However, at least
  support for AArch64 and RISC-V is planned.
- An other "not yet implemented" feature is dynamic linking. This is required to eventually make
  this crate a minimal drop-in replacement for `dlopen`. You cannot currently look up symbols, so
  all you get from loading an ELF is its entry point.
- Currently, custom linker scripts have to be used that page-align all loadable sections. To relax
  this requirement, I'd need help finding and understanding the source code of `ld.so` from `glibc`.
  I.e. this crate does not currently act as a program interpreter.
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
3. Call the `Elf`'s `mem_len` and `mem_align` functions. Those will give you the layout information
   needed to allocate a buffer for the next step.
4. Call `Elf::try_load` with the `Elf` struct and a mutable slice of your newly allocated
   memory. This will copy all necessary segments into the new memory region after zero-filling it
   first. On success, the result is a `LoadedElf` struct which holds a mutable borrow to your
   allocated memory.
5. Call `LoadedElf::try_reloc` with a chosen virtual base address and an optional memory protection
   callback. The base address is where the final running program will think its first memory page is
   located. This allows you to re-locate an ELF from within a different address space. If you don't
   change the memory mapping of the loaded ELF, then the base address is the pointer of your
   allocated memory block's slice. You can get this pointer from `LoadedElf::loader_base`.
6. The memory protection function receives base addresses, a slice, and the requested memory
   protection level. You can use this callback to actually apply memory protection flags as
   specified by the ELF data. Do not assume that protection regions won't overlap and just blindly
   handle each request in order.
7. On success, the `LoadedElf::try_reloc` function returns a `ReadyElf`. This struct provides
   functions needed to run the ELF or grab its memory range.

### Examples

```
# use elf_loader::*;
# use std::mem;
# fn get_aligned_buffer() -> &'static [u8] { &[][..] }
# fn alloc_aligned(_: usize, _: usize) -> &'static mut [u8] { &mut [][..] }
# fn dealloc(_: &[u8]) {}
# fn main() {
#     fn sub() -> Result<(), ElfError> {
#         let protection_fn = protect_noop;
// Grab a buffer containing ELF data.
let elf_data = get_aligned_buffer();

// Try parsing the buffer as ELF data.
let elf = Elf::try_parse(elf_data)?;

// For the next step, we need to allocate a bunch of page-aligned memory.
// You might as well use a pre-allocated buffer from your `.bss` section.
let align = elf.mem_align() as usize;
let size  = elf.mem_len()   as usize;
let mem   = alloc_aligned(size, align);

// Now, load the ELF into our allocated memory. After that, you are free to throw
// `elf_data` and `elf` out of the window.
let mut loaded_elf = elf.try_load(mem)?;
drop(elf);
drop(elf_data);

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

## B-but why?!

I have two personal goals for this crate.

One is to use it in my toy OS to unpack the kernels etc. from the OS loader,
without needing some fancy virtual init file system. Eventually, in a century,
it might even load user-space applications.

The other goal is to use it as an embeddable `dlopen`-replacement for plugins with
a minimum amount of dependencies. You could load the same ELF plugin into a Windows
or Linux build of your application. Just exchange some v-tables on plugin entry.

In both cases, possibly illformed ELF data should not cause any kind of undefined or
undesired behaviour, from parsing to re-locating. Your only security risk should be
calling the entry function.
 */

#![no_std]

// TODO IMPORTANT guarantee 100% that this can't `panic!`, at all, not counting Debug/Display
// TODO add thread-local storage (TLS) support

use core::slice::{ self, Iter };
use core::marker::PhantomData;
use core::ops::Range;
use core::mem;



mod elf;
mod error;
mod parse;
mod load;
mod reloc;

pub use self::error::{ ElfError, ParseElfError, LoadElfError, RelocElfError };

use self::elf::{
    ElfProgramHeader, ElfDyn,
    PF_R, PF_W, PF_X, PF_RW, PF_RX,
    PT_DYNAMIC, PT_GNU_RELRO, PT_GNU_STACK, PT_LOAD, PT_NULL,
};

use self::parse::try_parse_elf;
use self::load::try_load_elf;
use self::reloc::try_reloc_elf;



/// Represents a parsed and partially verified ELF binary with easy access
/// to the program headers required for loading an ELF.
///
/// This struct only accepts ELF data that fits within a 4GiB address range if
/// loaded at address zero.
#[derive(Clone)]
pub struct Elf<'a> {
    program_headers: ProgramHeaders<'a>,
    mem_len:   u32,
    mem_align: u32,
    entry:     u32,
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
    /// The given buffer must have `mem_align` alignment and be at least `mem_len` bytes in size.
    pub fn try_load<'b>(&self, mem: &'b mut [u8]) -> Result<LoadedElf<'b>, LoadElfError> {
        try_load_elf(self, mem)
    }

    /// Provides an iterator over the ELF's program headers.
    pub fn program_headers(&self) -> ProgramHeaders<'a> {
        self.program_headers.clone()
    }

    /// Minimum number of bytes to allocate to load this ELF.
    pub fn mem_len(&self) -> u32 {
        self.mem_len
    }

    /// Minimum alignment, in bytes, of the to-be-allocated load buffer.
    pub fn mem_align(&self) -> u32 {
        self.mem_align
    }
}



/// Represents a loaded, but not yet memory-protected and re-located ELF.
// TODO serialisability, possibly MessagePack, Binn?
pub struct LoadedElf<'a> {
    mem:       &'a mut [u8],
    dyns:      Slice32<ElfDyn>,
    mem_align: u32,
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
    pub fn try_reloc(mut self, base: *mut u8, prot: Option<ProtectFn>)
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
    pub fn loader_base(&mut self) -> *mut u8 {
        self.mem.as_mut_ptr()
    }

    /// Minimum number of bytes to allocate to load this ELF.
    pub fn mem_len(&self) -> usize {
        self.mem.len()
    }

    /// Minimum alignment, in bytes, of the to-be-allocated load buffer.
    pub fn mem_align(&self) -> u32 {
        self.mem_align
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
    p_base:  *mut u8,
    v_base:  *mut u8,
    mem_len: usize,
    range:   Range<usize>,
) -> Result<(), ()>;

/// A memory-protecting callback that does absolutely nothing.
///
/// Useful for systems like UEFI where there either is no way of protecting memory,
/// or where the system's API does not provide any methods to do such a thing.
pub extern "C" fn protect_noop(
    _: SegmentProtection, _: *mut u8, _: *mut u8, _: usize, _: Range<usize>
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
                range:   Slice32::new(0, 0),
                protect: SegmentProtection::RO,
            }; 8],
        }
    }

    pub fn try_push(&mut self, ph: &ProgramHeader<'_>) -> Result<(), LoadElfError> {
        if (self.len as usize) >= self.data.len() {
            return Err(LoadElfError::TooManySegments);
        }

        self.data[self.len as usize] = Segment {
            range:   ph.load_range,
            protect: ph.protection,
        };
        self.len += 1;

        Ok(())
    }
}

#[derive(Copy, Clone)]
struct Segment {
    range:   Slice32<u8>,
    protect: SegmentProtection,
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
// FIXME How does one handle alignment here? `readelf` reports offsets that don't fit alignment.
#[derive(Copy, Clone, Debug)]
pub struct ProgramHeader<'a> {
    /// What the current header wants us to do.
    pub kind: SegmentKind,

    /// What kind of memory protection to apply.
    pub protection: SegmentProtection,

    /// A slice into the buffer where the ELF is to be loaded.
    pub load_range: Slice32<u8>,

    /// Source of the data to copy.
    ///
    /// This is a sub-slice of the original ELF data.
    pub copy_from: &'a [u8],
}

impl<'a> ProgramHeader<'a> {
    fn from_elf(ph: &ElfProgramHeader, elf: &'a [u8]) -> Option<Self> {
        Some(ProgramHeader {
            kind:        SegmentKind      ::from_kind( ph.p_type )?,
            protection:  SegmentProtection::from_flags(ph.p_flags),
            load_range:  Slice32::new(ph.p_vaddr as u32, ph.p_memsz as u32),
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



/// A slice-ish thing that only uses 32-bit offset and length elements.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Slice32<T: Sized + Copy> {
    pub start: u32, // In 1 byte steps.
    pub len:   u32, // In multiples of `size_of::<T>()`.
    _wat: PhantomData<T>,
}

impl<T: Sized + Copy> Slice32<T> {
    /// Creates a new slice from starting offset and length.
    ///
    /// - `start` is relative to another slice.
    /// - `len` is in `T`-sized steps.
    pub fn new(start: u32, len: u32) -> Self {
        Self { start, len, _wat: PhantomData }
    }

    /// Creates a byte range for slicing memory.
    pub fn to_byte_range(self) -> Range<usize> {
        (self.start as usize)
        ..
        (self.start.wrapping_add(self.len.wrapping_mul(mem::size_of::<T>() as u32)) as usize)
    }

    /// Tries to grab a sub-slice of `T`s from `mem`.
    ///
    /// Fails if the sub-slice would have bad alignment.
    pub(crate) fn try_slice<'a, E>(self, mem: &'a [u8], bad_align: E)
    -> Result<&'a [T], E> {
        // No bounds checking required, will have been done at parsing time.
        let base = unsafe { mem.as_ptr().add(self.start as usize) } as *const T;

        if 0 != ((base as usize) % mem::align_of::<T>()) {
            return Err(bad_align);
        }

        Ok(unsafe { slice::from_raw_parts(base, self.len as usize) })
    }

    /// A specialisation of `try_slice` that avoids alignment checks.
    ///
    /// This is safe if `T == u8`, otherwise stay away from it.
    // FIXME Rather specialise `try_slice` for `u8` and `Result<&'a [u8], !>`, if stable `!`.
    pub unsafe fn as_slice<'a>(self, mem: &'a [u8]) -> &'a [T] {
        slice::from_raw_parts(
            mem.as_ptr().add(self.start as usize) as *const T,
            self.len as usize
        )
    }

    /// Like `as_slice`, but grabs a mutable reference. Again, no alignment checks.
    pub unsafe fn as_slice_mut<'a>(self, mem: &'a mut [u8]) -> &'a mut [T] {
        slice::from_raw_parts_mut(
            mem.as_mut_ptr().add(self.start as usize) as *mut T,
            self.len as usize
        )
    }

    /// Because `libcore` complains about the case where `T == U`, and I cannot
    /// opt-out of it.
    pub fn convert<U: Sized + Copy>(self) -> Slice32<U> {
        Slice32::new(
            self.start,
            (((self.len as usize) * mem::size_of::<T>()) / mem::size_of::<U>()) as u32
        )
    }
}
