use std::ffi::CStr;
use std::ptr;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use libc::{c_char, c_void, size_t, off_t};

// ==========================================
// 1. OS/2 Type Definitions
// ==========================================
pub type ULONG = u32;
pub type PVOID = *mut c_void;
pub type PPVOID = *mut PVOID;
pub type PSZ = *const c_char;
pub type APIRET = u32;
pub type PID = u32;

pub const NO_ERROR: APIRET = 0;
pub const ERROR_INVALID_PARAMETER: APIRET = 87;
pub const ERROR_NOT_ENOUGH_MEMORY: APIRET = 8;
pub const ERROR_ALREADY_EXISTS: APIRET = 183;
pub const ERROR_ACCESS_DENIED: APIRET = 5;

// OS/2 Allocation Flags
pub const PAG_COMMIT: ULONG  = 0x00000010;
pub const PAG_READ: ULONG    = 0x00000001;
pub const PAG_WRITE: ULONG   = 0x00000002;
pub const PAG_EXECUTE: ULONG = 0x00000004;

// ==========================================
// 2. IPC Registration Struct
// ==========================================
#[derive(Debug, Clone)]
#[repr(C)]
pub struct IpcRegisterMemReq {
    pub msg_type: u32,       // Route identifier (e.g., 0x01 for MemReg)
    pub host_base_addr: u64, // The 64-bit mmap host pointer
    pub size: u64,
    pub creator_pid: PID,
    pub shm_name: [u8; 64],  // Fixed buffer for the POSIX shm name
}

// ==========================================
// 3. The Shared Memory Bridge
// ==========================================
#[no_mangle]
pub extern "C" fn DosAllocSharedMem(
    ppb: PPVOID,
    psz_name: PSZ,
    cb: ULONG,
    flags: ULONG
) -> APIRET {
    if ppb.is_null() || cb == 0 {
        return ERROR_INVALID_PARAMETER;
    }

    // 1. Parse and Translate the OS/2 Shared Memory Name
    let mut shm_name = String::new(); 
    
    if !psz_name.is_null() {
        let c_str = unsafe { CStr::from_ptr(psz_name) };
        if let Ok(name_str) = c_str.to_str() {
            if name_str.to_uppercase().starts_with("\\SHAREMEM\\") {
                let stripped = &name_str[10..];
                // Map to Linux /dev/shm friendly name
                shm_name = format!("/neo_os2_{}", stripped.replace("\\", "_"));
            } else {
                return ERROR_INVALID_PARAMETER;
            }
        }
    } else {
        // Unnamed shared memory (giveable via DosGiveSharedMem)
        // Generate a random UUID-style identifier for the POSIX backend
        let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos();
        shm_name = format!("/neo_os2_unnamed_{}", timestamp);
    }

    // 2. Set POSIX Protection Flags
    let mut prot = libc::PROT_NONE;
    if (flags & PAG_READ) != 0 { prot |= libc::PROT_READ; }
    if (flags & PAG_WRITE) != 0 { prot |= libc::PROT_WRITE; }
    if (flags & PAG_EXECUTE) != 0 { prot |= libc::PROT_EXEC; }
    if (flags & PAG_COMMIT) == 0 { prot = libc::PROT_NONE; }

    // 3. Open the Linux /dev/shm File Descriptor
    let shm_name_c = std::ffi::CString::new(shm_name.clone()).unwrap();
    let fd = unsafe {
        libc::shm_open(
            shm_name_c.as_ptr(),
            libc::O_CREAT | libc::O_RDWR,
            0o660
        )
    };

    if fd < 0 {
        return ERROR_ALREADY_EXISTS; 
    }

    // 4. Set Block Size and Map Memory
    unsafe {
        if libc::ftruncate(fd, cb as off_t) != 0 {
            libc::close(fd);
            return ERROR_NOT_ENOUGH_MEMORY;
        }
    }

    let allocated_ptr = unsafe {
        libc::mmap(
            ptr::null_mut(),
            cb as size_t,
            prot,
            libc::MAP_SHARED,
            fd,
            0
        )
    };
    
    unsafe { libc::close(fd); } // Descriptor can be closed after mmap

    if allocated_ptr == libc::MAP_FAILED {
        return ERROR_NOT_ENOUGH_MEMORY;
    }

    // 5. Register with pmserver over Unix Domain Socket
    let mut req = IpcRegisterMemReq {
        msg_type: 0x01, // 0x01 = Register Memory
        host_base_addr: allocated_ptr as u64,
        size: cb as u64,
        creator_pid: std::process::id(), // Use native Linux PID for tracking
        shm_name: [0u8; 64],
    };

    let name_bytes = shm_name.as_bytes();
    let copy_len = name_bytes.len().min(63);
    req.shm_name[..copy_len].copy_from_slice(&name_bytes[..copy_len]);

    if let Ok(mut stream) = UnixStream::connect("/tmp/neo_os2_pmserver.sock") {
        let req_bytes: [u8; std::mem::size_of::<IpcRegisterMemReq>()] = unsafe { std::mem::transmute(req) };
        if stream.write_all(&req_bytes).is_err() {
            // In a production environment, you might unmap the memory here if registration fails
            return ERROR_ACCESS_DENIED; 
        }
        
        let mut res_bytes = [0u8; 16];
        let _ = stream.read_exact(&mut res_bytes); // Await acknowledgement
    } else {
        return ERROR_ACCESS_DENIED;
    }

    // 6. Return 64-bit host pointer for the MMU to translate to a 32-bit guest pointer
    unsafe {
        *ppb = allocated_ptr;
    }

    NO_ERROR
}
