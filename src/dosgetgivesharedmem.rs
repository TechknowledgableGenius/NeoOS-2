use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::ptr;
use libc::{c_void, size_t};

// ==========================================
// 1. OS/2 Type Definitions & Constants
// ==========================================
pub type ULONG = u32;
pub type PVOID = *mut c_void;
pub type PID = u32;
pub type APIRET = u32;

pub const NO_ERROR: APIRET = 0;
pub const ERROR_INVALID_PARAMETER: APIRET = 87;
pub const ERROR_ACCESS_DENIED: APIRET = 5;
pub const ERROR_NOT_ENOUGH_MEMORY: APIRET = 8;

pub const PAG_READ: ULONG    = 0x00000001;
pub const PAG_WRITE: ULONG   = 0x00000002;
pub const PAG_EXECUTE: ULONG = 0x00000004;

// ==========================================
// 2. IPC Structs for Linux pmserver Coordination
// ==========================================
#[derive(Debug, Clone)]
#[repr(C)]
pub struct IpcGiveMemReq {
    pub msg_type: u32,     // 0x02 = DosGiveSharedMem
    pub target_pid: PID,
    pub base_address: u64, // PVOID cast to u64 for safe transit
    pub flags: ULONG,
    pub pad: u32,          // Padding to ensure 8-byte alignment
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct IpcGetMemReq {
    pub msg_type: u32,     // 0x03 = DosGetSharedMem
    pub pad: u32,          // Padding for alignment
    pub base_address: u64,
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct IpcGetMemRes {
    pub status: APIRET,
    pub pad: u32,
    pub size: u64,
    pub shm_name: [u8; 64], // Fixed buffer for the Linux /dev/shm UUID string
}

// ==========================================
// 3. DosGiveSharedMem Linux Bridge
// ==========================================
#[no_mangle]
pub extern "C" fn DosGiveSharedMem(pb: PVOID, pid: PID, flags: ULONG) -> APIRET {
    if pb.is_null() {
        return ERROR_INVALID_PARAMETER;
    }

    let req = IpcGiveMemReq {
        msg_type: 0x02,
        target_pid: pid,
        base_address: pb as u64,
        flags,
        pad: 0,
    };

    let mut stream = match UnixStream::connect("/tmp/neo_os2_pmserver.sock") {
        Ok(s) => s,
        Err(_) => return ERROR_ACCESS_DENIED,
    };

    let req_bytes: [u8; std::mem::size_of::<IpcGiveMemReq>()] = unsafe { std::mem::transmute(req) };
    if stream.write_all(&req_bytes).is_err() {
        return ERROR_ACCESS_DENIED;
    }

    let mut res_bytes = [0u8; 4];
    if stream.read_exact(&mut res_bytes).is_err() {
        return ERROR_ACCESS_DENIED;
    }

    u32::from_ne_bytes(res_bytes)
}

// ==========================================
// 4. DosGetSharedMem Linux Bridge
// ==========================================
#[no_mangle]
pub extern "C" fn DosGetSharedMem(pb: PVOID, flags: ULONG) -> APIRET {
    if pb.is_null() {
        return ERROR_INVALID_PARAMETER;
    }

    let req = IpcGetMemReq {
        msg_type: 0x03,
        pad: 0,
        base_address: pb as u64,
    };

    let mut stream = match UnixStream::connect("/tmp/neo_os2_pmserver.sock") {
        Ok(s) => s,
        Err(_) => return ERROR_ACCESS_DENIED,
    };

    let req_bytes: [u8; std::mem::size_of::<IpcGetMemReq>()] = unsafe { std::mem::transmute(req) };
    if stream.write_all(&req_bytes).is_err() {
        return ERROR_ACCESS_DENIED;
    }

    let mut res_bytes = [0u8; std::mem::size_of::<IpcGetMemRes>()];
    if stream.read_exact(&mut res_bytes).is_err() {
        return ERROR_ACCESS_DENIED;
    }

    let res: IpcGetMemRes = unsafe { std::mem::transmute(res_bytes) };
    if res.status != NO_ERROR {
        return res.status;
    }

    // Extract the UUID name for the Linux /dev/shm filesystem
    let name_str = match std::ffi::CStr::from_bytes_until_nul(&res.shm_name) {
        Ok(s) => s,
        Err(_) => return ERROR_INVALID_PARAMETER,
    };

    let mut prot = libc::PROT_NONE;
    if (flags & PAG_READ) != 0 { prot |= libc::PROT_READ; }
    if (flags & PAG_WRITE) != 0 { prot |= libc::PROT_WRITE; }
    if (flags & PAG_EXECUTE) != 0 { prot |= libc::PROT_EXEC; }

    // Open the underlying Linux shared memory file descriptor
    let fd = unsafe { libc::shm_open(name_str.as_ptr(), libc::O_RDWR, 0) };
    if fd < 0 {
        return ERROR_ACCESS_DENIED;
    }

    // MAP_FIXED safely forces the POSIX memory block into the exact same 
    // virtual address expected by the OS/2 emulator process
    let allocated_ptr = unsafe {
        libc::mmap(
            pb,
            res.size as size_t,
            prot,
            libc::MAP_SHARED | libc::MAP_FIXED,
            fd,
            0
        )
    };

    unsafe { libc::close(fd); }

    if allocated_ptr == libc::MAP_FAILED {
        return ERROR_NOT_ENOUGH_MEMORY;
    }

    NO_ERROR
}