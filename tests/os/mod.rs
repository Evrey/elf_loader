
use std::ops::Range;
use crate::SegmentProtection;



pub fn alloc_aligned(len: usize, align: usize) -> &'static mut [u8] {
    self::os_impl::alloc_aligned(len, align)
}

pub extern "C" fn protection_fn(
    prot:    SegmentProtection,
    p_base:  *mut u8,
    v_base:  *mut u8,
    mem_len: usize,
    range:   Range<usize>
) -> Result<(), ()> {
    self::os_impl::protection_fn(prot, p_base, v_base, mem_len, range)
}



#[cfg(target_os = "linux")]
mod os_impl {
    use libc::*;
    use std::ptr;
    use std::slice;
    use std::ffi::c_void;
    use std::ops::Range;
    use crate::SegmentProtection;

    pub fn alloc_aligned(len: usize, align: usize) -> &'static mut [u8] {
        let mut mem_p = unsafe { mmap(
            ptr::null_mut(),
            len + align,
            PROT_WRITE,
            MAP_PRIVATE | MAP_ANONYMOUS,
            -1,
            0
        ) };

        assert_ne!(mem_p, MAP_FAILED);

        mem_p = (((mem_p as usize) + (align - 1)) & (!(align -1))) as *mut c_void;

        assert_eq!((mem_p as usize) % align, 0);

        unsafe { slice::from_raw_parts_mut(mem_p as *mut u8, len) }
    }

    pub fn protection_fn(
        prot:    SegmentProtection,
        p_base:  *mut u8,
        v_base:  *mut u8,
        mem_len: usize,
        range:   Range<usize>
    ) -> Result<(), ()> {
        let mem = unsafe { slice::from_raw_parts_mut(p_base, mem_len) };
        let seg = &mut mem[range];
        let _   = v_base;
        let prt = match prot {
            SegmentProtection::RO => PROT_READ,
            SegmentProtection::RW => PROT_READ | PROT_WRITE,
            SegmentProtection::RX => PROT_READ | PROT_EXEC,
        };

        let res = unsafe { mprotect(seg.as_mut_ptr() as *mut c_void, seg.len(), prt) };

        if res == 0 { Ok(()) }
        else {
            println!("`protection_fn`: {:#010X}, {}", res, res);
            Err(())
        }
    }
}
