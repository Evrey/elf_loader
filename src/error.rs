
use core::fmt;



/// A combined error for all things about toying with ELFs, for your convenience.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ElfError {
    /// An error that might occur while trying to parse ELF data.
    Parse(ParseElfError),

    /// An error that might occur while trying to load ELF segments.
    Load(LoadElfError),

    /// An error that might occur while trying to re-locate and memory-protect a loaded ELF.
    Reloc(RelocElfError),

    #[doc(hidden)] _Reserved,
}



/// An error that might occur while trying to parse ELF data.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u8)]
pub enum ParseElfError {
    /// ELF (section) header does not fit inside this buffer, or the buffer is at least 4GiB big.
    BadBufferSize = 0,

    /// Raw ELF data buffer does not have the correct alignment.
    BadBufferAlignment = 1,

    /// Buffer does not start with the ELF magic number.
    BufferNotElf = 2,

    /// The ELF header reports an ELF header struct size that does not match the struct used by
    /// this loader.
    BadHeaderSize = 3,

    /// The ELF header reports an ELF program header struct size that does not match the struct
    /// used by this loader.
    BadProgramHeaderSize = 4,

    /// This loader currently only supports parsing 64-bit ELF data.
    NotElf64 = 5,

    /// ELF does not contain a position-independent executable.
    NotPic = 6,

    /// The ELF data has an endianness differing from the target system's.
    BadEndian = 7,

    /// The ELF data contains code of an incompatible instruction set architecture (ISA).
    BadIsa = 8,

    /// The reported buffer range of the ELF program headers overflows or goes past the end of the
    /// entire ELF buffer.
    ProgramHeaderOverflow = 9,

    /// The ELF data contains an entry point that does not lie within the `.text` section.
    BadEntry = 10,

    /// A program header requests copying data outside the ELF buffer's range.
    BadPhRange = 11,

    /// A program header wants some impressive virtual memory allocation.
    BadVmemRange = 12,

    /// A program header wants to load more bytes into a segment than the segment is in size.
    PhSmallerThanVmem = 13,

    /// A program header wants to align its segment to more than 4GiB.
    ExcessiveAlignment = 14,

    #[doc(hidden)] _Reserved,
}



/// An error that might occur while trying to load ELF segments.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u8)]
pub enum LoadElfError {
    /// The given buffer is not big enough to load the ELF segments into.
    BadBufferSize = 0,

    /// The given buffer is not properly aligned.
    BadBufferAlignment = 1,

    /// The ELF loader only supports a limited number of segments of different kinds
    /// of memory protection.
    ///
    /// Typically, only 3 or 4 segments of type `LOAD` and 1 of type `GNU_RELRO` are
    /// needed. The ELF loader supports a few more than that. The typical `LOAD`
    /// segments are:
    ///
    /// - `LOAD` with `PF_R | PF_W` for the `DYNAMIC` segment.
    /// - `LOAD` with `PF_R | PF_X` for read-only data and executable code.
    /// - The 4th `LOAD` would be the result of splitting read-only data and code.
    /// - `LOAD` with `PF_R | PF_W` for initialised and uninitialised static data.
    /// - `GNU_RELRO` to make the loaded `DYNAMIC` segment read-only.
    ///
    /// If you get this error, then you most likely want to check your linker script.
    TooManySegments = 2,

    /// The ELF data contains more than one `DYNAMIC` segment. This dead simple ELF
    /// parser/loader only supports one, though.
    ///
    /// In case you encounter this error for non-broken ELF data, you might want to
    /// write a linker script that merges all dynamic re-location info into just one
    /// segment.
    MultipleDynamicSegments = 3,

    /// The ELF data contains no `DYNAMIC` segment. However, this ELF parser/loader
    /// only accepts re-locatable executables.
    NoDynamicSegments = 4,

    #[doc(hidden)] _Reserved,
}



/// An error that might occur while trying to re-locate and memory-protect an ELF.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u8)]
pub enum RelocElfError {
    /// The given base address does not fulfill the ELF's alignment requirements.
    BadBaseAddressAlignment = 0,

    /// The `Dyn` array pointed at by the ELF is out of the ELF's memory region's bounds.
    BadDynRange = 1,

