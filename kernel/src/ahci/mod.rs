// See AHCI specification 1.3.1 for details
// https://www.intel.com/content/dam/www/public/us/en/documents/technical-specifications/serial-ata-ahci-spec-rev1-3-1.pdf

pub mod hba;
pub mod port;

use core::ptr;
use spin::{Mutex, Once};
use x86_64::{PhysAddr, VirtAddr};

use crate::memory::{self, translate_addr};
use crate::pci::{self, MassStorageSubclass, PciClass};
use hba::*;

#[derive(Debug)]
pub enum AhciError {
    Timeout,
    TaskFileError(u32),
    BufferTooSmall,
    NoDevice,
}

/// State for a single AHCI port that has a connected device.
pub struct AhciPort {
    port_ptr: *mut HbaPort,
    cmd_list: *mut HbaCommandHeader,
    cmd_table_virt: VirtAddr,
    /// DMA bounce buffer in the direct-mapped region (one 4 KiB frame).
    dma_buf_virt: VirtAddr,
    dma_buf_phys: PhysAddr,
}

// SAFETY: We access these through a Mutex and the kernel is single-threaded.
unsafe impl Send for AhciPort {}
unsafe impl Sync for AhciPort {}

impl AhciPort {
    pub fn read_sectors(
        &mut self,
        lba: u64,
        count: u16,
        buffer: &mut [u8],
    ) -> Result<(), AhciError> {
        let byte_count = count as usize * 512;
        if buffer.len() < byte_count {
            return Err(AhciError::BufferTooSmall);
        }
        if byte_count > 4096 {
            return Err(AhciError::BufferTooSmall);
        }

        unsafe {
            port::read_sectors(
                self.port_ptr,
                self.cmd_list,
                self.cmd_table_virt,
                lba,
                count,
                self.dma_buf_virt,
                self.dma_buf_phys,
            )?;
        }

        // Copy from bounce buffer to caller's buffer
        let src = self.dma_buf_virt.as_ptr::<u8>();
        unsafe { core::ptr::copy_nonoverlapping(src, buffer.as_mut_ptr(), byte_count) };
        Ok(())
    }

    pub fn write_sectors(&mut self, lba: u64, count: u16, buffer: &[u8]) -> Result<(), AhciError> {
        let byte_count = count as usize * 512;
        if buffer.len() < byte_count {
            return Err(AhciError::BufferTooSmall);
        }
        if byte_count > 4096 {
            return Err(AhciError::BufferTooSmall);
        }

        // Copy caller's buffer into bounce buffer
        let dst = self.dma_buf_virt.as_mut_ptr::<u8>();
        unsafe { core::ptr::copy_nonoverlapping(buffer.as_ptr(), dst, byte_count) };

        unsafe {
            port::write_sectors(
                self.port_ptr,
                self.cmd_list,
                self.cmd_table_virt,
                lba,
                count,
                self.dma_buf_virt,
                self.dma_buf_phys,
            )
        }
    }
}

pub struct AhciController {
    /// The first usable port with a connected SATA device.
    pub port: AhciPort,
}

static AHCI_CONTROLLER: Once<Mutex<Option<AhciController>>> = Once::new();

/// Wait for the CR bit to clear in the port's command register.
fn wait_cr(port: *mut HbaPort) {
    // hopefully >500ms
    for _ in 0..1_000_000 {
        let cmd = unsafe { ptr::read_volatile(&(*port).cmd) };
        if cmd & HBA_PORT_CMD_CR == 0 {
            break;
        }
        core::hint::spin_loop();
    }
}

