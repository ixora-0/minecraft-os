use std::{
    env, fs,
    process::{self, Command},
};

fn main() {
    let image = env::args()
        .nth(1)
        .unwrap_or_else(|| env!("BIOS_IMAGE").to_string());
    let mut qemu = Command::new("qemu-system-x86_64");
    qemu.arg("-drive");
    qemu.arg(format!("format=raw,file={}", image));
    // hardware acceleration if available, falls back to TCG otherwise
    if fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/kvm")
        .is_ok()
    {
        qemu.arg("-accel").arg("kvm").arg("-cpu").arg("host");
    }
    let exit_status = qemu.status().unwrap();
    process::exit(exit_status.code().unwrap_or(-1));
}