    /// The `Dyn` array pointed at by the ELF is not properly aligned.
    BadDynAlignment = 2,

    /// The `PT_DYNAMIC` segment reported a bad `Rel` size.
    BadRelSize = 3,

    /// The `PT_DYNAMIC` segment reported a bad `Rela` size.
    BadRelaSize = 4,

    /// The `PT_DYNAMIC` segment reported a memory range for the re-location
    /// tables that is out of bounds.
    BadRelRelaTableRange = 5,

    /// The `PT_DYNAMIC` segment reported a memory range for the re-location
    /// tables that is under-aligned.
    BadRelRelaTableAlignment = 6,

    /// A `Rel` table entry wants to modify memory out of range.
    BadRelOffset = 7,

    /// The `Rel` table contains unsupported re-locations.
    UnsupportedRelType = 8,

    /// `Rel` is currently not supported for the target CPU architecture.
    UnsupportedRelArch = 9,

    /// A `Rela` table entry wants to modify memory out of range.
    BadRelaOffset = 10,

    /// The `Rela` table contains unsupported re-locations.
    UnsupportedRelaType = 11,

    /// `Rela` is currently not supported for the target CPU architecture.
    UnsupportedRelaArch = 12,

    /// An attempt of restricting memory access rights for a region of the loaded ELF's
    /// memory failed.
    MemProtectFailed = 13,

    #[doc(hidden)] _Reserved,
}



impl ParseElfError {
    /// Returns a descriptive short string of what the error is about.
    pub fn as_str(&self) -> &'static str {
        use self::ParseElfError::*;

        match *self {
            BadBufferSize         => "The ELF buffer is over 4GiB in size or smaller than a header",
            BadBufferAlignment    => "The ELF buffer is not properly aligned for one of the many \
                                      ELF headers; to be extra sure, page-align your ELF buffer",
            BufferNotElf          => "The ELF buffer does not contain an ELF magic number",
            BadHeaderSize         => "The ELF buffer's reported header size does not match the \
                                      loader's expected header size of 64 bytes",
            BadProgramHeaderSize  => "The ELF buffer's reported program header size does not match \
                                      the loader's expected program header size of 56 bytes",
            NotElf64              => "Currently, this loader only supports the ELF64 format, but \
                                      the given buffer does not contain ELF64 data",
            NotPic                => "The ELF buffer does not contain position-independent code, \
                                      which is not supported - Ensure the ELF type is set to \
                                      `ET_DYN`",
            BadEndian             => "The ELF buffer is not in the native endian format, which is \
                                      currently and probably forever unsupported",
            BadIsa                => "The ELF buffers code is not compiled for the native ISA, as \
                                      in e.g. trying to run RISC-V code on an ARM chip",
            ProgramHeaderOverflow => "The ELF buffer reports a program headers range that goes \
                                      past the end of the buffer or overflows",
            BadEntry              => "The ELF's reported entry point does not lie within the \
                                      virtual address range of an executable segment",
            BadPhRange            => "One of the ELF's program headers reported a physical buffer \
                                      range that goes past the end of the whole ELF buffer",
            BadVmemRange          => "One of the ELF's program headers reported a virtual buffer \
                                      range that is over 4GiB in size or goes past the 4GiB \
                                      virtual address range",
            PhSmallerThanVmem     => "One of the ELF's program headers reported a segment file \
                                      size that is bigger then the loaded ELF's segment's virtual \
                                      memory size",
            ExcessiveAlignment    => "One of the ELF's program headers reported a segment \
                                      alignment to more than 4GiB",

            _Reserved => "",
        }
    }
}

impl fmt::Display for ParseElfError {
    #[inline] fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { f.write_str(self.as_str()) }
}

impl LoadElfError {
    /// Returns a descriptive short string of what the error is about.
    pub fn as_str(&self) -> &'static str {
        use self::LoadElfError::*;

        match *self {
            BadBufferSize           => "The given buffer is not big enough to load the ELF into",
            BadBufferAlignment      => "The given buffer is not properly aligned",
            TooManySegments         => "The program headers describe more than 8 segments", // TODO
            MultipleDynamicSegments => "There is more than one `PT_DYNAMIC` segment",
            NoDynamicSegments       => "There is no `PT_DYNAMIC` segment, but this loader only \
                                        supports re-locatable ELFs",

            _Reserved => "",
        }
    }
}

