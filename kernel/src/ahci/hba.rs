//! AHCI Host Bus Adapter register structures.
//!
//! See AHCI specification 1.3.1 for register layouts (section 3):
//! https://www.intel.com/content/dam/www/public/us/en/documents/technical-specifications/serial-ata-ahci-spec-rev1-3-1.pdf

// section 3.1 in specs
/// HBA Memory Registers (Generic Host Control)
#[repr(C)]
pub struct HbaMemory {
    /// Host Capabilities.
    /// 0x00 - 0x03
    pub cap: u32,
    /// Global Host Control.
    /// 0x04 - 0x07
    pub ghc: u32,
    /// Interrupt Status.
    /// 0x08 - 0x0B
    pub is: u32,
    /// Ports Implemented.
    /// 0x0C - 0x0F
    pub pi: u32,
    /// Version.
    /// 0x10 - 0x13
    pub vs: u32,
    /// Command Completion Coalescing Control.
    /// 0x14 - 0x17
    pub ccc_ctl: u32,
    /// Command Completion Coalescing Ports.
    /// 0x18 - 0x1B
    pub ccc_ports: u32,
    /// Enclosure Management Location.
    /// 0x1C - 0x1F
    pub em_loc: u32,
    /// Enclosure Management Control.
    /// 0x20 - 0x23
    pub em_ctl: u32,
    /// Host Capabilities Extended.
    /// 0x24 - 0x27
    pub cap2: u32,
    /// BIOS/OS Handoff Control and Status.
    /// 0x28 - 0x2B
    pub bohc: u32,
    /// Reserved.
    /// 0x2C - 0x9F
    _reserved: [u8; 0xA0 - 0x2C],
    /// Vendor Specific.
    /// 0xA0 - 0xFF
    _vendor: [u8; 0x100 - 0xA0],
    /// Port control registers (up to 32 ports).
    pub ports: [HbaPort; 32],
}

// section 3.3 in specs
/// Per-port registers
#[repr(C)]
pub struct HbaPort {
    /// Command List Base Address (lower 32 bits, 1024-byte aligned).
    /// 0x00 - 0x03
    pub clb: u32,
    /// Command List Base Address Upper 32 bits.
    /// 0x04 - 0x07
    pub clbu: u32,
    /// FIS Base Address (lower 32 bits, 256-byte aligned).
    /// 0x08 - 0x0B
    pub fb: u32,
    /// FIS Base Address Upper 32 bits.
    /// 0x0C - 0x0F
    pub fbu: u32,
    /// Interrupt Status.
    /// 0x10 - 0x13
    pub is: u32,
    /// Interrupt Enable.
    /// 0x14 - 0x17
    pub ie: u32,
    /// Command and Status.
    /// 0x18 - 0x1B
    pub cmd: u32,
    /// Reserved.
    /// 0x1C - 0x1F
    _reserved0: u32,
    /// Task File Data.
    /// 0x20 - 0x23
    pub tfd: u32,
    /// Signature.
    /// 0x24 - 0x27
    pub sig: u32,
    /// Serial ATA Status (SCR0: SStatus).
    /// 0x28 - 0x2B
    pub ssts: u32,
    /// Serial ATA Control (SCR2: SControl).
    /// 0x2C - 0x2F
    pub sctl: u32,
    /// Serial ATA Error (SCR1: SError).
    /// 0x30 - 0x33
    pub serr: u32,
    /// Serial ATA Active (SCR3: SActive).
    /// 0x34 - 0x37
    pub sact: u32,
    /// Command Issue.
    /// 0x38 - 0x3B
    pub ci: u32,
    /// Serial ATA Notification (SCR4: SNotification).
    /// 0x3C - 0x3F
    pub sntf: u32,
    /// FIS-based Switching Control.
    /// 0x40 - 0x43
    pub fbs: u32,
    /// Device Sleep.
    /// 0x44 - 0x47
    pub devslp: u32,
    /// Reserved.
    /// 0x48 - 0x6F
    _reserved1: [u8; 0x70 - 0x48],
    /// Vendor Specific.
    /// 0x70 - 0x7F
    _vendor: [u8; 0x80 - 0x70],
}

