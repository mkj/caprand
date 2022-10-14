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

use caprand::cap;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    // let mut cp = cortex_m::peripheral::Peripherals::take().unwrap();

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

    const PRINT: usize = 20000;
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

    // let mut gpios = [
    //     // serial
    //     // p.PIN_0.degrade().into_ref(),
    //     // p.PIN_1.degrade().into_ref(),

    //     p.PIN_2.degrade(),
    //     p.PIN_3.degrade(),
    //     p.PIN_4.degrade(),
    //     p.PIN_5.degrade(),
    //     p.PIN_6.degrade(),
    //     p.PIN_7.degrade(),
    //     p.PIN_8.degrade(),
    //     p.PIN_9.degrade(),
    //     p.PIN_10.degrade(),
    //     p.PIN_11.degrade(),
    //     p.PIN_12.degrade(),
    //     p.PIN_13.degrade(),
    //     p.PIN_14.degrade(),
    //     p.PIN_15.degrade(),
    //     p.PIN_16.degrade(),
    //     p.PIN_17.degrade(),
    //     p.PIN_18.degrade(),
    //     p.PIN_19.degrade(),
    //     p.PIN_20.degrade(),
    //     p.PIN_21.degrade(),
    //     p.PIN_22.degrade(),
    //     // wl_on
    //     // p.PIN_23.degrade().into_ref(),
    //     // p.PIN_24.degrade().into_ref(),
    //     // wl_cs, gate of vsys adc mosfet
    //     p.PIN_25.degrade(),
    //     p.PIN_26.degrade(),
    //     p.PIN_27.degrade(),
    //     p.PIN_28.degrade(),
    //     // p.PIN_29.degrade().into_ref(),
    // ];
    let mut gpios = [
        p.PIN_25.degrade(),
        // p.PIN_10.degrade(),
    ];

    const DUMPS: usize = 1<<17;
    let mut dump  = [0u8; DUMPS];

    loop {
        dump.fill(0);
        for gpio in gpios.iter_mut() {
            let pin = gpio.pin();
            let low_cycles = cap::best_low_time(gpio, 30..=50u32).unwrap();
            let mut n = 0;
            let mut hist = [0u32; 33];
            cap::noise(gpio, low_cycles,
                |v| {
                    // info!("{}", v);
                    dump[n % DUMPS] = v as u8;
                    let lb = cap::lsb(v);
                    hist[lb] += 1;
                    n += 1;
                    if n % PRINT == 0 {
                        info!("gpio {} delay {} iter {}", pin, low_cycles, n);
                        for (p, h) in hist.iter_mut().enumerate() {
                            if *h > 0 {
                                info!("{}: {}", p, h);
                                *h = 0;
                            }
                        }
                    }
                    n < PRINT
            }).unwrap();
        info!("done");
        }
    }
}
