#![doc = include_str!("../README.md")]

#![no_std]


pub mod rng;
pub mod cap;
pub mod health;

pub use rng::{setup, getrandom, CapRng};