impl fmt::Display for LoadElfError {
    #[inline] fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { f.write_str(self.as_str()) }
}

impl RelocElfError {
    /// Returns a descriptive short string of what the error is about.
    pub fn as_str(&self) -> &'static str {
        use self::RelocElfError::*;

        match *self {
            BadBaseAddressAlignment  => "The given base address is not page-aligned",
            BadDynRange              => "The `Dyn` array pointed at by the ELF's program headers \
                                         goes past the ELF's memory region's bounds",
            BadDynAlignment          => "The `Dyn` array pointed at by the ELF's program headers \
                                         is not properly aligned for `Dyn` structs",
            BadRelSize               => "The `PT_DYNAMIC` segment reported a struct size of the \
                                         `Rel` array that does not match the loader's expected \
                                         size of 16 bytes",
            BadRelaSize              => "The `PT_DYNAMIC` segment reported a struct size of the \
                                         `Rela` array that does not match the loader's expected \
                                         size of 24 bytes",
            BadRelRelaTableRange     => "The `PT_DYNAMIC` segment reported a `Rel` or `Rela` array \
                                         that goes past the bounds of the loaded ELF's memory \
                                         region",
            BadRelRelaTableAlignment => "The `PT_DYNAMIC` segment reported a `Rel` or `Rela` array \
                                         that is not properly aligned for its element types",
            BadRelOffset             => "A `Rel` table entry wants to modify memory out of range",
            UnsupportedRelArch       => "`Rel` re-locations are not supported on the current \
                                         CPU architecture",
            UnsupportedRelType       => "A `Rel` table entry requires an unsupported re-location \
                                         method",
            BadRelaOffset            => "A `Rela` table entry wants to modify memory out  of range",
            UnsupportedRelaArch      => "`Rela` re-locations are not supported on the current \
                                         CPU architecture",
            UnsupportedRelaType      => "A `Rela` table entry requires an unsupported re-location \
                                         method",
            MemProtectFailed         => "The given memory protection function failed to restrict \
                                         access to a given range of memory",

            _Reserved => "",
        }
    }
}

impl fmt::Display for RelocElfError {
    #[inline] fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { f.write_str(self.as_str()) }
}

impl ElfError {
    /// Returns the descriptive short string of what the sub-error is about.
    pub fn as_str(&self) -> &'static str {
        use self::ElfError::*;

        match *self {
            Parse(e)  => e.as_str(),
            Load( e)  => e.as_str(),
            Reloc(e)  => e.as_str(),
            _Reserved => "",
        }
    }
}

impl fmt::Display for ElfError {
    #[inline] fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::ElfError::*;

        write!(f, "{}: {}",
            match *self {
                Parse(_)  => "Error trying to parse an ELF",
                Load( _)  => "Error trying to load an ELF",
                Reloc(_)  => "Error trying to re-locate and memory-protect an ELF",
                _Reserved => "",
            },
            self.as_str()
        )
    }
}



impl From<ParseElfError> for ElfError {
    #[inline] fn from(e: ParseElfError) -> Self { ElfError::Parse(e) }
}

impl From<LoadElfError> for ElfError {
    #[inline] fn from(e: LoadElfError) -> Self { ElfError::Load(e) }
}

impl From<RelocElfError> for ElfError {
    #[inline] fn from(e: RelocElfError) -> Self { ElfError::Reloc(e) }
}



#[allow(dead_code)]
mod static_assert {
    use core::mem::size_of as sz;
    use crate::elf::*;

    const fn assert(expr: bool) -> () {
        const A: [(); 1] = [()];

        A[(!expr) as usize]
    }

    const SZ_ELF_HDR_64: () = assert(sz::<ElfFileHeader   >() == 64);
    const SZ_PRG_HDR_64: () = assert(sz::<ElfProgramHeader>() == 56);
    const SZ_REL_16:     () = assert(sz::<ElfRel          >() == 16);
    const SZ_RELA_24:    () = assert(sz::<ElfRela         >() == 24);
}
