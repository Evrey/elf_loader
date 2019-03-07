
use elf_loader::*;
use std::mem;



mod os;



// FIXME force alignment, using a custom section if necessary.
static ELF: &[u8] = include_bytes!("./simple.elf");



#[test]
fn simple_elf_works() {
    let mut buf = Vec::from(ELF);
    println!("ELF @{:p}", buf.as_ptr());

    let elf = Elf::try_parse(&buf[..]).expect("Parsing `simple.elf` failed");

    let mem_len   = elf.mem_len()   as usize;
    let mem_align = elf.mem_align() as usize;
    let mem       = os::alloc_aligned(mem_len, mem_align); // Just leak here.

    let mut loaded_elf = elf.try_load(mem).expect("Loading `simple.elf` failed");

    drop(elf);
    buf.iter_mut().for_each(|x| *x = 0xCC);
    drop(buf);

    let base  = loaded_elf.loader_base();
    let ready = loaded_elf.try_reloc(base, Some(os::protection_fn))
                          .expect("Re-locating `simple.elf` failed");

    let main: fn()->i32 = unsafe { mem::transmute(ready.p_entry()) };

    let res = (main)();

    assert_eq!(res, 0815);
}
