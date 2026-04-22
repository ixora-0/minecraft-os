use core::ptr;

use crate::memory;
use x86_64::{PhysAddr, VirtAddr};

use super::AhciError;
use super::hba::*;

/// Issue a command on slot 0 and poll for completion.
/// `port` must be a valid pointer to an initialized HBA port.
/// `cmd_table_virt` is the virtual address of the command table for slot 0.
unsafe fn issue_command(port: *mut HbaPort) -> Result<(), AhciError> {
    // wait for port not busy
    {
        let mut elapsed = 0u32;
        loop {
            let tfd = unsafe { ptr::read_volatile(&(*port).tfd) };
            if tfd & (HBA_PORT_TFD_BSY | HBA_PORT_TFD_DRQ) == 0 {
                break;
            }
            elapsed += 1;
            if elapsed > 1_000_000 {
                return Err(AhciError::Timeout);
            }
            core::hint::spin_loop();
        }
    }

    // issue command on slot 0
    unsafe { ptr::write_volatile(&mut (*port).ci, 1) };

    // poll for completion
    const TIMEOUT: u32 = 50_000_000; // hopefully >~500ms
    let mut elapsed = 0u32;
    loop {
        let ci = unsafe { ptr::read_volatile(&(*port).ci) };
        if ci & 1 == 0 {
            break;
        }
        let tfd = unsafe { ptr::read_volatile(&(*port).tfd) };
        if tfd & HBA_PORT_TFD_ERR != 0 {
            return Err(AhciError::TaskFileError(tfd));
        }
        core::hint::spin_loop();
        elapsed += 1;
        if elapsed > TIMEOUT {
            return Err(AhciError::Timeout);
        }
    }

    // error check
    let tfd = unsafe { ptr::read_volatile(&(*port).tfd) };
    if tfd & HBA_PORT_TFD_ERR != 0 {
        return Err(AhciError::TaskFileError(tfd));
    }

    Ok(())
}

/// Set up command header for slot 0 with the given command FIS and buffer.
/// We only use slot 0 since we issue one command at a time. AHCI supports up to
/// 32 slots (CAP.NCS) for concurrent command queuing, but we don't need that.
unsafe fn setup_command(
    cmd_list: *mut HbaCommandHeader,
    cmd_table_virt: VirtAddr,
    cfis: &[u8],
    buffer_phys: u64,
    byte_count: u32,
    write: bool,
) {
    let header = unsafe { &mut *cmd_list };

    let cfis_len = (cfis.len() / 4) as u32;

    // fill command header
    let mut flags = cfis_len & 0x1F; // command FIS length in DWORDs
    flags |= 1 << 16; // 1 PRDT entry
    if write {
        flags |= 1 << 6; // Write bit
    }
    header.flags = flags;
    header.prdbc = 0;
    // cmd_table_virt was allocated within the direct-mapped region, so we can safely convert it to a physical address
    let cmd_table_phys = memory::virt_to_phys(cmd_table_virt);
    header.ctba = cmd_table_phys.as_u64() as u32;
    header.ctbau = (cmd_table_phys.as_u64() >> 32) as u32;

    // fill command table
    let table = unsafe { &mut *(cmd_table_virt.as_mut_ptr::<HbaCommandTable>()) };

    // Zero the command table first
    unsafe {
        ptr::write_bytes(table as *mut HbaCommandTable, 0, 1);
    }

    // copy command FIS
    table.cfis[..cfis.len()].copy_from_slice(cfis);

    // set up PRDT entry
    // tells HBA where to put the data
    table.prdt[0].dba = buffer_phys as u32;
    table.prdt[0].dbau = (buffer_phys >> 32) as u32;
    table.prdt[0].dbc = byte_count - 1; // no interrupt on completion
}

