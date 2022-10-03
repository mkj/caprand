#![no_std]

#[cfg(feature="log")]
use log::error;

mod rng;
mod cap;

pub use rng::{setup, random};
pub use cap::cap_rand;

