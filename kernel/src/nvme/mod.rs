// See NVMe specification 2.3 for details
// https://nvmexpress.org/wp-content/uploads/NVM-Express-Base-Specification-Revision-2.3-2025.08.01-Ratified.pdf

pub mod queue;
pub mod regs;

use core::ptr;
use spin::{Mutex, Once};
use x86_64::{PhysAddr, VirtAddr};

use crate::memory;
use crate::pci;
use crate::pci::MassStorageSubclass;
use crate::pci::PciClass;
use queue::{CompletionQueue, SubmissionQueue};
use regs::*;

/// Queue size (number of entries).
/// 16 is small but good enough for single-command-at-a-time usage.
const QUEUE_SIZE: u16 = 16;

#[derive(Debug)]
pub enum NvmeError {
    Timeout,
    CommandError(u16),
    BufferTooSmall,
    NoDevice,
}

pub struct NvmeController {
    #[allow(dead_code)]
    regs: *mut NvmeRegisters,
    #[allow(dead_code)]
    admin_sq: SubmissionQueue,
    #[allow(dead_code)]
    admin_cq: CompletionQueue,

    io_sq: SubmissionQueue,
    io_cq: CompletionQueue,
    dma_buf_virt: VirtAddr,
    dma_buf_phys: PhysAddr,
}

// SAFETY: accessed through a Mutex
unsafe impl Send for NvmeController {}
unsafe impl Sync for NvmeController {}

impl NvmeController {
    // Assumptions:
    // - NSID 1 exists (true for all consumer drives)
    // - Block size is 512 bytes (could be 4096 on some drives; read Identify Namespace FLBAS/LBAF to check)
    // - No IOMMU (VT-d) blocking DMA to our physical addresses
    pub fn read_sectors(
        &mut self,
        lba: u64,
        count: u16,
        buffer: &mut [u8],
    ) -> Result<(), NvmeError> {
        let byte_count = count as usize * 512;
        if buffer.len() < byte_count || byte_count > 4096 {
            return Err(NvmeError::BufferTooSmall);
        }

        let mut cmd = NvmeSqe::default();
        cmd.set_opcode(IO_CMD_READ);
        cmd.nsid = 1;
        cmd.prp1 = self.dma_buf_phys.as_u64();
        cmd.cdw10 = lba as u32;
        cmd.cdw11 = (lba >> 32) as u32;
        cmd.cdw12 = (count - 1) as u32; // 0-based

        self.io_sq.submit(&cmd);
        let cqe = self.io_cq.wait()?;

        let status = cqe.status_code();
        if status != 0 {
            return Err(NvmeError::CommandError(status));
        }

        // copy from bounce buffer to caller
        let src = self.dma_buf_virt.as_ptr::<u8>();
        unsafe { core::ptr::copy_nonoverlapping(src, buffer.as_mut_ptr(), byte_count) };
        Ok(())
    }

    pub fn write_sectors(&mut self, lba: u64, count: u16, buffer: &[u8]) -> Result<(), NvmeError> {
        let byte_count = count as usize * 512;
        if buffer.len() < byte_count || byte_count > 4096 {
            return Err(NvmeError::BufferTooSmall);
        }

        // copy caller data into bounce buffer
        let dst = self.dma_buf_virt.as_mut_ptr::<u8>();
        unsafe { core::ptr::copy_nonoverlapping(buffer.as_ptr(), dst, byte_count) };

        // refer to NVM Command Set Specification, section 3.3.6
        // https://nvmexpress.org/wp-content/uploads/NVM-Express-NVM-Command-Set-Specification-Revision-1.1-2024.08.05-Ratified.pdf
        let mut cmd = NvmeSqe::default();
        cmd.set_opcode(IO_CMD_WRITE);
        cmd.nsid = 1;
        cmd.prp1 = self.dma_buf_phys.as_u64();

        // lba to cdw10 and 11
        cmd.cdw10 = lba as u32;
        cmd.cdw11 = (lba >> 32) as u32;

        cmd.cdw12 = (count - 1) as u32;
        // not setting upper bits of cdw12, as well as cdw13-15, not necessary

        self.io_sq.submit(&cmd);
        let cqe = self.io_cq.wait()?;

        let status = cqe.status_code();
        if status != 0 {
            return Err(NvmeError::CommandError(status));
        }
        Ok(())
    }

    /// Submit an admin command and wait for completion.
    fn _admin_cmd(&mut self, cmd: &NvmeSqe) -> Result<NvmeCqe, NvmeError> {
        self.admin_sq.submit(cmd);
        let cqe = self.admin_cq.wait()?;
        let status = cqe.status_code();
        if status != 0 {
            return Err(NvmeError::CommandError(status));
        }
        Ok(cqe)
    }
}

static NVME_CONTROLLER: Once<Mutex<Option<NvmeController>>> = Once::new();

