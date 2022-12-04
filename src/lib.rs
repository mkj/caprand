#![doc = include_str!("../README.md")]

#![no_std]

#[cfg(feature="log")]
use log::error;

pub mod rng;
pub mod cap;

pub use rng::{setup, getrandom};

