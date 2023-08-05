//! Uses the random number generator via `getrandom()` as a normal program would.

#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use core::fmt::Write;

#[allow(unused_imports)]
use defmt::{debug, info, warn, error};
use {defmt_rtt as _, panic_probe as _};

use embassy_executor::Spawner;
use embassy_time::{Timer, Duration};

use getrandom::register_custom_getrandom;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let mut gpio = p.PIN_10;

    caprand::setup(&mut gpio).unwrap();
    register_custom_getrandom!(caprand::getrandom);

    loop {
        let mut mystery = [0u8; 10];
        getrandom::getrandom(mystery.as_mut_slice()).unwrap();

        let mut s = heapless::String::<33>::new();
        for m in mystery.iter() {
            write!(s, "{:02x} ", *m).unwrap();
        }
        info!("mystery bytes!  {}", s.as_str());
        Timer::after(Duration::from_millis(333)).await;
    }
}
