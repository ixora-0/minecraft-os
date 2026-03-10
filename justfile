default:
    @just --list

run:
    cargo run -p minecraft-os

test:
    cargo test

integration-test:
    cargo integration-test
