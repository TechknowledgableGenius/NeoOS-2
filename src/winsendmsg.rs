use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

// ==========================================
// 1. OS/2 Types & IPC Structures
// ==========================================
pub type HWND = u32;
pub type ULONG = u32;
pub type MRESULT = u64; // The synchronous return value

// The payload sent over the Linux Unix Domain Socket
#[derive(Debug, Clone)]
#[repr(C)]
pub struct IpcSyncMessageReq {
    pub target_hwnd: HWND,
    pub msg: ULONG,
    pub mp1: u64,
    pub mp2: u64,
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct IpcSyncMessageRes {
    pub result: MRESULT,
    pub error_code: ULONG,
}

// ==========================================
// 2. Local State Checking
// ==========================================
/// Checks if the window belongs to the current process thread
fn is_local_window(hwnd: HWND) -> bool {
    // In production, this queries a thread-local hash map of HWNDs
    // created by WinCreateWindow in this specific process.
    hwnd < 1000 // Mock logic: assume low HWNDs are local
}

/// Directly executes the local Window Procedure
fn execute_local_wndproc(hwnd: HWND, _msg: ULONG, _mp1: u64, _mp2: u64) -> MRESULT {
    // Fetch the function pointer registered during WinRegisterClass
    println!("Executing local WndProc for HWND: {}", hwnd);
    0 // Mock MRESULT
}

// ==========================================
// 3. The API Entry Point
// ==========================================
#[no_mangle]
pub extern "C" fn WinSendMsg(hwnd: HWND, msg: ULONG, mp1: u64, mp2: u64) -> MRESULT {
    // 1. Fast Path: Local Execution
    if is_local_window(hwnd) {
        return execute_local_wndproc(hwnd, msg, mp1, mp2);
    }

    // 2. Slow Path: Cross-Process RPC via pmserver
    let req = IpcSyncMessageReq { target_hwnd: hwnd, msg, mp1, mp2 };
    
    // Connect to the central Presentation Manager Server daemon on Linux
    // using standard Unix Domain Sockets
    let mut stream = match UnixStream::connect("/tmp/neo_os2_pmserver.sock") {
        Ok(s) => s,
        Err(_) => return 0, // Fallback/Error state
    };

    // Apply a strict timeout to prevent catastrophic cross-process deadlocks
    stream.set_read_timeout(Some(Duration::from_millis(5000))).unwrap();

    // Serialize and send the request
    // 24 bytes (4 + 4 + 8 + 8)
    let req_bytes: [u8; 24] = unsafe { std::mem::transmute(req) };
    if stream.write_all(&req_bytes).is_err() {
        return 0; // Communication failure
    }

    // 3. Block and wait for the target process to execute and return
    // 16 bytes due to repr(C) alignment (8 + 4 + 4 bytes of padding)
    let mut res_bytes = [0u8; 16];
    if stream.read_exact(&mut res_bytes).is_err() {
        // Timeout or target process crash
        return 0; 
    }

    // Deserialize the response and return the MRESULT to the legacy app
    let res: IpcSyncMessageRes = unsafe { std::mem::transmute(res_bytes) };
    res.result
}
