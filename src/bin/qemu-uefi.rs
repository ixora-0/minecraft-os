use std::{
    env,
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
