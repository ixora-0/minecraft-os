#![no_std]

pub mod logger;
pub mod rendering;
pub mod serial;

pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}
