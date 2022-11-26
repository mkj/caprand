//! Prints raw samples from the capacitor random number generator.
//! These would not usually be used directly, instead use `CapRng`.
//! Raw samples are useful to analyse the entropy.

#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

#[cfg(not(feature = "defmt"))]
use log::{debug, info, warn, error, trace};

#[cfg(feature = "defmt")]
use defmt::{debug, info, warn, panic, error};
#[cfg(feature = "defmt")]
use {defmt_rtt as _, panic_probe as _};

use embassy_rp::gpio::{Flex, AnyPin, Pin};
use embassy_rp::Peripheral;
use embassy_executor::Spawner;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let mut p = embassy_rp::init(Default::default());
    let mut cp = cortex_m::peripheral::Peripherals::take().unwrap();

    let low_cycles = 12;
    let pin = &mut p.PIN_10;
    let syst = &mut cp.SYST;
    let mut noise = caprand::cap::Noise::new(pin, low_cycles).unwrap();

    const ITER: usize = 1000;

    // This might break embassy if we were using async timers.
    syst.set_clock_source(cortex_m::peripheral::syst::SystClkSource::Core);

    loop {
        let mut buf = [0u32; ITER];

        for b in buf.iter_mut() {
            *b = noise.next_with_systick(syst).unwrap();
        }

        // discard the first sample
        for b in buf[1..].iter() {
            info!("{}", b);
        }
        info!("");
    }
}
