default:
    @just --list

run:
    cargo run -p minecraft-os

run-bios:
    cargo run -p minecraft-os --bin qemu-bios

test:
    cargo test

integration-test:
    cargo integration-test
