use bootloader::DiskImageBuilder;
use ovmf_prebuilt::{Arch, FileType, Prebuilt, Source};
use std::{
    env,
    path::PathBuf,
    process::{Command, exit},
};
use tests_integration::QemuExitCode;

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent().unwrap();

    // Cargo passes the compiled binary path as arg1
    let compiled_binary = PathBuf::from(env::args().nth(1).expect("missing kernel path"));
    println!("Running compiled binary at: {}", compiled_binary.display());
    let compiled_binary_name = compiled_binary.file_name().unwrap().to_str().unwrap();

    // directory for generated artifacts
    let out_dir = workspace_root.join("target/bootimage");
    std::fs::create_dir_all(&out_dir).unwrap();

    let uefi_img = out_dir.join(format!("{}-uefi.img", compiled_binary_name));

    // build bootable image
    let builder = DiskImageBuilder::new(compiled_binary);
    println!("Building uefi image to {}", uefi_img.display());
    builder.create_uefi_image(&uefi_img).unwrap();

    // fetch OVMF firmware
    let prebuilt = Prebuilt::fetch(Source::LATEST, workspace_root.join("target/ovmf")).unwrap();
    let ovmf_code = prebuilt.get_file(Arch::X64, FileType::Code);
    let ovmf_vars = prebuilt.get_file(Arch::X64, FileType::Vars);

    // run qemu
    let status = Command::new("qemu-system-x86_64")
        .args([
            "-drive",
            &format!(
                "format=raw,if=pflash,readonly=on,file={}",
                ovmf_code.display()
            ),
            "-drive",
            &format!("format=raw,if=pflash,file={}", ovmf_vars.display()),
            "-drive",
            &format!("format=raw,file={}", uefi_img.display()),
            "-serial",
            "stdio",
            "-display",
            "none",
            "-device",
            "isa-debug-exit,iobase=0xf4,iosize=0x04",
        ])
        .status()
        .expect("failed to start qemu");

    let exit_code = match status.code().unwrap_or(-1) {
        QemuExitCode::TEST_SUCCEESS_EXIT_CODE => 0,
        c => c,
    };
    exit(exit_code);
}
