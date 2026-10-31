use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::os::unix::net::{UnixListener, UnixStream};
use std::io::{Read, Write};
use std::thread;
use std::fs;

type PID = u32;

#[derive(Clone, Debug)]
pub struct SharedMemBlock {
    pub shm_name: String,      // The Linux /dev/shm object name (e.g., /neo_os2_uuid-1234)
    pub size: u64,
    pub creator_pid: PID,
    pub allowed_pids: HashSet<PID>, // PIDs that have been 'Given' access
}

pub struct PmServerMemRegistry {
    // Maps the creator's 32-bit guest base address to the shared block metadata
    pub unnamed_blocks: Mutex<HashMap<u32, SharedMemBlock>>,
    // Maps OS/2 names (e.g., "\SHAREMEM\DATA") to shared block metadata
    pub named_blocks: Mutex<HashMap<String, SharedMemBlock>>,
}

impl PmServerMemRegistry {
    /// Called when Process A uses DosGiveSharedMem to Process B
    pub fn give_access(&self, guest_addr: u32, source_pid: PID, target_pid: PID) -> Result<(), u32> {
        let mut blocks = self.unnamed_blocks.lock().unwrap();
        
        if let Some(block) = blocks.get_mut(&guest_addr) {
            // Security: Only the creator (or already allowed PIDs) can give it away
            if block.creator_pid == source_pid || block.allowed_pids.contains(&source_pid) {
                block.allowed_pids.insert(target_pid);
                return Ok(());
            }
        }
        Err(5) // ERROR_ACCESS_DENIED
    }

    /// Called when Process B uses DosGetSharedMem
    pub fn verify_access(&self, guest_addr: u32, requesting_pid: PID) -> Option<SharedMemBlock> {
        let blocks = self.unnamed_blocks.lock().unwrap();
        if let Some(block) = blocks.get(&guest_addr) {
            if block.allowed_pids.contains(&requesting_pid) {
                return Some(block.clone());
            }
        }
        None
    }
}

// ==========================================
// Linux Daemon Listener
// ==========================================
pub fn run_registry_daemon() {
    let socket_path = "/tmp/neo_os2_pmserver.sock";
    
    // Clean up stale socket file from previous crashes
    let _ = fs::remove_file(socket_path);
    
    let listener = match UnixListener::bind(socket_path) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Fatal: Could not bind pmserver socket to {}: {}", socket_path, e);
            std::process::exit(1);
        }
    };
    
    println!("Neo OS/2 PM Server listening on {}", socket_path);

    // Wrap the registry in an Arc so it can be shared across client connection threads
    let registry = Arc::new(PmServerMemRegistry {
        unnamed_blocks: Mutex::new(HashMap::new()),
        named_blocks: Mutex::new(HashMap::new()),
    });

    // Main event loop for accepting IPC connections
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let registry_clone = Arc::clone(&registry);
                thread::spawn(move || {
                    handle_ipc_client(stream, registry_clone);
                });
            }
            Err(err) => eprintln!("Failed to accept incoming IPC connection: {}", err),
        }
    }
}

/// Routes incoming IPC requests (WinSendMsg, DosGetSharedMem, etc.)
fn handle_ipc_client(mut stream: UnixStream, _registry: Arc<PmServerMemRegistry>) {
    // Determine message type (in a full implementation, you'd read a standardized header first)
    // For this bridge, we expect the 24-byte payload from winsendmsg.rs or memory requests
    let mut req_bytes = [0u8; 24];
    
    if stream.read_exact(&mut req_bytes).is_ok() {
        println!("Received IPC request");
        
        // Handle routing logic based on the payload...
        
        // Respond to unblock the caller (16-byte response aligned for Linux repr(C))
        let res_bytes = [0u8; 16]; 
        let _ = stream.write_all(&res_bytes);
    }
}