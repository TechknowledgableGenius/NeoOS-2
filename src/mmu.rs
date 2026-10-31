use std::collections::BTreeMap;
use std::sync::RwLock;

// Represents a mapped region of memory
#[derive(Clone, Debug)]
pub struct MemoryRegion {
    pub host_ptr: u64,       // The 64-bit pointer returned by libc::mmap
    pub size: u32,
    pub flags: u32,
}

pub struct GuestMMU {
    // Maps a 32-bit Guest Address to a host MemoryRegion
    // Using RwLock to allow thread-safe access within the emulator process
    pub regions: RwLock<BTreeMap<u32, MemoryRegion>>,
    pub next_guest_addr: u32,
}

impl GuestMMU {
    pub fn new() -> Self {
        GuestMMU {
            regions: RwLock::new(BTreeMap::new()),
            // Start guest allocations at a safe offset (e.g., 64MB mark)
            next_guest_addr: 0x0400_0000, 
        }
    }

    /// Maps a real 64-bit host pointer into the 32-bit guest space
    pub fn map_to_guest(&mut self, host_ptr: u64, size: u32, flags: u32) -> u32 {
        let mut regions = self.regions.write().unwrap();
        
        // Page-align the size
        let aligned_size = (size + 0xFFF) & !0xFFF;
        let guest_addr = self.next_guest_addr;
        
        regions.insert(guest_addr, MemoryRegion {
            host_ptr,
            size: aligned_size,
            flags,
        });

        // Bump allocator for the next guest mapping
        self.next_guest_addr += aligned_size;
        guest_addr
    }

    /// Translates a 32-bit guest pointer back to a 64-bit host pointer for read/write
    pub fn translate(&self, guest_ptr: u32) -> Option<u64> {
        let regions = self.regions.read().unwrap();
        // Find the region containing the guest_ptr
        if let Some((&base_addr, region)) = regions.range(..=guest_ptr).next_back() {
            let offset = guest_ptr - base_addr;
            if offset < region.size {
                return Some(region.host_ptr + offset as u64);
            }
        }
        None // Segmentation Fault!
    }
}