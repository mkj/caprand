//! Prints raw samples from the capacitor random number generator.
//! These would not usually be used directly, instead use `CapRng`.
//! Raw samples are useful to analyse the entropy.

#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

#[cfg(not(feature = "defmt"))]
use log::{debug, info, warn, error};
#[cfg(not(feature = "defmt"))]
use panic_abort as _;

#[cfg(feature = "defmt")]
use defmt::{debug, info, warn, panic, error, trace};
#[cfg(feature = "defmt")]
use {defmt_rtt as _, panic_probe as _};

use embassy_rp::gpio::{Flex, AnyPin, Pin};
use embassy_rp::Peripheral;
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_time::{Timer, Duration, Instant};
use embassy_rp::interrupt;
use embassy_rp::usb::{Driver, Instance};
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_usb::{Builder, Config};

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("top");
    let mut p = embassy_rp::init(Default::default());

    // Create the driver, from the HAL.
    let irq = interrupt::take!(USBCTRL_IRQ);
    let driver = Driver::new(p.USB, irq);

    // Create embassy-usb Config
    let mut config = Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("Embassy");
    config.product = Some("USB noise");
    config.serial_number = Some("12345678");
    // let mut config = Config::new(0x6666, 0x628d);
    // config.manufacturer = Some("Matt");
    // config.product = Some("Noise");
    // config.serial_number = Some("12345");
    config.max_power = 100;
    config.max_packet_size_0 = 64;

    // Required for windows compatiblity.
    // https://developer.nordicsemi.com/nRF_Connect_SDK/doc/1.9.1/kconfig/CONFIG_CDC_ACM_IAD.html#help
    config.device_class = 0xEF;
    config.device_sub_class = 0x02;
    config.device_protocol = 0x01;
    config.composite_with_iads = true;

    // Create embassy-usb DeviceBuilder using the driver and config.
    // It needs some buffers for building the descriptors.
    let mut device_descriptor = [0; 256];
    let mut config_descriptor = [0; 256];
    let mut bos_descriptor = [0; 256];
    let mut control_buf = [0; 64];

    let mut state = State::new();

    let mut builder = Builder::new(
        driver,
        config,
        &mut device_descriptor,
        &mut config_descriptor,
        &mut bos_descriptor,
        &mut control_buf,
        None,
    );

    // Create classes on the builder.
    let mut class = CdcAcmClass::new(&mut builder, &mut state, 64);

    // Build the builder.
    let mut usb = builder.build();

    // Run the USB device.
    let usb_fut = usb.run();

    // Do stuff with the class!
    let echo_fut = async {
        loop {
            class.wait_connection().await;
            info!("Connected");
            let _ = run(&mut p.PIN_25, &mut class).await;
            info!("Disconnected");
        }
    };

    join(usb_fut, echo_fut).await;
}


fn nibble_hex(c: u8) -> u8 {
    debug_assert!(c <= 0xf);
    if c < 10 {
        b'0' + c
    } else {
        b'a' - 0xa + c
    }
}

// struct Extractor {
//     acc: u8,
//     bit: u8,
//     deci_pos: u8,
//     prev: u8
// }

// impl Extractor {
//     const DECIM: u8 = 32;
//     fn new() -> Self {
//         acc: 0,
//         bit: 0,
//         deci_pos: 0,
//         prev: u8,
//     }

//     fn feed(&mut self, v: u8) -> Option<u8> {
//         self.deci_pos += 1;
//         if self.deci_pos % DECIM != 0 {
//             return None
//         }

//         jk
//     }
// }

async fn run<'d, T: Instance + 'd>(pin: &mut impl Pin, class: &mut CdcAcmClass<'d, Driver<'d, T>>) -> Result<(), ()> {

    // max packet is 64 bytes. we hex encode and add newline, 31*2+1
    const CHUNK: usize = 31;

    let mut buf = [false; CHUNK * 8 * 10];

    let mut a = None;
    let mut pos = 0u32;
    let deci = 40u32;
    loop {
        let mut b = buf.iter_mut();
        let low_delay = 30;
        // XXX should discard first iter or something.
        caprand::cap::noise(pin, low_delay,
            |v| {
                pos = pos.wrapping_add(1);
                if pos % deci == 0 {
                    let v = v as u8;
                    if a.is_none() {
                        a = Some(v);
                        true
                    } else {
                        let aa = a.take().unwrap();
                        if aa != v {
                            if let Some(i) = b.next() {
                                *i = aa > v;
                                true
                            } else {
                                false
                            }
                        } else {
                            true
                        }
                    }

                } else {
                    // discard
                    true
                }
        }).unwrap();

        for chunk in buf.chunks(CHUNK*8) {
            // write!(&mut buf, "{:?}", chunk);
            let mut hex = ['B' as u8; CHUNK*2+1];
            let mut c = chunk.iter();
            let mut m = hex.iter_mut();
            for _ in 0..CHUNK {
                let mut x = 0u8;
                for b in 0..8 {
                    x |= (*c.next().unwrap() as u8) << b;
                }
                *m.next().unwrap() = nibble_hex(x >> 4);
                *m.next().unwrap() = nibble_hex(x & 0xf);
            }
            *m.next().unwrap() = b'\n';
            class.write_packet(&hex).await.unwrap();
        }
    }
}