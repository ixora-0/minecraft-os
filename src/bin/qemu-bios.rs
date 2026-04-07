use std::{
    env,
    process::{self, Command},
};

fn main() {
    let image = env::args()
        .nth(1)
        .unwrap_or_else(|| env!("BIOS_IMAGE").to_string());
    let mut qemu = Command::new("qemu-system-x86_64");
    qemu.arg("-drive");
    qemu.arg(format!("format=raw,file={}", image));
    let exit_status = qemu.status().unwrap();
    process::exit(exit_status.code().unwrap_or(-1));
}
