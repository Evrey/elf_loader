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

    // In case of an error, you get back your memory slice to de-allocate or inspect it.
    // You don't get a `LoadedElf` back for a retry, that one is consumed. The reason for
    // this is that `m` might already have been partially modified until the error occurred.
    // You can, however, just re-load `elf` if you did not yet throw it away.
    Err((m, e)) => {
        dealloc(m);
        return Err(e.into());
    },
};

// Now you can grab an entry function pointer for whichever address space. Go on and have fun!
let main: fn() = unsafe { mem::transmute(ready.p_entry()) };
unsafe { (main)() };

// Done? Better not leak all the precious memory. Only you have control over all the allocations.
dealloc(ready.p_mem());
```
