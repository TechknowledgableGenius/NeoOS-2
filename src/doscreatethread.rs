use std::thread;
use std::sync::{Arc, Mutex};
use crate::mmu::GuestMMU;

pub type ULONG = u32;
pub type TID = u32;
pub type PFNTHREAD = u32; 
pub type APIRET = u32;

pub const NO_ERROR: APIRET = 0;
pub const ERROR_MAX_THRDS_REACHED: APIRET = 164;

pub struct CpuState {
    pub eax: u32, pub ebx: u32, pub ecx: u32, pub edx: u32,
    pub eip: u32, pub esp: u32, pub eflags: u32,
}

pub struct Emulator {
    pub cpu: CpuState,
    pub mmu: Arc<Mutex<GuestMMU>>, // Wrap the MMU in a Mutex
}

impl Emulator {
    pub fn run_loop(&mut self) {}
}

fn global_mmu() -> Arc<Mutex<GuestMMU>> {
    Arc::new(Mutex::new(GuestMMU::new()))
}

// Prefixed with underscore to silence the unused variable warning
fn allocate_host_stack(_size: ULONG) -> u64 {
    0x1000_0000 
}

fn generate_new_tid() -> TID {
    static mut NEXT_TID: TID = 1;
    unsafe {
        let tid = NEXT_TID;
        NEXT_TID += 1;
        tid
    }
}

#[no_mangle]
pub extern "C" fn DosCreateThread(
    ptid: *mut TID,
    pfn: PFNTHREAD,
    param: ULONG,
    _flag: ULONG,
    cbstack: ULONG,
) -> APIRET {
    let mmu_ref = global_mmu();
    
    // Acquire the lock before mutating the MMU
    let guest_stack_base = mmu_ref.lock().unwrap().map_to_guest(allocate_host_stack(cbstack), cbstack, 0x03); 
    let guest_esp = guest_stack_base + cbstack;

    let thread_mmu = mmu_ref.clone();

    thread::spawn(move || {
        let thread_cpu = CpuState {
            eax: param, ebx: 0, ecx: 0, edx: 0,
            eip: pfn,       
            esp: guest_esp, 
            eflags: 0x202,
        };

        let mut thread_emulator = Emulator {
            cpu: thread_cpu,
            mmu: thread_mmu,
        };

        thread_emulator.run_loop();
    });

    let new_tid = generate_new_tid();
    unsafe {
        if !ptid.is_null() {
            *ptid = new_tid;
        }
    }

    NO_ERROR
}
