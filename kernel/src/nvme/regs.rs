//! See NVMe specification 2.3 for register layouts
//! https://nvmexpress.org/wp-content/uploads/NVM-Express-Base-Specification-Revision-2.3-2025.08.01-Ratified.pdf

// https://wiki.osdev.org/NVMe#Base_address_registers
// and section 3.1.4 in specs
/// NVMe controller registers (BAR0).
#[repr(C)]
pub struct NvmeRegisters {
    /// Controller Capabilities.
    /// 0x00 - 0x07
    pub cap: u64,
    /// Version.
    /// 0x08 - 0x0B
    pub vs: u32,
    /// Interrupt Mask Set.
    /// 0x0C - 0x0F
    pub intms: u32,
    /// Interrupt Mask Clear.
    /// 0x10 - 0x13
    pub intmc: u32,
    /// Controller Configuration.
    /// 0x14 - 0x17
    pub cc: u32,
    /// Reserved.
    /// 0x18 - 0x1B
    _reserved: u32,
    /// Controller Status.
    /// 0x1C - 0x1F
    pub csts: u32,
    /// NVM Subsystem Reset.
    /// 0x20 - 0x23
    pub nssr: u32,
    /// Admin Queue Attributes.
    /// 0x24 - 0x27
    pub aqa: u32,
    /// Admin Submission Queue Base Address.
    /// 0x28 - 0x2F
    pub asq: u64,
    /// Admin Completion Queue Base Address.
    /// 0x30 - 0x37
    pub acq: u64,
}

// CAP field accessors
// section 3.1.4.1 in specs
/// Maximum Queue Entries Supported (0-based)
pub fn cap_mqes(cap: u64) -> u16 {
    // bits 15:0
    (cap & 0xFFFF) as u16
}
/// Get DSTRD (u4).
/// Doorbell stride = 2^(2+DSTRD) bytes = 4 << DSTRD
// (secion 3.1.4.1 in specs)
pub fn cap_dstrd(cap: u64) -> u64 {
    // bits 35:32
    (cap >> 32) & 0xF
}

// CC bits
// section 3.1.4.5 in specs
/// Enable bit. Bit 0.
pub const CC_EN: u32 = 1 << 0;
/// I/O command set selected. Bits 6:4
pub const CC_CSS_NVM: u32 = 0 << 4;
/// Memory Page Size. The memory page size is 2 ^ (12 + MPS).
/// MPS = 0 corresponds to 4 KiB pages. Bits 10:7
pub const CC_MPS_4K: u32 = 0 << 7;
/// Defines the I/O Submission Queue Entry Size that is used
/// for the selected I/O Command Set(s).
/// We set to 64 bytes (2^6). Bits 19:16
pub const CC_IOSQES_64: u32 = 6 << 16;
/// Defines the I/O Completion Queue Entry Size that is used
/// for the selected I/O Command Set(s).
/// We set to 16 bytes (2^4). Bits 23:20
pub const CC_IOCQES_16: u32 = 4 << 20;

// CSTS bits
// section 3.1.4.6 in specs
/// Ready bit. Bit 0.
pub const CSTS_RDY: u32 = 1 << 0;

// Admin command opcodes
// https://wiki.osdev.org/NVMe#Admin_commands
/// Identify.
pub const ADMIN_IDENTIFY: u8 = 0x06;
/// Create I/O Completion Queue.
pub const ADMIN_CREATE_IO_CQ: u8 = 0x05;
/// Create I/O Submission Queue.
pub const ADMIN_CREATE_IO_SQ: u8 = 0x01;

// I/O command opcodes
// https://wiki.osdev.org/NVMe#IO_commands
// also NVM Command Set Specification section 3.3
// https://nvmexpress.org/wp-content/uploads/NVM-Express-NVM-Command-Set-Specification-Revision-1.1-2024.08.05-Ratified.pdf
pub const IO_CMD_READ: u8 = 0x02;
pub const IO_CMD_WRITE: u8 = 0x01;

// https://wiki.osdev.org/NVMe#Submission_queue_entry
// section 4.1.1 in specs
/// NVMe Submission Queue Entry
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct NvmeSqe {
    /// Command DWord 0.
    /// Opcode (7:0), FUSE (9:8), PSDT (15:14), CID (31:16)
    pub cdw0: u32,
    /// Namespace ID.
    pub nsid: u32,
    /// Command DWord 2.
    pub cdw2: u32,
    /// Command DWord 3.
    pub cdw3: u32,
    /// Metadata pointer.
    pub mptr: u64,

    // Data pointer, specifies data used in command.
    /// PRP Entry 1 (data buffer physical address)
    pub prp1: u64,
    /// PRP Entry 2 (for transfers spanning two pages, or PRP list pointer)
    pub prp2: u64,

    /// Command DWord 10.
    pub cdw10: u32,
    /// Command DWord 11.
    pub cdw11: u32,
    /// Command DWord 12.
    pub cdw12: u32,
    /// Command DWord 13.
    pub cdw13: u32,
    /// Command DWord 14.
    pub cdw14: u32,
    /// Command DWord 15.
    pub cdw15: u32,
}

impl NvmeSqe {
    pub fn set_opcode(&mut self, opcode: u8) {
        self.cdw0 = (self.cdw0 & !0xFF) | opcode as u32;
    }

    pub fn set_cid(&mut self, cid: u16) {
        self.cdw0 = (self.cdw0 & 0xFFFF) | ((cid as u32) << 16);
    }
}

// section 4.2.1 in specs
/// NVMe Completion Queue Entry (16 bytes)
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct NvmeCqe {
    pub dw0: u32,
    pub dw1: u32,
    pub sq_head: u16,
    pub sq_id: u16,
    pub cid: u16,
    /// Bit 0 = phase tag, bits 15:1 = status
    pub status: u16,
}

impl NvmeCqe {
    /// Phase bit (bit 0 of status)
    pub fn phase(&self) -> bool {
        self.status & 1 != 0
    }

    /// Status code (bits 15:1), 0 = success
    pub fn status_code(&self) -> u16 {
        (self.status >> 1) & 0x7FF
    }
}
