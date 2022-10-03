//! Prints raw samples from the capacitor random number generator.
//! These would not usually be used directly, instead use `CapRng`.
//! Raw samples are useful to analyse the entropy.

#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

#[cfg(not(feature = "defmt"))]
use log::{debug, info, warn, error};

#[cfg(feature = "defmt")]
use defmt::{debug, info, warn, panic, error};
#[cfg(feature = "defmt")]
use {defmt_rtt as _, panic_probe as _};

use embassy_rp::gpio::Flex;
use embassy_executor::Spawner;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let mut cp = cortex_m::peripheral::Peripherals::take().unwrap();

    let mut gpio = (Flex::new(p.PIN_10), 10);
    loop {
        caprand::cap_rand(&mut gpio.0, gpio.1, &mut cp.SYST, 10_000,
            |v| info!("{}", v)).unwrap();
    }
}
