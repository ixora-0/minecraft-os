use std::{
    env, fs,
    process::{self, Command},
};

use clap::Parser;

#[derive(Parser, Debug)]
struct Args {
    /// QEMU display backend, e.g. `sdl` or `gtk` [default: `sdl` on Wayland, QEMU's default otherwise]
    #[arg(long)]
    display: Option<String>,
    #[arg(default_value = env!("BIOS_IMAGE"))]
    image: String,
}

fn main() {
    let args = Args::parse();
    let mut qemu = Command::new("qemu-system-x86_64");
    qemu.arg("-drive");
    qemu.arg(format!("format=raw,file={}", args.image));
    // GTK can't capture relative mouse motion on Wayland, so default to SDL there
    let display = args
        .display
        .or_else(|| env::var_os("WAYLAND_DISPLAY").map(|_| "sdl".to_string()));
    if let Some(display) = display {
        qemu.arg("-display").arg(display);
    }
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
