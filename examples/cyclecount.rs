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

use cortex_m::peripheral::SYST;

use heapless::String;
use core::fmt::Write;

use caprand::cap;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("top");
    let mut p = embassy_rp::init(Default::default());
    let mut cp = cortex_m::peripheral::Peripherals::take().unwrap();

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

    let syst = &mut cp.SYST;
    // This might break embassy if we were using async timers.
    syst.set_clock_source(cortex_m::peripheral::syst::SystClkSource::Core);

    // Do stuff with the class!
    let echo_fut = async {
        loop {
            class.wait_connection().await;
            info!("Connected");
            let _ = run(&mut p.PIN_10, syst, &mut class).await;
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

async fn run<'d, T: Instance + 'd>(pin: &mut impl Pin, syst: &mut SYST, class: &mut CdcAcmClass<'d, Driver<'d, T>>) -> Result<(), ()> {

    // max packet is 64 bytes. We discard the first sample from the buffer.
    const CHUNK: usize = 32;

    // let low_cycles = caprand::cap::best_low_time(pin, 10..=90u32).unwrap();
    let low_cycles = 1;
    let mut noise = caprand::cap::RawNoise::new(pin, low_cycles)?;
    trace!("low_cycles = {}", low_cycles);

    let mut s = String::<64>::new();

    loop {

        let mut buf = [0u32; 1000];
        for b in buf.iter_mut() {
            *b = noise.next_with_systick(syst).unwrap();
        }

        // discard the first few
        for b in &buf[10..] {
            if write!(s, "{}\n", b).is_err() {
                class.write_packet(s.as_ref()).await.unwrap();
                s.clear();
                write!(s, "{}\n", b).unwrap();
            }
        }
    }
}
