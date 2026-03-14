use acpi::AcpiTables;
use acpi::aml::namespace::{AmlName, NameComponent, NameSeg};
use acpi::aml::object::Object;
use acpi::aml::{self, AmlError, Interpreter};
use acpi::platform::AcpiPlatform;
use acpi::{Handle, Handler, PciAddress, PhysicalMapping, sdt::fadt::Fadt};
use alloc::vec;
use core::ptr::NonNull;
use spin::{Mutex, Once};
use x86_64::{VirtAddr, instructions::port::Port};

use crate::memory::PHYS_MEM_OFFSET;

pub static ACPI_TABLES: Once<acpi::AcpiTables<KernelACPI>> = Once::new();
pub static FADT_MAPPING: Once<Mutex<PhysicalMapping<KernelACPI, Fadt>>> = Once::new();
pub static AML_INTERPRETER: Once<Interpreter<KernelACPI>> = Once::new();
const PM1_SLP_EN: u16 = 1 << 13;

fn translate_addr(physical_address: usize) -> VirtAddr {
    *PHYS_MEM_OFFSET.get().expect("Physical memory offset is not yet initialized. Should get this from boot_info passed into _start by the bootloader.") + physical_address as u64
}
#[derive(Copy, Clone)]
pub struct KernelACPI;
impl Handler for KernelACPI {
    unsafe fn map_physical_region<T>(
        &self,
        physical_address: usize,
        size: usize,
    ) -> PhysicalMapping<Self, T> {
        let mapped_length = ((size - 1) / 4096 + 1) * 4096;
        PhysicalMapping {
            physical_start: physical_address,
            virtual_start: NonNull::new(translate_addr(physical_address).as_mut_ptr())
                .expect("Failed to get virtual address"),
            region_length: size,
            mapped_length,
            handler: self.clone(),
        }
    }

    fn unmap_physical_region<T>(_region: &PhysicalMapping<Self, T>) {}

    // --- Physical memory read/write ---
    fn read_u8(&self, address: usize) -> u8 {
        unsafe { *((translate_addr(address)).as_ptr::<u8>()) }
    }
    fn read_u16(&self, address: usize) -> u16 {
        unsafe { *((translate_addr(address)).as_ptr::<u16>()) }
    }
    fn read_u32(&self, address: usize) -> u32 {
        unsafe { *((translate_addr(address)).as_ptr::<u32>()) }
    }
    fn read_u64(&self, address: usize) -> u64 {
        unsafe { *((translate_addr(address)).as_ptr::<u64>()) }
    }
    fn write_u8(&self, address: usize, value: u8) {
        unsafe { *((translate_addr(address)).as_mut_ptr::<u8>()) = value }
    }
    fn write_u16(&self, address: usize, value: u16) {
        unsafe { *((translate_addr(address)).as_mut_ptr::<u16>()) = value }
    }
    fn write_u32(&self, address: usize, value: u32) {
        unsafe { *((translate_addr(address)).as_mut_ptr::<u32>()) = value }
    }
    fn write_u64(&self, address: usize, value: u64) {
        unsafe { *((translate_addr(address)).as_mut_ptr::<u64>()) = value }
    }

    // --- I/O ports ---
    fn read_io_u8(&self, port: u16) -> u8 {
        unsafe { Port::<u8>::new(port).read() }
    }
    fn read_io_u16(&self, port: u16) -> u16 {
        unsafe { Port::<u16>::new(port).read() }
    }
    fn read_io_u32(&self, port: u16) -> u32 {
        unsafe { Port::<u32>::new(port).read() }
    }
    fn write_io_u8(&self, port: u16, value: u8) {
        unsafe { Port::<u8>::new(port).write(value) }
    }
    fn write_io_u16(&self, port: u16, value: u16) {
        unsafe { Port::<u16>::new(port).write(value) }
    }
    fn write_io_u32(&self, port: u16, value: u32) {
        unsafe { Port::<u32>::new(port).write(value) }
    }

    // --- PCI (stubbed) ---
    fn read_pci_u8(&self, _: PciAddress, _: u16) -> u8 {
        0
    }
    fn read_pci_u16(&self, _: PciAddress, _: u16) -> u16 {
        0
    }
    fn read_pci_u32(&self, _: PciAddress, _: u16) -> u32 {
        0
    }
    fn write_pci_u8(&self, _: PciAddress, _: u16, _: u8) {}
    fn write_pci_u16(&self, _: PciAddress, _: u16, _: u16) {}
    fn write_pci_u32(&self, _: PciAddress, _: u16, _: u32) {}

    // --- Timing (stubbed for now) ---
    fn nanos_since_boot(&self) -> u64 {
        0
    }
    fn stall(&self, _microseconds: u64) {}
    fn sleep(&self, _milliseconds: u64) {}

