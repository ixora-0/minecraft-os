use core::sync::atomic::{AtomicU64, Ordering};
use x86_64::instructions::port::Port;

/// The PIT has 3 channels. Channel 0 (I/O port 0x40) is used for IRQ 0,
/// which generates the system timer interrupt.
const PIT_CHANNEL_0_PORT: u16 = 0x40;
const PIT_CONTROL_PORT: u16 = 0x43;

/// Control word format (port 0x43):
///  Bits 7-6: Select channel (00 = channel 0)
///  Bits 5-4: Read/Load (11 = low byte first, then high byte)
///  Bits 3-1: Mode (010 = rate generator)
///  Bit 0:    BCD (0 = binary mode, 1 = BCD mode. Should always use 0)
const PIT_CONTROL_WORD: u8 = 0b00110100;

/// Frequency of the PIT input clock in hz
const PIT_FREQ: u64 = 1_193_182; // roughly, techinically 1.193181.666... hz
/// Calculated by `PIT_FREQ / desired_freq`.
/// PIT interprets divisor 0 as 65536
const PIT_DIVISOR: u16 = 1000;
const NANOS_PER_TICK: u64 = {
    let divisor = match PIT_DIVISOR {
        0 => 65536,
        _ => PIT_DIVISOR as u64,
    };
    let freq = PIT_FREQ as f64 / divisor as f64;
    (1_000_000_000.0 / freq) as u64
};
static TICK_COUNT: AtomicU64 = AtomicU64::new(0);

/// Initialize the Programmable Interval Timer.
pub fn init() {
    unsafe {
        // write the control word to configure the PIT.
        Port::new(PIT_CONTROL_PORT).write(PIT_CONTROL_WORD);

        // write the divisor
        const LOW: u8 = (PIT_DIVISOR & 0xFF) as u8;
        const HIGH: u8 = ((PIT_DIVISOR >> 8) & 0xFF) as u8;
        Port::new(PIT_CHANNEL_0_PORT).write(LOW);
        Port::new(PIT_CHANNEL_0_PORT).write(HIGH);
    }
}

/// Called from the timer ISR on each PIT interrupt.
pub fn tick() {
    TICK_COUNT.fetch_add(1, Ordering::Relaxed);
}

/// Returns the number of PIT ticks since boot.
pub fn ticks() -> u64 {
    TICK_COUNT.load(Ordering::Relaxed)
}

/// Approximate nanoseconds since boot.
pub fn nanos_since_boot() -> u64 {
    ticks().wrapping_mul(NANOS_PER_TICK) // most likely not going to overflow
}

/// Wait for approximately (at least) `ns` nanoseconds.
pub fn sleep(ns: u64) {
    let target = nanos_since_boot().wrapping_add(ns);
    while nanos_since_boot() < target {
        // halt until the next timer interrupt
        x86_64::instructions::hlt();
    }
}
