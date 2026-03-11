#![no_std]
#![feature(abi_x86_interrupt)]

pub mod gdt;
pub mod interrupts;
pub mod logger;
pub mod rendering;
pub mod serial;

pub fn init() {
    gdt::init();
    interrupts::init_idt();
}
pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}
