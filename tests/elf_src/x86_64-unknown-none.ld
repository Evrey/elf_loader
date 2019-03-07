
OUTPUT_FORMAT(elf64-x86-64)

ENTRY(_start)

SECTIONS {
    . = 0;

    .dynamic . : {
        *(.dynamic)
        *(.dynamic.*)
    }
    .got : {
        *(.got)
        *(.got.*)
    }

    .dynsym ALIGN(4096) : {
        *(.dynsym)
        *(.dynsym.*)
    }
    .dynstr : {
        *(.dynstr)
        *(.dynstr.*)
    }
    .rela.dyn : {
        *(.rela.dyn)
        *(.rela.dyn.*)
    }
    .rodata : {
        *(.rodata)
        *(.rodata.*)
    }

    .text ALIGN(4096) : {
        *(.text)
        *(.text.*)
    }

    .data ALIGN(4096) : {
        *(.data)
        *(.data.*)
    }
    .bss : {
        *(.bss)
        *(.bss.*)
    }

    /DISCARD/ : {
        *(.gnu.hash)
        *(.gnu.hash.*)
        *(.hash)
        *(.hash.*)
        *(.symtab)
        *(.symtab.*)
        *(.comment)
        *(.comment.*)
    }
}
