use alloc::vec::Vec;
use spin::Once;
use x86_64::instructions::port::Port;

const CONFIG_ADDRESS: u16 = 0xCF8;
const CONFIG_DATA: u16 = 0xCFC;

// https://wiki.osdev.org/PCI#Class_Codes
#[non_exhaustive]
pub enum PciClass {
    Unclassified(UnclassifiedSubclass),
    MassStorage(MassStorageSubclass),
    // ...
}
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
#[non_exhaustive]
pub enum UnclassifiedSubclass {
    NonVGA = 0x00,
    VGACompatible = 0x01,
}
#[repr(u8)]
pub enum MassStorageSubclass {
    SCSIBus,
    IDE,
    Floppy,
    IPIBus,
    RAID,
    ATA,
    SATA,
    SerialAttachedSCSI,
    NVMe,
    Other,
}
impl From<PciClass> for (u8, u8) {
    fn from(class: PciClass) -> Self {
        match class {
            PciClass::Unclassified(sub) => (0x00, sub as u8),
            PciClass::MassStorage(sub) => (0x01, sub as u8),
            // _ => unimplemented!(),
        }
    }
}

/// A discovered PCI device.
#[derive(Debug, Clone)]
pub struct PciDevice {
    pub bus: u8,
    pub device: u8,
    pub function: u8,

    pub vendor_id: u16,
    pub device_id: u16,

    pub class: u8,
    pub subclass: u8,
    pub prog_if: u8,

    pub header_type: u8,

    /// Base Address Registers
    /// (where the device is mapped in memory/io)
    pub bars: [u32; 6],

    pub interrupt_line: u8,
    pub interrupt_pin: u8,
}

static PCI_DEVICES: Once<Vec<PciDevice>> = Once::new();

/// Builds 32-bit PCI config space address that uses mechanism #1 for the given bus, device, function, and offset.
fn config_address(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    // https://wiki.osdev.org/PCI#Configuration_Space_Access_Mechanism_#1
    // X      | XXXXXXX  | XXXXXXXX | XXXXX  | XXX      | XXXXXXXX
    // 31     | 30-24    | 23-16    | 15-11  | 10-8     | 7-0
    // enable | reserved | bus      | device | function | offset
    (1u32 << 31)
        | ((bus as u32) << 16)
        | ((device as u32) << 11)
        | ((function as u32) << 8)
        | ((offset as u32) & 0xFC) // align to 4-byte boundary
}

pub fn pci_read_u32(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    let addr = config_address(bus, device, function, offset);
    unsafe {
        Port::<u32>::new(CONFIG_ADDRESS).write(addr);
        Port::<u32>::new(CONFIG_DATA).read()
    }
}

pub fn pci_read_u16(bus: u8, device: u8, function: u8, offset: u8) -> u16 {
    let val = pci_read_u32(bus, device, function, offset & 0xFC);
    ((val >> ((offset & 2) * 8)) & 0xFFFF) as u16
}

pub fn pci_read_u8(bus: u8, device: u8, function: u8, offset: u8) -> u8 {
    let val = pci_read_u32(bus, device, function, offset & 0xFC);
    ((val >> ((offset & 3) * 8)) & 0xFF) as u8
}

pub fn pci_write_u32(bus: u8, device: u8, function: u8, offset: u8, value: u32) {
    let addr = config_address(bus, device, function, offset);
    unsafe {
        Port::<u32>::new(CONFIG_ADDRESS).write(addr);
        Port::<u32>::new(CONFIG_DATA).write(value);
    }
}

pub fn pci_write_u16(bus: u8, device: u8, function: u8, offset: u8, value: u16) {
    let current = pci_read_u32(bus, device, function, offset & 0xFC);
    let shift = (offset & 2) * 8;
    let mask = !(0xFFFF << shift);
    let new = (current & mask) | ((value as u32) << shift);
    pci_write_u32(bus, device, function, offset & 0xFC, new);
}

pub fn pci_write_u8(bus: u8, device: u8, function: u8, offset: u8, value: u8) {
    let current = pci_read_u32(bus, device, function, offset & 0xFC);
    let shift = (offset & 3) * 8;
    let mask = !(0xFF << shift);
    let new = (current & mask) | ((value as u32) << shift);
    pci_write_u32(bus, device, function, offset & 0xFC, new);
}