/// Stop the command engine for a port.
unsafe fn stop_port(port: *mut HbaPort) {
    let cmd = unsafe { ptr::read_volatile(&(*port).cmd) };
    if cmd & (HBA_PORT_CMD_ST | HBA_PORT_CMD_FRE) == 0 {
        return;
    }

    // section 10.1.2 in specs, step 3
    // ST must be cleared first, and the port must finish processing the current command slot before FRE can be
    // safely cleared.
    unsafe { ptr::write_volatile(&mut (*port).cmd, cmd & !HBA_PORT_CMD_ST) };
    wait_cr(port);
    let cmd = unsafe { ptr::read_volatile(&(*port).cmd) };
    unsafe { ptr::write_volatile(&mut (*port).cmd, cmd & !HBA_PORT_CMD_FRE) };
    wait_cr(port);
}

/// Start the command engine for a port.
unsafe fn start_port(port: *mut HbaPort) {
    wait_cr(port);
    let cmd = unsafe { ptr::read_volatile(&(*port).cmd) };
    unsafe { ptr::write_volatile(&mut (*port).cmd, cmd | HBA_PORT_CMD_FRE | HBA_PORT_CMD_ST) };
}

/// Initialize AHCI, find controller via PCI, set up the first available port.
/// Returns `None` if no AHCI controller is found, if ABAR is not available, or if no devices are connected to the AHCI controller.
fn init_inner() -> Option<AhciController> {
    // find ACHI controller
    // https://wiki.osdev.org/AHCI#Find_an_AHCI_controller
    let (class_code, subclass_code) = PciClass::MassStorage(MassStorageSubclass::SATA).into();
    let ahci_dev = match pci::find_by_class(class_code, subclass_code).next() {
        Some(dev) => dev,
        None => {
            log::warn!("AHCI: No AHCI controller found on PCI bus");
            return None;
        }
    };

    log::debug!(
        "AHCI: Found controller at PCI {:02x}:{:02x}.{} ({:04x}:{:04x})",
        ahci_dev.bus,
        ahci_dev.device,
        ahci_dev.function,
        ahci_dev.vendor_id,
        ahci_dev.device_id,
    );

    // enable bus mastering for DMA
    pci::enable_bus_mastering(ahci_dev);

    // read ABAR (BAR5)
    // https://wiki.osdev.org/AHCI#AHCI_Registers_and_Memory_Structures
    let abar_phys = pci::read_bar_address(ahci_dev, 5) as usize;
    if abar_phys == 0 {
        log::error!("AHCI: ABAR is zero");
        return None;
    }

    let hba_virt = translate_addr(abar_phys);
    let hba = hba_virt.as_mut_ptr::<HbaMemory>();

    // enable AHCI mode
    // section 10.1.2 in specs, step 1
    unsafe {
        let ghc = ptr::read_volatile(&(*hba).ghc);
        ptr::write_volatile(&mut (*hba).ghc, ghc | hba::HBA_GHC_AE);
    }

    let pi = unsafe { ptr::read_volatile(&(*hba).pi) };
    let vs = unsafe { ptr::read_volatile(&(*hba).vs) };
    log::debug!(
        "AHCI: version {}.{}, ports implemented: 0b{:032b}",
        (vs >> 16) & 0xFFFF,
        vs & 0xFFFF,
        pi,
    );

    // find first port with a connected device
    for i in 0..32u8 {
        if pi & (1 << i) == 0 {
            continue;
        }

        let port_ptr = unsafe { &mut (*hba).ports[i as usize] as *mut HbaPort };

        // section 3.3.10 in specs
        let ssts = unsafe { ptr::read_volatile(&(*port_ptr).ssts) };
        let det = ssts & 0xF; // device detection, 0x00 - 0x03
        if det != 3 {
            continue; // no device present / PHY communication not established
        }

        // section 3.3.9 in specs
        let sig = unsafe { ptr::read_volatile(&(*port_ptr).sig) };
        log::debug!("AHCI: Port {} - device detected (sig: 0x{:08x})", i, sig);
        if sig != SATA_SIG_ATA {
            log::debug!("AHCI: Port {} - not SATA disk, skipping", i);
            continue;
        }

        // initialize this port
        unsafe { stop_port(port_ptr) };

        // section 10.1.2 in specs, step 5
        // allocate one physical frame for AHCI control structures.
        // layout within the 4 KiB frame:
        //   offset 0x000: command list   (1024 bytes, 1024-aligned)
        //   offset 0x400: FIS receive    (256 bytes, 256-aligned)
        //   offset 0x500: command table  (144 bytes, 128-aligned; 0x500 = 1280 = 10*128)
        let (ctrl_virt, ctrl_phys) = memory::alloc_phys_frame();
        let cmd_list_virt = ctrl_virt;
        let cmd_list_phys = ctrl_phys;
        let fis_phys = PhysAddr::new(ctrl_phys.as_u64() + 0x400);
        let cmd_table_virt = ctrl_virt + 0x500u64;
        // set CLB and FB
        unsafe {
            ptr::write_volatile(&mut (*port_ptr).clb, cmd_list_phys.as_u64() as u32);
            ptr::write_volatile(&mut (*port_ptr).clbu, (cmd_list_phys.as_u64() >> 32) as u32);
            ptr::write_volatile(&mut (*port_ptr).fb, fis_phys.as_u64() as u32);
            ptr::write_volatile(&mut (*port_ptr).fbu, (fis_phys.as_u64() >> 32) as u32);
        }

        // clear SERR
        // section 10.1.2 in specs, step 6
        unsafe { ptr::write_volatile(&mut (*port_ptr).serr, 0xFFFFFFFF) };

        // start the port
        unsafe { start_port(port_ptr) };

        // allocate a DMA bounce buffer (one 4 KiB frame) for data transfers
        let (dma_buf_virt, dma_buf_phys) = memory::alloc_phys_frame();

        // try IDENTIFY to confirm the drive works
        let cmd_list = cmd_list_virt.as_mut_ptr::<HbaCommandHeader>();
        match unsafe {
            port::identify(
                port_ptr,
                cmd_list,
                cmd_table_virt,
                dma_buf_virt,
                dma_buf_phys,
            )
        } {
            Ok(()) => {
                // Extract model string from IDENTIFY (words 27-46, byte-swapped)
                let identify_buf: &[u8; 512] = unsafe { &*(dma_buf_virt.as_ptr::<[u8; 512]>()) };
                let model = extract_ata_string(identify_buf, 27, 46);
                log::info!("AHCI: Port {} - drive model: {}", i, model.trim());
            }
            Err(e) => {
                log::error!("AHCI: Port {} - IDENTIFY failed: {:?}", i, e);
                continue;
            }
        }

        return Some(AhciController {
            port: AhciPort {
                port_ptr,
                cmd_list,
                cmd_table_virt,
                dma_buf_virt,
                dma_buf_phys,
            },
        });
    }

    log::warn!("AHCI: No usable SATA devices found");
    None
}

/// Extract an ATA string from IDENTIFY data (words are byte-swapped).
fn extract_ata_string(
    identify: &[u8; 512],
    word_start: usize,
    word_end: usize,
) -> alloc::string::String {
    let mut s = alloc::string::String::new();
    for word in word_start..=word_end {
        let offset = word * 2;
        if offset + 1 < 512 {
            // ATA strings are byte-swapped within each word
            s.push(identify[offset + 1] as char);
            s.push(identify[offset] as char);
        }
    }
    s
}

/// Initialize the AHCI subsystem. Call after `pci::init()`.
pub fn init() {
    AHCI_CONTROLLER.call_once(|| Mutex::new(init_inner()));
}

/// Access the AHCI controller's first port (if available).
pub fn with_port<F, R>(f: F) -> Result<R, AhciError>
where
    F: FnOnce(&mut AhciPort) -> Result<R, AhciError>,
{
    let controller = AHCI_CONTROLLER.get().ok_or(AhciError::NoDevice)?;
    let mut lock = controller.lock();
    match lock.as_mut() {
        Some(ctrl) => f(&mut ctrl.port),
        None => Err(AhciError::NoDevice),
    }
}
