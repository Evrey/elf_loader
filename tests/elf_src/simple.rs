
#![no_std]
#![no_main]

#[no_mangle]
pub extern "C" fn _start() -> i32 {
    0815
}

// Fuck this shit! Dead code elimitation should make this unnecessary.
#[panic_handler]
fn panic(_: &::core::panic::PanicInfo) -> ! {
    unsafe { ::core::hint::unreachable_unchecked() }
}
