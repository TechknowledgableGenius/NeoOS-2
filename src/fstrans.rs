use std::ffi::{CStr, CString};
use libc::{c_char, c_int, O_RDWR};

pub const ERROR_INVALID_PARAMETER: u32 = 87;
pub const ERROR_FILE_NOT_FOUND: u32 = 2;

// ==========================================
// 1. Filesystem Translation Core
// ==========================================
/// Translates an OS/2 path into a Linux-compatible filesystem path.
pub fn translate_os2_path(psz_path: *const c_char) -> Result<CString, u32> {
    if psz_path.is_null() {
        return Err(ERROR_INVALID_PARAMETER);
    }

    let c_str = unsafe { CStr::from_ptr(psz_path) };
    let os2_path = match c_str.to_str() {
        Ok(s) => s,
        Err(_) => return Err(ERROR_INVALID_PARAMETER),
    };

    // 1. Swap OS/2 backslashes for POSIX forward slashes
    let mut normalized = os2_path.replace("\\", "/");

    // 2. Handle Drive Letters (e.g., "C:/..." -> "/var/lib/neo_os2/drives/C/...")
    if normalized.len() >= 2 && normalized.chars().nth(1) == Some(':') {
        let drive_letter = normalized.chars().nth(0).unwrap().to_ascii_uppercase();
        // Slice off the "C:" and prepend the Linux virtual filesystem root
        normalized = format!("/var/lib/neo_os2/drives/{}{}", drive_letter, &normalized[2..]);
    } else if normalized.starts_with("/") {
        // Absolute paths without drive letters default to the current working drive (C:)
        normalized = format!("/var/lib/neo_os2/drives/C{}", normalized);
    } else {
        // Relative paths need to be appended to the process's tracked working directory.
        // For demonstration, we assume execution from the root of C:
        normalized = format!("/var/lib/neo_os2/drives/C/{}", normalized);
    }

    // 3. Convert to a CString for the libc::open call
    CString::new(normalized).map_err(|_| ERROR_INVALID_PARAMETER)
}

// ==========================================
// 2. The DosOpen API Bridge
// ==========================================
#[no_mangle]
pub extern "C" fn DosOpen(
    pszFileName: *const c_char,
    /* ... other DosOpen args like pHf (file handle ptr), action, attr, flags ... */
) -> u32 {
    let linux_path = match translate_os2_path(pszFileName) {
        Ok(p) => p,
        Err(err_code) => return err_code,
    };

    // Call native Linux open using the sandboxed path
    // In a full implementation, OS/2 OPEN_FLAGS would map to Linux O_CREAT, O_EXCL, etc.
    let fd = unsafe { libc::open(linux_path.as_ptr(), O_RDWR) };
    if fd < 0 {
        // A robust implementation must check libc::errno to return 
        // ERROR_ACCESS_DENIED, ERROR_FILE_NOT_FOUND, etc.
        return ERROR_FILE_NOT_FOUND;
    }
    
    // The resulting `fd` is a 32-bit integer that perfectly matches 
    // the size of an OS/2 HFILE (handle), so it can be returned directly 
    // (or mapped in a local handle table) to the emulated process.
    
    0 // NO_ERROR
}