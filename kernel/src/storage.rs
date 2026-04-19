use crate::ahci;
use crate::nvme;

#[derive(Debug)]
pub enum StorageError {
    Ahci(ahci::AhciError),
    Nvme(nvme::NvmeError),
    NoDevice,
}

enum Backend {
    Nvme,
    Ahci,
}

static BACKEND: spin::Once<Option<Backend>> = spin::Once::new();

/// Initialize storage. Call after `nvme::init()` and `ahci::init()`.
pub fn init() {
    BACKEND.call_once(|| {
        // prefer NVMe
        if nvme::with_controller(|_| Ok(())).is_ok() {
            log::info!("Storage: using NVMe backend");
            return Some(Backend::Nvme);
        }
        // fall back to AHCI
        if ahci::with_port(|_| Ok(())).is_ok() {
            log::info!("Storage: using AHCI backend");
            return Some(Backend::Ahci);
        }
        log::warn!("Storage: no storage device available");
        None
    });
}

pub fn read_sectors(lba: u64, count: u16, buffer: &mut [u8]) -> Result<(), StorageError> {
    match BACKEND.get() {
        Some(Some(Backend::Nvme)) => nvme::with_controller(|c| c.read_sectors(lba, count, buffer))
            .map_err(StorageError::Nvme),
        Some(Some(Backend::Ahci)) => {
            ahci::with_port(|p| p.read_sectors(lba, count, buffer)).map_err(StorageError::Ahci)
        }
        _ => Err(StorageError::NoDevice),
    }
}

pub fn write_sectors(lba: u64, count: u16, buffer: &[u8]) -> Result<(), StorageError> {
    match BACKEND.get() {
        Some(Some(Backend::Nvme)) => nvme::with_controller(|c| c.write_sectors(lba, count, buffer))
            .map_err(StorageError::Nvme),
        Some(Some(Backend::Ahci)) => {
            ahci::with_port(|p| p.write_sectors(lba, count, buffer)).map_err(StorageError::Ahci)
        }
        _ => Err(StorageError::NoDevice),
    }
}