/// Command Header (one of 32 entries in the command list)
#[repr(C)]
pub struct HbaCommandHeader {
    /// DW0: command FIS length (bits 0-4), ATAPI (5), Write (6), Prefetchable (7),
    /// Reset (8), BIST (9), Clear Busy upon R_OK (10), reserved (11),
    /// Port Multiplier Port (12-15), PRDT Length (16-31)
    pub flags: u32,
    /// Physical Region Descriptor Byte Count
    pub prdbc: u32,
    /// Command Table Descriptor Base Address (128-byte aligned)
    pub ctba: u32,
    /// Command Table Descriptor Base Address Upper 32 bits
    pub ctbau: u32,
    _reserved: [u32; 4],
}

// https://sata-io.org/system/files/specifications/SerialATA_Revision_3_5_Gold.pdf
// section 10.5.5
/// FIS - Host to Device Register (type 0x27)
#[repr(C)]
#[derive(Default)]
pub struct FisRegH2D {
    /// FIS type (should be 0x27)
    pub fis_type: u8,
    /// Port multiplier (bits 0-3), reserved (4-6), C bit (7) = 1 for command
    pub pm_and_c: u8,
    /// ATA command register
    pub command: u8,
    /// Features (lower 8 bits)
    pub featurel: u8,

    /// LBA low
    pub lba0: u8,
    /// LBA mid
    pub lba1: u8,
    /// LBA high
    pub lba2: u8,
    /// Device register
    pub device: u8,

    /// LBA low (expanded)
    pub lba3: u8,
    /// LBA mid (expanded)
    pub lba4: u8,
    /// LBA high (expanded)
    pub lba5: u8,
    /// Features (upper 8 bits)
    pub features: u8,

    /// Sector count (lower)
    pub countl: u8,
    /// Sector count (upper)
    pub counth: u8,
    /// ISO command completion
    pub icc: u8,
    /// Control
    pub control: u8,

    _auxiliary: [u8; 4],
}

/// Physical Region Descriptor Table entry
#[repr(C)]
pub struct HbaPrdt {
    /// Data Base Address (2-byte aligned)
    pub dba: u32,
    /// Data Base Address Upper 32 bits
    pub dbau: u32,
    _reserved: u32,
    /// Byte count (bit 0 must be 1 = even byte count; bits 1-21 = byte count - 1;
    /// bit 31 = interrupt on completion)
    pub dbc: u32,
}

/// Command Table (pointed to by command header)
/// This is the minimum layout with 1 PRDT entry.
/// Must be 128-byte aligned.
#[repr(C)]
pub struct HbaCommandTable {
    /// Command FIS (up to 64 bytes)
    pub cfis: [u8; 64],
    /// ATAPI Command (16 bytes)
    pub acmd: [u8; 16],
    _reserved: [u8; 48],
    /// Physical Region Descriptor Table (variable length, we use 1 entry)
    pub prdt: [HbaPrdt; 1],
}

// HBA port CMD register bits
// section 3.3.7 in specs
/// Start bit in HBA port CMD register
pub const HBA_PORT_CMD_ST: u32 = 1 << 0;
/// FIS Receive Enable bit in HBA port CMD register
pub const HBA_PORT_CMD_FRE: u32 = 1 << 4;
/// FIS Receive Running bit in HBA port CMD register
pub const HBA_PORT_CMD_FR: u32 = 1 << 14;
/// Command List Running bit in HBA port CMD register
pub const HBA_PORT_CMD_CR: u32 = 1 << 15;

// HBA GHC bits
/// AHCI Enable bit in HBA GHC register
pub const HBA_GHC_AE: u32 = 1 << 31;

// TFD bits
// section 3.3.8 in specs
/// Indicates interface is busy
pub const HBA_PORT_TFD_BSY: u32 = 1 << 7;
/// Indicates a data transfer is requested
pub const HBA_PORT_TFD_DRQ: u32 = 1 << 3;
/// Indicates an error occurred
pub const HBA_PORT_TFD_ERR: u32 = 1 << 0;

// ATA commands
// https://wiki.osdev.org/ATA_Command_Matrix
pub const ATA_CMD_IDENTIFY: u8 = 0xEC;
pub const ATA_CMD_READ_DMA_EXT: u8 = 0x25;
pub const ATA_CMD_WRITE_DMA_EXT: u8 = 0x35;

// SATA signatures
/// Signature in the SIG register indicating an ATA device
/// https://wiki.osdev.org/AHCI#Detect_attached_SATA_devices
pub const SATA_SIG_ATA: u32 = 0x00000101;