/// Wait for CSTS.RDY to reach the expected value.
fn wait_ready(regs: *mut NvmeRegisters, expected: bool) -> Result<(), NvmeError> {
    let target = if expected { CSTS_RDY } else { 0 };
    for _ in 0..50_000_000u32 {
        let csts = unsafe { ptr::read_volatile(&(*regs).csts) };
        if (csts & CSTS_RDY) == target {
            return Ok(());
        }
        core::hint::spin_loop();
    }
    Err(NvmeError::Timeout)
}

fn init_inner() -> Option<NvmeController> {
    let (class_code, subclass_code) = PciClass::MassStorage(MassStorageSubclass::NVMe).into();
    let nvme_dev = match pci::find_by_class(class_code, subclass_code).next() {
        Some(dev) => dev,
        None => {
            log::debug!("NVMe: no controller found on PCI bus");
            return None;
        }
    };

    log::debug!(
        "NVMe: found controller at PCI {:02x}:{:02x}.{} ({:04x}:{:04x})",
        nvme_dev.bus,
        nvme_dev.device,
        nvme_dev.function,
        nvme_dev.vendor_id,
        nvme_dev.device_id,
    );

    // enable bus mastering for DMA
    pci::enable_bus_mastering(nvme_dev);

    // map BAR0
    let bar0_phys = pci::read_bar_address(nvme_dev, 0);
    if bar0_phys == 0 {
        log::error!("NVMe: BAR0 is zero");
        return None;
    }
    let regs_virt = memory::translate_addr(bar0_phys as usize);
    let regs = regs_virt.as_mut_ptr::<NvmeRegisters>();

    let cap = unsafe { ptr::read_volatile(&(*regs).cap) };
    let vs = unsafe { ptr::read_volatile(&(*regs).vs) };
    let dstrd = cap_dstrd(cap);
    let mqes = cap_mqes(cap);
    log::debug!(
        "NVMe: version {}.{}.{}, MQES={}, DSTRD={}",
        (vs >> 16) & 0xFF,
        (vs >> 8) & 0xFF,
        vs & 0xFF,
        mqes + 1,
        dstrd
    );

    // 1. disable controller
    let cc = unsafe { ptr::read_volatile(&(*regs).cc) };
    if cc & CC_EN != 0 {
        unsafe { ptr::write_volatile(&mut (*regs).cc, cc & !CC_EN) };
        if wait_ready(regs, false).is_err() {
            log::error!("NVMe: timeout waiting for controller to disable");
            return None;
        }
    }

    // 2. allocate admin queues
    let (admin_sq_virt, admin_sq_phys) = memory::alloc_phys_frame();
    let (admin_cq_virt, admin_cq_phys) = memory::alloc_phys_frame();

    // 3. configure admin queues
    // section 3.3.1.1 in specs
    {
        // AQA: bits 27:16 = ACQS (CQ size - 1), bits 11:0 = ASQS (SQ size - 1)
        // section 3.1.4.8 in specs
        let aqa = (((QUEUE_SIZE - 1) as u32) << 16) | ((QUEUE_SIZE - 1) as u32);
        unsafe {
            ptr::write_volatile(&mut (*regs).aqa, aqa);
            ptr::write_volatile(&mut (*regs).asq, admin_sq_phys.as_u64());
            ptr::write_volatile(&mut (*regs).acq, admin_cq_phys.as_u64());
        }
    }

    // 4. enable controller
    let cc = CC_EN | CC_CSS_NVM | CC_MPS_4K | CC_IOSQES_64 | CC_IOCQES_16;
    unsafe { ptr::write_volatile(&mut (*regs).cc, cc) };
    if wait_ready(regs, true).is_err() {
        log::error!("NVMe: timeout waiting for controller to enable");
        return None;
    }
    log::trace!("NVMe: controller enabled");

    // 5. set up doorbell pointers
    // doorbell stride in bytes = 2 * (2 + 2^DSTRD) = 4 << DSTRD
    // section 3.1.4.1 in specs
    let doorbell_stride = 4 << dstrd;
    // PCIe register for doorbell
    // section 3.1.4 in specs
    let doorbell_base = regs_virt + 0x1000;
    // admin SQ tail doorbell = doorbell_base + 0 * stride
    // admin CQ head doorbell = doorbell_base + 1 * stride
    let admin_sq_doorbell = doorbell_base.as_mut_ptr::<u32>();
    let admin_cq_doorbell = (doorbell_base + doorbell_stride).as_mut_ptr::<u32>();
    let mut admin_sq =
        SubmissionQueue::new(admin_sq_virt, admin_sq_phys, QUEUE_SIZE, admin_sq_doorbell);
    let mut admin_cq =
        CompletionQueue::new(admin_cq_virt, admin_cq_phys, QUEUE_SIZE, admin_cq_doorbell);

    // 6. identify controller
    // section 5.2.13 in specs
    let (identify_virt, identify_phys) = memory::alloc_phys_frame();
    {
        let mut cmd = NvmeSqe::default();
        cmd.set_opcode(ADMIN_IDENTIFY);
        cmd.prp1 = identify_phys.as_u64();
        cmd.cdw10 = 1; // CNS = 1 = identify controller

        admin_sq.submit(&cmd);
        match admin_cq.wait() {
            Ok(cqe) => {
                let status = cqe.status_code();
                if status != 0 {
                    log::error!("NVMe: Identify Controller failed, status=0x{:03x}", status);
                    return None;
                }
                // serial number at bytes 4..24 (SN)
                // model name at bytes 24..63 (MN)
                // section 5.2.13.2.1 in specs
                let data =
                    unsafe { core::slice::from_raw_parts(identify_virt.as_ptr::<u8>(), 4096) };
                let serial = core::str::from_utf8(&data[4..24]).unwrap_or("???");
                let model = core::str::from_utf8(&data[24..64]).unwrap_or("???");
                log::info!("NVMe: model: {}", model.trim());
                log::debug!("NVMe: serial: {}", serial.trim());
            }
            Err(e) => {
                log::error!("NVMe: Identify Controller timeout: {:?}", e);
                return None;
            }
        }
    }

    // 7. create I/O completion queue (QID = 1)
    // section 5.3.1 in specs
    let (io_cq_virt, io_cq_phys) = memory::alloc_phys_frame(); // assuming QUEUE_SIZE fits in 1 4KiB frame
    {
        let mut cmd = NvmeSqe::default();
        cmd.set_opcode(ADMIN_CREATE_IO_CQ);
        cmd.prp1 = io_cq_phys.as_u64();
        // CDW10: bits 31:16 = queue size (0-based), bits 15:0 = QID
        cmd.cdw10 = (((QUEUE_SIZE - 1) as u32) << 16) | 1;
        // CDW11: bit 0 = physically contiguous, bit 1 = interrupts disabled
        cmd.cdw11 = 0b01; // PC=1, IEN=0

        admin_sq.submit(&cmd);
        match admin_cq.wait() {
            Ok(cqe) if cqe.status_code() != 0 => {
                log::error!(
                    "NVMe: Create IO CQ failed, status=0x{:03x}",
                    cqe.status_code()
                );
                return None;
            }
            Err(e) => {
                log::error!("NVMe: Create IO CQ timeout: {:?}", e);
                return None;
            }
            _ => {}
        }
    }
    log::trace!("NVMe: I/O completion queue created");

    // 8. create I/O submission queue (QID = 1, linked to CQ 1)
    // section 5.3.2 in specs
    let (io_sq_virt, io_sq_phys) = memory::alloc_phys_frame();
    {
        let mut cmd = NvmeSqe::default();
        cmd.set_opcode(ADMIN_CREATE_IO_SQ);
        cmd.prp1 = io_sq_phys.as_u64();
        // CDW10: bits 31:16 = queue size (0-based), bits 15:0 = QID
        cmd.cdw10 = (((QUEUE_SIZE - 1) as u32) << 16) | 1;
        // CDW11: bits 31:16 = CQID, bit 0 = physically contiguous
        cmd.cdw11 = (1 << 16) | 1; // CQID=1, PC=1

        admin_sq.submit(&cmd);
        match admin_cq.wait() {
            Ok(cqe) if cqe.status_code() != 0 => {
                log::error!(
                    "NVMe: Create IO SQ failed, status=0x{:03x}",
                    cqe.status_code()
                );
                return None;
            }
            Err(e) => {
                log::error!("NVMe: Create IO SQ timeout: {:?}", e);
                return None;
            }
            _ => {}
        }
    }
    log::trace!("NVMe: I/O submission queue created");

    // I/O queue doorbells: SQ 1 tail = db_base + 2*stride, CQ 1 head = db_base + 3*stride
    let io_sq_doorbell = (doorbell_base + 2 * doorbell_stride).as_mut_ptr::<u32>();
    let io_cq_doorbell = (doorbell_base + 3 * doorbell_stride).as_mut_ptr::<u32>();
    let io_sq = SubmissionQueue::new(io_sq_virt, io_sq_phys, QUEUE_SIZE, io_sq_doorbell);
    let io_cq = CompletionQueue::new(io_cq_virt, io_cq_phys, QUEUE_SIZE, io_cq_doorbell);

    // 9. allocate DMA bounce buffer for data transfers
    let (dma_buf_virt, dma_buf_phys) = memory::alloc_phys_frame();

    Some(NvmeController {
        regs,
        admin_sq,
        admin_cq,
        io_sq,
        io_cq,
        dma_buf_virt,
        dma_buf_phys,
    })
}

/// Initialize the NVMe subsystem. Call after `pci::init()`.
pub fn init() {
    NVME_CONTROLLER.call_once(|| Mutex::new(init_inner()));
}

/// Access the NVMe controller (if available).
pub fn with_controller<F, R>(f: F) -> Result<R, NvmeError>
where
    F: FnOnce(&mut NvmeController) -> Result<R, NvmeError>,
{
    let ctrl = NVME_CONTROLLER.get().ok_or(NvmeError::NoDevice)?;
    let mut lock = ctrl.lock();
    match lock.as_mut() {
        Some(c) => f(c),
        None => Err(NvmeError::NoDevice),
    }
}
