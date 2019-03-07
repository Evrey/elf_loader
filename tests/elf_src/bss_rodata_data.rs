
#![no_std]
#![no_main]



#[no_mangle]
pub extern "C" fn _start(test: &mut u32) -> bool {
    let v = *test;

    if v == 0xDEADBEEF { return true; }

    *test = 0xDEADBEEF + RODATA[(v & 0x0F) as usize];

    assert_ne!(unsafe { DATA }, 0); // Force `.dynstr` section.
    unsafe { DATA = v };
    unsafe { BSS  = v };

    v == 0xFF7_42_FF7
}



#[panic_handler]
fn panic_handler(_info: &::core::panic::PanicInfo) -> ! {
    loop {}
}



// Force `.rodata` section.
static RODATA: [u32; 16] = [
    b'0' as u32, b'1' as u32, b'2' as u32, b'3' as u32,
    b'4' as u32, b'5' as u32, b'6' as u32, b'7' as u32,
    b'8' as u32, b'9' as u32, b'A' as u32, b'B' as u32,
    b'C' as u32, b'D' as u32, b'E' as u32, b'F' as u32,
];

// Force `.data` section.
#[no_mangle]
pub static mut DATA: u32 = 0xFF9_CC_FF9;

// Force `.bss` section.
#[no_mangle]
pub static mut BSS: u32 = 0;
