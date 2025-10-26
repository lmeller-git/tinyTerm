OUTPUT_FORMAT(elf64-x86-64)

ENTRY(main)

PHDRS
{
    text    PT_LOAD;
    rodata  PT_LOAD;
    data    PT_LOAD;
}

SECTIONS
{
    . = 0x400000;
    
    .text : {
        *(.text .text.*)
    } :text

    /* Move to the next memory page for .rodata */
    . = ALIGN(4096);

    .rodata : {
        *(.rodata .rodata.*)
    } :rodata

    /* Move to the next memory page for .data */
    . = ALIGN(4096);

    .data : {
        *(.data .data.*)
    } :data

    . = ALIGN(4096);
    
    .bss : {
      *(.bss .bss.*)
    } :data
    
    /DISCARD/ : {
        *(.note .note.*)
        *(.note.GNU-stack)
    }
}
