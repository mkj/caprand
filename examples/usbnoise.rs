//! Prints raw samples from the capacitor random number generator.
//! These would not usually be used directly, instead use `CapRng`.
//! Raw samples are useful to analyse the entropy.

#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

#[cfg(not(feature = "defmt"))]
#[allow(unused_imports)]
use log::{debug, info, warn, error};
#[cfg(not(feature = "defmt"))]
use panic_abort as _;

#[cfg(feature = "defmt")]
#[allow(unused_imports)]
use defmt::{debug, info, warn, panic, error, trace};
#[cfg(feature = "defmt")]
use {defmt_rtt as _, panic_probe as _};

use embassy_rp::gpio::Pin;
use embassy_executor::Spawner;
use embassy_futures::join::join;
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
    let mut config = Config::new(0x6666, 0x5c4f);
    config.manufacturer = Some("Matt Johnston");
    config.product = Some("caprand raw noise");
    config.serial_number = Some("12345678");
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
            let _ = run(&mut p.PIN_10, &mut class).await;
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

async fn run<'d, T: Instance + 'd>(pin: &mut impl Pin, class: &mut CdcAcmClass<'d, Driver<'d, T>>) -> Result<(), ()> {

    // max packet is 64 bytes. We discard the first sample from the buffer.
    const CHUNK: usize = 32;

    let low_cycles = 1;
    let mut noise = caprand::cap::RawNoise::new(pin, low_cycles);

    loop {
        // discard one value at the top of the loop
        noise.next().unwrap()?;

        let mut hex = [b'B'; CHUNK*2+1];
        let mut m = hex.iter_mut();
        for _ in 0..CHUNK {
            let c = noise.next().unwrap()?;
            *m.next().unwrap() = nibble_hex(c >> 4);
            *m.next().unwrap() = nibble_hex(c & 0xf);
        }

        *m.next().unwrap() = b'\n';
        // discard first sample
        class.write_packet(&hex[2..]).await.unwrap();
    }
}
