#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

#[cfg(not(feature = "defmt"))]
use log::{debug, info, warn, error};

#[cfg(feature = "defmt")]
use defmt::{debug, info, warn, panic, error};
#[cfg(feature = "defmt")]
use {defmt_rtt as _, panic_probe as _};

use embassy_rp::gpio::Pin;
use embassy_rp::Peripheral;
use embassy_executor::Spawner;
use embassy_time::{Timer, Duration};

use getrandom::register_custom_getrandom;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let gpio = p.PIN_25.degrade().into_ref();

    caprand::setup(gpio).unwrap();

    register_custom_getrandom!(caprand::random);

    loop {
        let mut mystery = [0u8; 10];
        getrandom::getrandom(mystery.as_mut_slice()).unwrap();

        info!("mystery bytes!");
        for m in mystery.iter() {
            info!("{:x}", *m);
        }
        Timer::after(Duration::from_millis(333)).await;
    }
}
