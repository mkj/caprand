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

use embassy_rp::gpio::{Flex, AnyPin, Pin};
use embassy_rp::Peripheral;
use embassy_executor::Spawner;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let mut cp = cortex_m::peripheral::Peripherals::take().unwrap();

    // 0.1uF
    // let mut gpio = (Flex::new(p.PIN_6), 6);
    // 0.01uF ?
    // let mut gpio = (Flex::new(p.PIN_10), 10);
    // nothing
    // let mut gpio = Flex::new(p.PIN_22);
    // nothing, adc pin
    // let mut gpio = (Flex::new(p.PIN_28), 28);

    // let mut gpio = (Flex::new(p.PIN_15), 15);

    // // wifi sd_clk, or led for non-w
    // let mut gpio = (Flex::new(p.PIN_29), 29);
    // // check wifi is off.
    // let mut gpio23 = Flex::new(p.PIN_23);
    // assert!(gpio23.is_low());
    // // wifi cs high
    // let mut gpio25 = Flex::new(p.PIN_25);
    // gpio25.set_low();
    // gpio25.set_as_output();

    // let mut gpio = Flex::new(p.PIN_15);
    // let mut gpio = Flex::new(p.PIN_20);
    // let mut gpio = p.PIN_20.into();

    let mut hist = [0u32; 200];

    let PRINT = 1000000;
    // let PRINT = 50;

    // let mut gpios = [
    //     p.PIN_22.degrade().into_ref(),
    //     p.PIN_6.degrade().into_ref(),
    //     p.PIN_10.degrade().into_ref(),
    //     p.PIN_13.degrade().into_ref(),
    //     // p.PIN_23.degrade().into_ref(),
    //     // p.PIN_24.degrade().into_ref(),
    //     p.PIN_25.degrade().into_ref(),
    // ];

    let mut gpios = [
        // serial
        // p.PIN_0.degrade().into_ref(),
        // p.PIN_1.degrade().into_ref(),

        // p.PIN_2.degrade().into_ref(),
        // p.PIN_3.degrade().into_ref(),
        // p.PIN_4.degrade().into_ref(),
        // p.PIN_5.degrade().into_ref(),
        // p.PIN_6.degrade().into_ref(),
        // p.PIN_7.degrade().into_ref(),
        // p.PIN_8.degrade().into_ref(),
        // p.PIN_9.degrade().into_ref(),
        // p.PIN_10.degrade().into_ref(),
        // p.PIN_11.degrade().into_ref(),
        // p.PIN_12.degrade().into_ref(),
        // p.PIN_13.degrade().into_ref(),
        // p.PIN_14.degrade().into_ref(),
        // p.PIN_15.degrade().into_ref(),
        // p.PIN_16.degrade().into_ref(),
        // p.PIN_17.degrade().into_ref(),
        // p.PIN_18.degrade().into_ref(),
        // p.PIN_19.degrade().into_ref(),
        // p.PIN_20.degrade().into_ref(),
        // p.PIN_21.degrade().into_ref(),
        // p.PIN_22.degrade().into_ref(),
        // p.PIN_23.degrade().into_ref(),
        // p.PIN_24.degrade().into_ref(),
        // wl_cs, gate of vsys adc mosfet
        p.PIN_25.degrade().into_ref(),
        // p.PIN_26.degrade().into_ref(),
        // p.PIN_27.degrade().into_ref(),
        // p.PIN_28.degrade().into_ref(),
        // p.PIN_29.degrade().into_ref(),
    ];

    info!("gpio,delay,time");
    for gpio in gpios.iter_mut() {
        let low_delay = 69;
        // for low_delay in 0..1{
            let pin = gpio.pin();
            let mut n = 0;
            caprand::noise(gpio.reborrow(), low_delay,
                |v| {
                    info!("{},{},{}", pin, low_delay, v);
                    n += 1;
                    n < PRINT
            }).unwrap();
        // }
    }
    cortex_m::asm::bkpt()
}
