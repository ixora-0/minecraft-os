use bootloader::DiskImageBuilder;
use std::{env, io, path::PathBuf, process::Command};

/// Custom target: `x86_64-unknown-none` with hardware SSE2 instead of soft-float.
const KERNEL_TARGET: &str = "x86_64-minecraft-os";

fn main() {
    let kernel_path = build_kernel();
    let disk_builder = DiskImageBuilder::new(kernel_path);

    // specify output paths
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let uefi_path = out_dir.join("uefi.img");
    let bios_path = out_dir.join("bios.img");

    // create the disk images
    disk_builder.create_uefi_image(&uefi_path).unwrap();
    disk_builder.create_bios_image(&bios_path).unwrap();

    // pass the disk image paths via environment variables
    println!("cargo:rustc-env=UEFI_IMAGE={}", uefi_path.display());
    println!("cargo:rustc-env=BIOS_IMAGE={}", bios_path.display());
}

/// Builds the kernel with a nested cargo invocation and returns the path to its binary.
///
/// The custom target has no prebuilt `core`, so it needs `-Zbuild-std`. Cargo panics
/// when build-std is combined with an artifact dependency, so the kernel can't be an
/// artifact build-dependency.
fn build_kernel() -> PathBuf {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let target_spec = root.join(format!("{KERNEL_TARGET}.json"));
    // separate target dir, so the nested cargo doesn't wait on the outer cargo's lock
    let target_dir = root.join("target").join("kernel");
    // "debug" or "release"
    let profile = env::var("PROFILE").unwrap();

    let mut cmd = Command::new(env::var("CARGO").unwrap());
    cmd.current_dir(&root)
        .args(["build", "-p", "kernel", "--bin", "kernel"])
        .arg("--target")
        .arg(&target_spec)
        .arg("--target-dir")
        .arg(&target_dir)
        .args([
            "-Zjson-target-spec",
            "-Zbuild-std=core,alloc,compiler_builtins",
            "-Zbuild-std-features=compiler-builtins-mem",
        ]);
    if profile == "release" {
        cmd.arg("--release");
    }
    // flags and wrappers of the outer (host) build don't apply to the kernel
    for var in [
        "CARGO_ENCODED_RUSTFLAGS",
        "RUSTFLAGS",
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "CARGO_TARGET_DIR",
        "CARGO_BUILD_TARGET",
    ] {
        cmd.env_remove(var);
    }
    // stdout of a build script is parsed for cargo directives, keep cargo's output off it
    cmd.stdout(io::stderr());

    let status = cmd.status().expect("failed to run cargo for the kernel");
    assert!(status.success(), "kernel build failed");

    for path in [
        "kernel",
        "kernel-core",
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        &format!("{KERNEL_TARGET}.json"),
    ] {
        println!("cargo:rerun-if-changed={}", root.join(path).display());
    }

    target_dir.join(KERNEL_TARGET).join(profile).join("kernel")
}