fn read_device(bus: u8, device: u8, function: u8) -> Option<PciDevice> {
    let vendor_id = pci_read_u16(bus, device, function, 0x00);
    if vendor_id == 0xFFFF {
        return None;
    }

    // https://wiki.osdev.org/PCI#Common_Header_Fields
    let device_id = pci_read_u16(bus, device, function, 0x02);
    let class = pci_read_u8(bus, device, function, 0x0B);
    let subclass = pci_read_u8(bus, device, function, 0x0A);
    let prog_if = pci_read_u8(bus, device, function, 0x09);
    let header_type = pci_read_u8(bus, device, function, 0x0E);
    let interrupt_line = pci_read_u8(bus, device, function, 0x3C);
    let interrupt_pin = pci_read_u8(bus, device, function, 0x3D);

    let mut bars = [0u32; 6];

    // Only type 0 headers have 6 BARs; type 1 (bridge) has 2.
    // https://wiki.osdev.org/PCI#Header_Type_0x0
    // https://wiki.osdev.org/PCI#Header_Type_0x1_(PCI-to-PCI_bridge)
    let bar_count = if header_type & 0x7F == 0 { 6 } else { 2 };
    for (i, bar) in bars[..bar_count].iter_mut().enumerate() {
        *bar = pci_read_u32(bus, device, function, 0x10 + (i as u8) * 4);
    }
    Some(PciDevice {
        bus,
        device,
        function,
        vendor_id,
        device_id,
        class,
        subclass,
        prog_if,
        header_type,
        bars,
        interrupt_line,
        interrupt_pin,
    })
}

/// Enumerate all PCI devices using brute force scan.
fn scan_all() -> Vec<PciDevice> {
    let mut devices = Vec::new();

    for bus in 0..=255u16 {
        for device in 0..32u8 {
            // check exists
            let Some(dev) = read_device(bus as u8, device, 0) else {
                continue;
            };

            // https://wiki.osdev.org/PCI#Header_Type_Register
            let is_multifunction = dev.header_type & 0x80 != 0;

            devices.push(dev);
            if is_multifunction {
                for function in 1..8u8 {
                    if let Some(dev) = read_device(bus as u8, device, function) {
                        devices.push(dev);
                    }
                }
            }
        }
    }

    devices
}

// --- Public API ---

/// Scan PCI buses and store discovered devices.
pub fn init() {
    let devices = PCI_DEVICES.call_once(scan_all);
    for dev in devices {
        log::debug!(
            "PCI {:02x}:{:02x}.{} - {:04x}:{:04x} class {:02x}:{:02x} prog_if {:02x}",
            dev.bus,
            dev.device,
            dev.function,
            dev.vendor_id,
            dev.device_id,
            dev.class,
            dev.subclass,
            dev.prog_if,
        );
    }
}

/// Get all discovered PCI devices.
pub fn devices() -> &'static [PciDevice] {
    PCI_DEVICES.get().map(|v| v.as_slice()).unwrap_or(&[])
}

/// Find PCI devices by class and subclass.
pub fn find_by_class(class: u8, subclass: u8) -> impl Iterator<Item = &'static PciDevice> {
    devices()
        .iter()
        .filter(move |d| d.class == class && d.subclass == subclass)
}

/// Read a BAR address.
/// Returns the base address with type/prefetchable bits masked off.
pub fn read_bar_address(dev: &PciDevice, bar_index: usize) -> u64 {
    // https://wiki.osdev.org/PCI#Base_Address_Registers
    if bar_index >= 6 {
        return 0;
    }
    let bar = dev.bars[bar_index];

    if bar & 1 != 0 {
        // I/O BAR — lower 2 bits are type
        return (bar & !0x3) as u64;
    }

    // memory BAR — check type (bits 2:1)
    let bar_type = (bar >> 1) & 0x3;
    let base = (bar & !0xF) as u64;

    if bar_type == 2 && bar_index < 5 {
        // 64-bit BAR: next BAR is upper 32 bits
        let upper = dev.bars[bar_index + 1] as u64;
        base | (upper << 32)
    } else {
        base
    }
}

/// Enable bus mastering (set bit 2 of PCI command register).
pub fn enable_bus_mastering(dev: &PciDevice) {
    let cmd = pci_read_u16(dev.bus, dev.device, dev.function, 0x04);
    if cmd & (1 << 2) == 0 {
        pci_write_u16(dev.bus, dev.device, dev.function, 0x04, cmd | (1 << 2));
    }
}
