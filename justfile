default:
    @just --list

run *ARGS:
    cargo run -p minecraft-os -- {{ARGS}}

run-bios *ARGS:
    cargo run -p minecraft-os --bin qemu-bios -- {{ARGS}}

alias t := test
test:
    cargo test

alias ti := test-integration
test-integration:
    cargo test-integration

alias b := build
build:
    cargo build

alias br := build-release
build-release:
    cargo build --release

move-images:
    cargo run -p minecraft-os --bin move-images
