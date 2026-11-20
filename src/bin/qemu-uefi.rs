use std::{
    env, fs,
    path::Path,
    process::{self, Command},
};

use clap::Parser;
use ovmf_prebuilt::{Arch, FileType, Prebuilt, Source};

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, default_value = "false")]
    nvme: bool,
    #[arg(long, default_value = "false")]
    ahci: bool,
    /// QEMU display backend, e.g. `sdl` or `gtk` [default: `sdl` on Wayland, QEMU's default otherwise]
    #[arg(long)]
    display: Option<String>,
    #[arg(default_value = env!("UEFI_IMAGE"))]
    image: String,
}

fn main() {
    let args = Args::parse();

    let prebuilt = Prebuilt::fetch(Source::LATEST, "target/ovmf").unwrap();
    let ovmf_code = prebuilt.get_file(Arch::X64, FileType::Code);
    let ovmf_vars = prebuilt.get_file(Arch::X64, FileType::Vars);

    {
        fn ensure_image_exists(path: &str) {
            if !Path::new(path).exists() {
                Command::new("qemu-img")
                    .args(["create", "-f", "raw", path, "1M"])
                    .status()
                    .unwrap();
            }
        }
        if args.ahci {
            ensure_image_exists("AHCI.img");
        }
        if args.nvme {
            ensure_image_exists("NVME.img");
        }
    }

    let mut qemu = Command::new("qemu-system-x86_64");
    qemu.arg("-drive").arg(format!(
        "format=raw,if=pflash,readonly=on,file={}",
        ovmf_code.display()
    ));
    qemu.arg("-drive")
        .arg(format!("format=raw,if=pflash,file={}", ovmf_vars.display()));
    qemu.arg("-drive")
        .arg(format!("format=raw,file={}", args.image));
    qemu.arg("-serial").arg("stdio");
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

    if args.ahci {
        qemu.arg("-drive")
            .arg("id=disk,file=AHCI.img,format=raw,if=none");
        qemu.arg("-device").arg("ich9-ahci,id=ahci");
        qemu.arg("-device").arg("ide-hd,drive=disk,bus=ahci.0");
    }
    if args.nvme {
        qemu.arg("-drive")
            .arg("id=nvme-disk,file=NVME.img,format=raw,if=none");
        qemu.arg("-device")
            .arg("nvme,serial=deadbeef,drive=nvme-disk");
    }

    let exit_status = qemu.status().unwrap();
    process::exit(exit_status.code().unwrap_or(-1));
}
