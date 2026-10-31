use std::ptr;

// ==========================================
// 1. OS/2 Type Definitions
// ==========================================
pub type ULONG = u32;
pub type PVOID = *mut libc::c_void;
pub type PPVOID = *mut PVOID;
pub type APIRET = u32;

// OS/2 Return Codes
pub const NO_ERROR: APIRET = 0;
pub const ERROR_INVALID_PARAMETER: APIRET = 87;
pub const ERROR_NOT_ENOUGH_MEMORY: APIRET = 8;

// OS/2 Allocation Flags (Control Program)
pub const PAG_COMMIT: ULONG  = 0x00000010;
pub const PAG_EXECUTE: ULONG = 0x00000004;
pub const PAG_READ: ULONG    = 0x00000001;
pub const PAG_WRITE: ULONG   = 0x00000002;
pub const OBJ_TILE: ULONG    = 0x00000040; // 16-bit alias mapping (usually ignored in 32-bit)

// ==========================================
// 2. The Memory Allocation Bridge
// ==========================================
/// Bridges the legacy OS/2 DosAllocMem call to Redox OS native memory mapping.
#[no_mangle]
pub extern "C" fn DosAllocMem(ppb: PPVOID, cb: ULONG, flags: ULONG) -> APIRET {
    // 1. Validate inputs
    if ppb.is_null() || cb == 0 {
        return ERROR_INVALID_PARAMETER;
    }

    // 2. Translate OS/2 PAG_* flags to POSIX PROT_* flags
    let mut prot = libc::PROT_NONE;
    
    if (flags & PAG_READ) != 0 {
        prot |= libc::PROT_READ;
    }
    if (flags & PAG_WRITE) != 0 {
        prot |= libc::PROT_WRITE;
    }
    if (flags & PAG_EXECUTE) != 0 {
        prot |= libc::PROT_EXEC;
    }

    // 3. Handle Allocation Strategy
    // OS/2 allows allocating virtual memory space without committing it to physical RAM.
    // If PAG_COMMIT is not set, we are just reserving address space. 
    // On Redox/POSIX, we handle this by mapping with PROT_NONE.
    if (flags & PAG_COMMIT) == 0 {
        prot = libc::PROT_NONE;
    }

    // 4. Request Memory from the Redox Kernel via libc::mmap
    // MAP_PRIVATE and MAP_ANON are used to allocate standard application RAM.
    let mmap_flags = libc::MAP_PRIVATE | libc::MAP_ANONYMOUS;
    
    let allocated_ptr = unsafe {
        libc::mmap(
            ptr::null_mut(),
            cb as libc::size_t,
            prot,
            mmap_flags,
            -1, // No file descriptor for anonymous memory
            0,  // No offset
        )
    };

    // 5. Handle Allocation Failures
    if allocated_ptr == libc::MAP_FAILED {
        return ERROR_NOT_ENOUGH_MEMORY;
    }

    // 6. Return the pointer to the caller
    // IMPORTANT: allocated_ptr is a 64-bit host pointer. The x86 emulator must intercept 
    // this and map it into its 32-bit guest address translation table before handing it 
    // to the legacy OS/2 binary.
    unsafe {
        *ppb = allocated_ptr;
    }

    NO_ERROR
}

// ==========================================
// 3. The Memory Deallocation Bridge (Free)
// ==========================================
#[no_mangle]
pub extern "C" fn DosFreeMem(pb: PVOID) -> APIRET {
    if pb.is_null() {
        return ERROR_INVALID_PARAMETER;
    }

    // In a full implementation, you must track the size (cb) of each allocation 
    // in a HashMap<PVOID, ULONG> because DosFreeMem does not receive the size, 
    // but libc::munmap requires it. 
    
    // Example placeholder for unmapping:
    // let cb = internal_allocation_table.get(&pb).unwrap();
    // unsafe { libc::munmap(pb, cb as libc::size_t) };

    NO_ERROR
}