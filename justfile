default:
    @just --list

alias r:= run
run *ARGS:
    cargo run -p minecraft-os -- {{ARGS}}

alias rr := run-release
run-release *ARGS:
    cargo run --release -p minecraft-os -- {{ARGS}}

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

alias f := format
format:
    cargo fmt --all

alias l := lint
lint:
    cargo clippy -- -D warnings

alias c := check
check:
    cargo fmt --all --check
    cargo clippy -- -D warnings
    cargo test -p kernel-core

move-images:
    cargo run -p minecraft-os --bin move-images
