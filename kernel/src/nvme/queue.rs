use core::ptr;

use x86_64::{PhysAddr, VirtAddr};

use super::NvmeError;
use super::regs::{NvmeCqe, NvmeSqe};

pub struct SubmissionQueue {
    /// Virtual address of the queue entries in the direct-mapped region (for CPU access).
    base_virt: VirtAddr,
    /// Physical address of the queue entries (given to the controller via ASQ or Create IO SQ).
    base_phys: PhysAddr,
    /// Index of the next slot to write a command into. Wraps around at `size`.
    tail: u16,
    /// Number of entries in the queue.
    size: u16,
    /// MMIO pointer to the tail doorbell register. Writing `tail` here tells the controller
    /// that new commands are available.
    doorbell: *mut u32,
    /// Monotonically increasing counter for assigning unique command IDs.
    cid_counter: u16,
}

// SAFETY: accessed through a Mutex in NvmeController
unsafe impl Send for SubmissionQueue {}
unsafe impl Sync for SubmissionQueue {}

impl SubmissionQueue {
    pub fn new(base_virt: VirtAddr, base_phys: PhysAddr, size: u16, doorbell: *mut u32) -> Self {
        Self {
            base_virt,
            base_phys,
            tail: 0,
            size,
            doorbell,
            cid_counter: 0,
        }
    }

    pub fn phys(&self) -> PhysAddr {
        self.base_phys
    }

    /// Submit a command to the queue. Sets the CID field automatically.
    /// Returns the assigned CID.
    pub fn submit(&mut self, cmd: &NvmeSqe) -> u16 {
        let cid = self.cid_counter;
        self.cid_counter = self.cid_counter.wrapping_add(1);

        let mut entry = *cmd;
        entry.set_cid(cid);

        let slot = self.tail as usize;
        let entry_ptr = (self.base_virt + (slot * 64) as u64).as_mut_ptr::<NvmeSqe>();
        unsafe { ptr::write_volatile(entry_ptr, entry) };

        // advance tail and ring doorbell
        self.tail = (self.tail + 1) % self.size;
        unsafe { ptr::write_volatile(self.doorbell, self.tail as u32) };

        cid
    }
}

pub struct CompletionQueue {
    /// Virtual address of the queue entries in the direct-mapped region (for CPU access).
    base_virt: VirtAddr,
    /// Physical address of the queue entries (given to the controller via ACQ or Create IO CQ).
    base_phys: PhysAddr,
    /// Index of the next entry to read. Advanced after consuming an entry.
    head: u16,
    /// Number of entries in the queue.
    size: u16,
    /// Expected phase bit. The controller toggles bit 0 of each CQE's status field
    /// every time it wraps around the queue. We track this to distinguish new entries
    /// from stale ones.
    phase: bool,
    /// MMIO pointer to the head doorbell register. Writing `head` here tells the controller
    /// we've consumed entries up to that index.
    doorbell: *mut u32,
}

// SAFETY: accessed through a Mutex in NvmeController
unsafe impl Send for CompletionQueue {}
unsafe impl Sync for CompletionQueue {}

impl CompletionQueue {
    pub fn new(base_virt: VirtAddr, base_phys: PhysAddr, size: u16, doorbell: *mut u32) -> Self {
        Self {
            base_virt,
            base_phys,
            head: 0,
            size,
            phase: true, // hardware starts with phase = 1
            doorbell,
        }
    }

    pub fn phys(&self) -> PhysAddr {
        self.base_phys
    }

    /// Poll for a completion entry. Returns None if no new entry is available.
    fn poll(&mut self) -> Option<NvmeCqe> {
        let slot = self.head as usize;
        let entry_ptr = (self.base_virt + (slot * 16) as u64).as_ptr::<NvmeCqe>();
        let entry = unsafe { ptr::read_volatile(entry_ptr) };

        if entry.phase() != self.phase {
            return None;
        }

        // advance head
        self.head += 1;
        if self.head >= self.size {
            self.head = 0;
            self.phase = !self.phase; // flip expected phase on wrap
        }

        // ring doorbell
        unsafe { ptr::write_volatile(self.doorbell, self.head as u32) };

        Some(entry)
    }

    /// Spin-poll until a completion arrives or timeout.
    pub fn wait(&mut self) -> Result<NvmeCqe, NvmeError> {
        // hopefully ~500ms
        for _ in 0..500_000_000u32 {
            if let Some(cqe) = self.poll() {
                return Ok(cqe);
            }
            core::hint::spin_loop();
        }
        Err(NvmeError::Timeout)
    }
}
