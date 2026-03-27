#![no_std]

#[cfg(test)]
#[macro_use]
extern crate approx; // for approximate float comparisons in tests

extern crate alloc;

pub mod game;
pub mod rendering;