/// Build an H2D Register FIS for an ATA command with LBA48 addressing.
fn build_h2d_fis(command: u8, lba: u64, count: u16) -> [u8; 20] {
    let mut fis = FisRegH2D::default();

    fis.fis_type = 0x27;
    fis.pm_and_c = 0x80; // C bit = 1 (command)
    fis.command = command;
    fis.device = 1 << 6; // LBA mode

    fis.lba0 = (lba & 0xFF) as u8;
    fis.lba1 = ((lba >> 8) & 0xFF) as u8;
    fis.lba2 = ((lba >> 16) & 0xFF) as u8;
    fis.lba3 = ((lba >> 24) & 0xFF) as u8;
    fis.lba4 = ((lba >> 32) & 0xFF) as u8;
    fis.lba5 = ((lba >> 40) & 0xFF) as u8;

    fis.countl = (count & 0xFF) as u8;
    fis.counth = ((count >> 8) & 0xFF) as u8;

    // convert to bytes
    let mut buf = [0u8; 20];
    unsafe {
        ptr::copy_nonoverlapping(&fis as *const _ as *const u8, buf.as_mut_ptr(), 20);
    }
    buf
}

/// Read sectors from an AHCI port using READ DMA EXT.
/// DMA writes into the bounce buffer at `dma_buf_phys`; caller reads from `dma_buf_virt`.
///
/// # Safety
/// `port`, `cmd_list`, and `cmd_table_virt` must point to valid, initialized AHCI structures;
/// `dma_buf_phys` must be a valid DMA-capable physical address.
pub unsafe fn read_sectors(
    port: *mut HbaPort,
    cmd_list: *mut HbaCommandHeader,
    cmd_table_virt: VirtAddr,
    lba: u64,
    count: u16,
    _dma_buf_virt: VirtAddr,
    dma_buf_phys: PhysAddr,
) -> Result<(), AhciError> {
    let byte_count = count as u32 * 512;

    let fis = build_h2d_fis(ATA_CMD_READ_DMA_EXT, lba, count);

    unsafe {
        setup_command(
            cmd_list,
            cmd_table_virt,
            &fis,
            dma_buf_phys.as_u64(),
            byte_count,
            false,
        );
        issue_command(port)
    }
}

/// Write sectors to an AHCI port using WRITE DMA EXT.
/// Caller writes data into `dma_buf_virt` before calling; DMA reads from `dma_buf_phys`.
///
/// # Safety
/// `port`, `cmd_list`, and `cmd_table_virt` must point to valid, initialized AHCI structures;
/// `dma_buf_phys` must be a valid DMA-capable physical address.
pub unsafe fn write_sectors(
    port: *mut HbaPort,
    cmd_list: *mut HbaCommandHeader,
    cmd_table_virt: VirtAddr,
    lba: u64,
    count: u16,
    _dma_buf_virt: VirtAddr,
    dma_buf_phys: PhysAddr,
) -> Result<(), AhciError> {
    let byte_count = count as u32 * 512;

    let fis = build_h2d_fis(ATA_CMD_WRITE_DMA_EXT, lba, count);

    unsafe {
        setup_command(
            cmd_list,
            cmd_table_virt,
            &fis,
            dma_buf_phys.as_u64(),
            byte_count,
            true,
        );
        issue_command(port)
    }
}

/// Send IDENTIFY DEVICE command. Result is DMA'd into the bounce buffer.
///
/// # Safety
/// `port`, `cmd_list`, and `cmd_table_virt` point to valid, initialized AHCI structures;
/// `dma_buf_phys` must be a valid DMA-capable physical address.
pub unsafe fn identify(
    port: *mut HbaPort,
    cmd_list: *mut HbaCommandHeader,
    cmd_table_virt: VirtAddr,
    _dma_buf_virt: VirtAddr,
    dma_buf_phys: PhysAddr,
) -> Result<(), AhciError> {
    let fis = build_h2d_fis(ATA_CMD_IDENTIFY, 0, 0);

    unsafe {
        setup_command(
            cmd_list,
            cmd_table_virt,
            &fis,
            dma_buf_phys.as_u64(),
            512,
            false,
        );
        issue_command(port)
    }
}
