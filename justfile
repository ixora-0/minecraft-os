default:
    @just --list

run:
    cargo run -p minecraft-os

run-bios:
    cargo run -p minecraft-os --bin qemu-bios

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
