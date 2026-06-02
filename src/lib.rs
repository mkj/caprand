#![doc = include_str!("../README.md")]
#![no_std]

pub mod cap;
pub mod health;
pub mod rng;

pub use rng::{getrandom, setup, CapRng};
