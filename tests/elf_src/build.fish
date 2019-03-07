#!/usr/bin/fish



cargo xbuild --target ./x86_64-unknown-none.json --release
cp ./target/x86_64-unknown-none/release/simple ../simple.elf
cp ./target/x86_64-unknown-none/release/bss_rodata_data ../bss_rodata_data.elf