    fn create_mutex(&self) -> Handle {
        Handle(0)
    }
    fn acquire(&self, _mutex: Handle, _timeout: u16) -> Result<(), AmlError> {
        Ok(())
    }
    fn release(&self, _mutex: Handle) {}
}

pub fn init(rsdp_addr: u64) {
    let handler = KernelACPI;
    let rsdp_addr = rsdp_addr as usize;

    // log::trace!("RSDP address: 0x{:x}", rsdp_addr);
    let tables = ACPI_TABLES.call_once(|| unsafe {
        acpi::AcpiTables::from_rsdp(handler, rsdp_addr)
            .expect("unable to create ACPI tables from RSDP address.")
    });

    // log::trace!("ACPI tables found, revision: {}", tables.rsdp_revision);

    let _fadt = FADT_MAPPING
        .call_once(|| Mutex::new(tables.find_table::<Fadt>().expect("Failed to find FADT")));

    // have to create new AcpiTables because it doesn't have clone
    let tables_for_aml = unsafe {
        match AcpiTables::from_rsdp(handler.clone(), rsdp_addr) {
            Ok(t) => t,
            Err(e) => {
                log::error!("Failed to create AcpiTables for AML: {:?}", e);
                return;
            }
        }
    };
    let platform = match AcpiPlatform::new(tables_for_aml, handler) {
        Ok(p) => p,
        Err(e) => {
            log::error!("Failed to create AcpiPlatform: {:?}", e);
            return;
        }
    };
    let interpreter = match aml::Interpreter::new_from_platform(&platform) {
        Ok(i) => i,
        Err(e) => {
            log::error!("Failed to create AML interpreter: {:?}", e);
            return;
        }
    };
    AML_INTERPRETER.call_once(|| interpreter);
}

pub fn shutdown() {
    log::info!("Initiating ACPI shutdown...");

    let fadt_mapping = FADT_MAPPING
        .get()
        .expect("FADT not initialized, cannot shutdown")
        .lock();
    let fadt = fadt_mapping.get();

    let pm1a_addr = match fadt.pm1a_control_block() {
        Ok(ga) => ga.address as u16,
        Err(e) => {
            log::error!("Failed to get PM1a control block: {:?}", e);
            return;
        }
    };
    log::trace!("PM1a control block: 0x{:x}", pm1a_addr);

    let pm1b_addr = match fadt.pm1b_control_block() {
        Ok(Some(ga)) => Some(ga.address as u16),
        Ok(None) => None,
        Err(e) => {
            log::warn!("failed to get PM1b control block: {:?}", e);
            return;
        }
    };
    log::trace!("PM1b control block: {:?}", pm1b_addr);

    // Get SLP_TYP from AML \_S5_, fall back to 0 (works on QEMU) if unavailable
    let slp_typ: u16 = {
        let aml_interpreter = AML_INTERPRETER.get();
        match aml_interpreter.as_ref() {
            Some(interp) => match get_slp_typ_s5(interp) {
                Ok(v) => {
                    log::trace!("SLP_TYP from \\_S5_: {}", v);
                    v
                }
                Err(e) => {
                    log::warn!("\\_S5_ failed ({:?}), falling back to 0", e);
                    0
                }
            },
            None => {
                log::warn!("AML not initialized, falling back to 0");
                0
            }
        }
    };

    let slp_cmd = PM1_SLP_EN | (slp_typ << 10);
    log::trace!("Sending shutdown command: 0x{:x}", slp_cmd);

    unsafe {
        Port::<u16>::new(pm1a_addr).write(slp_cmd);
        if let Some(pm1b) = pm1b_addr {
            Port::<u16>::new(pm1b).write(slp_cmd);
        }
    }

    log::info!(
        "ACPI shutdown command sent. If system doesn't power off, please power off manually."
    );
    crate::hlt_loop();
}

fn get_slp_typ_s5(aml_interpreter: &aml::Interpreter<KernelACPI>) -> Result<u16, AmlError> {
    let s5_name = AmlName::from_components(vec![
        NameComponent::Root,
        NameComponent::Segment(NameSeg::from_bytes([b'_', b'S', b'5', b'_'])?),
    ]);
    let s5 = aml_interpreter.evaluate(s5_name, vec![])?;
    match &*s5 {
        Object::Package(elements) => {
            // \_S5_ package: [SLP_TYP_a, SLP_TYP_b, ...]
            let slp_typ_a = elements.get(0).ok_or(AmlError::MethodArgCountIncorrect)?;
            let val = slp_typ_a.as_integer()?;
            Ok(val as u16)
        }
        _ => Err(AmlError::MethodArgCountIncorrect),
    }
}
