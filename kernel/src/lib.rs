#![no_std]
#![feature(abi_x86_interrupt)]
#![feature(const_trait_impl)]
#![feature(const_default)]

extern crate alloc;

pub mod acpi;
pub mod allocator;
pub mod gdt;
pub mod interrupts;
pub mod logger;
pub mod memory;
pub mod ps2;
pub mod rendering;
pub mod serial;

use bootloader_api::BootloaderConfig;

pub static BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(bootloader_api::config::Mapping::Dynamic);
    config
};

fn build_pic_masks(enabled_irqs: &[u8]) -> (u8, u8) {
    let mut mask1: u8 = 0xFF;
    let mut mask2: u8 = 0xFF;
    let mut has_secondary_irqs = false;
    for &irq in enabled_irqs {
        if irq < 8 {
            mask1 &= !(1 << irq);
        } else if irq < 16 {
            mask2 &= !(1 << (irq - 8));
            has_secondary_irqs = true;
        }
    }
    // enable IRQ 2 (cascade) if any slave PIC IRQs are enabled
    if has_secondary_irqs {
        mask1 &= !(1 << 2);
    }
    (mask1, mask2)
}

pub fn init() {
    gdt::init();
    interrupts::init_idt();

    let (mask1, mask2) = build_pic_masks(&[0, 1, 12]);
    unsafe {
        let mut pics = interrupts::PICS.lock();
        pics.initialize();
        pics.write_masks(mask1, mask2);
    }
}
pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}
