use std::ffi::CStr;
use std::ptr;
use libc::{c_char, c_void, size_t, stat, O_RDWR};

// ==========================================
// 1. OS/2 Type Definitions
// ==========================================
pub type ULONG = u32;
pub type PVOID = *mut c_void;
pub type PPVOID = *mut PVOID;
pub type PSZ = *const c_char;
pub type APIRET = u32;

pub const NO_ERROR: APIRET = 0;
pub const ERROR_FILE_NOT_FOUND: APIRET = 2;
pub const ERROR_ACCESS_DENIED: APIRET = 5;
pub const ERROR_NOT_ENOUGH_MEMORY: APIRET = 8;
pub const ERROR_INVALID_PARAMETER: APIRET = 87;

pub const PAG_READ: ULONG    = 0x00000001;
pub const PAG_WRITE: ULONG   = 0x00000002;

// ==========================================
// 2. The Linux Named Memory Bridge
// ==========================================
#[no_mangle]
pub extern "C" fn DosGetNamedSharedMem(
    ppb: PPVOID,
    psz_name: PSZ,
    flags: ULONG
) -> APIRET {
    if ppb.is_null() || psz_name.is_null() {
        return ERROR_INVALID_PARAMETER; 
    }

    // 1. Translate the OS/2 name to the Linux /dev/shm format
    let c_str = unsafe { CStr::from_ptr(psz_name) };
    let name_str = match c_str.to_str() {
        Ok(s) => {
            // Using raw strings (r"...") prevents escape character parsing errors
            if s.to_uppercase().starts_with(r"\SHAREMEM\") {
                format!("/neo_os2_{}", s[10..].replace(r"\", "_"))
            } else {
                return ERROR_INVALID_PARAMETER;
            }
        }
        Err(_) => return ERROR_INVALID_PARAMETER,
    };

    // 2. Open the existing Linux shared memory file descriptor (No O_CREAT)
    let shm_name_c = std::ffi::CString::new(name_str).unwrap();
    let fd = unsafe {
        libc::shm_open(
            shm_name_c.as_ptr(),
            O_RDWR, 
            0
        )
    };

    if fd < 0 {
        return ERROR_FILE_NOT_FOUND; 
    }

    // 3. Query the size of the existing shared memory block via fstat
    let mut file_stat: stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(fd, &mut file_stat) } < 0 {
        unsafe { libc::close(fd); }
        return ERROR_ACCESS_DENIED;
    }
    let cb = file_stat.st_size as size_t;

    // 4. Map to POSIX protection flags
    let mut prot = libc::PROT_NONE;
    if (flags & PAG_READ) != 0 { prot |= libc::PROT_READ; }
    if (flags & PAG_WRITE) != 0 { prot |= libc::PROT_WRITE; }

    // 5. Map into the 64-bit Linux host space
    let allocated_ptr = unsafe {
        libc::mmap(
            ptr::null_mut(),
            cb,
            prot,
            libc::MAP_SHARED, 
            fd,
            0
        )
    };

    unsafe { libc::close(fd); } 

    if allocated_ptr == libc::MAP_FAILED {
        return ERROR_NOT_ENOUGH_MEMORY;
    }

    // 6. Return the host pointer for the MMU
    unsafe {
        *ppb = allocated_ptr;
    }

    NO_ERROR
}
