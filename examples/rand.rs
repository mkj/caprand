//! Uses the random number generator via `getrandom()` as a normal program would.

#![no_std]
#![no_main]

use core::fmt::Write;

#[allow(unused_imports)]
use defmt::{debug, error, info, warn};
use {defmt_rtt as _, panic_probe as _};

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let gpio = p.PIN_10;

    caprand::setup(gpio).unwrap();

    loop {
        let mut mystery = [0u8; 10];
        getrandom::fill(mystery.as_mut_slice()).unwrap();

        let mut s = heapless::String::<33>::new();
        for m in mystery.iter() {
            write!(s, "{:02x} ", *m).unwrap();
        }
        info!("mystery bytes!  {}", s.as_str());
        Timer::after(Duration::from_millis(333)).await;
    }
}

#[unsafe(no_mangle)]
unsafe extern "Rust" fn __getrandom_v03_custom(
    dest: *mut u8,
    len: usize,
) -> Result<(), getrandom::Error> {
    caprand::getrandom_raw(dest, len).map_err(|_| getrandom::Error::new_custom(123))
}
