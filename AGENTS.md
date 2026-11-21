# minecraft-os

Hobby x86_64 OS in Rust (`no_std`) that boots into a voxel game.

- `kernel/`: bare-metal kernel and drivers.
- `kernel-core/`: hardware-independent game and rendering logic, testable on the host.
- Root crate: `build.rs` packs the kernel into UEFI/BIOS disk images; `src/bin/` holds the QEMU runners.

## Environment

- With nix: the `flake.nix` dev shell (via direnv, or `nix develop -c <cmd>`) provides the toolchain, `qemu`, and `just`.
- Without nix: install rustup (it reads `rust-toolchain.toml`), `qemu-system-x86_64`, and optionally `just`.

## Kernel target

The kernel is built for `x86_64-minecraft-os.json`: `x86_64-unknown-none` with hardware SSE2 instead of soft-float (`kernel::enable_sse` turns SSE on at boot). The spec was generated from the built-in target, then edited:

```sh
rustc -Z unstable-options --print target-spec-json --target x86_64-unknown-none > x86_64-minecraft-os.json
```

- `features`: `-mmx,-sse,-sse2,...,+soft-float` -> `-mmx,+sse,+sse2,-sse3,...` (no `+soft-float`).
- Removed `"rustc-abi": "softfloat"`.
- `metadata`: updated `description`, `tier` set to `null`.

Regenerate it the same way after a toolchain bump if the spec format changes.

- Custom targets have no prebuilt `core`, so they need `-Zbuild-std`. Cargo panics when build-std is combined with an artifact dependency or `default-target`, so `build.rs` builds the kernel with a nested `cargo build` into `target/kernel/` instead.
- `default-target` in `kernel/` and `tests-integration/` stays `x86_64-unknown-none`, so `cargo clippy` and rust-analyzer check the kernel against the soft-float target. Same source, only float codegen differs.
- `just test-integration` builds the tests for the SSE target.

## Commands

- `just check`: formatting, clippy, and kernel-core tests, same as CI. Run before finishing a change.
- `just format`, `just lint`, `just test-integration` (runs tests inside QEMU), `just run`.

### Verifying headless

Kernel logs go to serial. To check a change without a window, run QEMU directly (after `just build`; `target/ovmf` is fetched by the first `just run`):

```sh
qemu-system-x86_64 \
  -drive format=raw,if=pflash,readonly=on,file=target/ovmf/x64/code.fd \
  -drive format=raw,if=pflash,file=<copy of target/ovmf/x64/vars.fd> \
  -drive format=raw,file=<newest target/debug/build/minecraft-os/*/out/uefi.img> \
  -serial file:serial.log -display none -monitor stdio
```

Several hash directories can exist under `build/minecraft-os/`, so pick the newest image (`ls -t`). Older toolchains used `build/minecraft-os-<hash>/out/`, and those leftover images are stale. Use `target/release/...` after a release build.

Monitor commands: `screendump out.ppm` captures the framebuffer, `mouse_move dx dy` / `sendkey <key>` inject input, `quit` exits.

## Git and releases

- Do not commit automatically, unless explicitly instructed to do so.
- Conventional commit messages title: `type: Capitalized summary.`
- Try to make atomic commits.
- Release: bump `workspace.package.version` in the root `Cargo.toml`, add a `CHANGELOG.md` section, commit `chore: Bump version to X.Y.Z.`, then push tag `vX.Y.Z`. `release.yml` builds the images and publishes the GitHub release.
