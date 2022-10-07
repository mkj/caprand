#![no_std]

#[cfg(feature="log")]
use log::error;

mod rng;
mod cap;
// mod numpin;

pub use rng::{setup, random};
pub use cap::noise;

