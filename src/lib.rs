#![no_std]

#[cfg(feature="log")]
use log::error;

pub mod rng;
pub mod cap;
// mod numpin;

pub use rng::{setup, getrandom};

