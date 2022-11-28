//! Prints raw samples as ascii art, to usb serial.

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
use embassy_rp::pac;
use embassy_rp::usb::{Driver, Instance};
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_usb::{Builder, Config};

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("top");
    let mut p = embassy_rp::init(Default::default());

    let rosc = unsafe { pac::ROSC.ctrl().read().enable().0 };
    trace!("rosc {}", rosc);
    // unsafe{ pac::ROSC.ctrl().modify(|s| s.set_enable(pac::rosc::vals::Enable::DISABLE)) };
    let rosc = unsafe { pac::ROSC.ctrl().read().enable().0 };
    trace!("rosc {}", rosc);

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
            let _ = run(&mut p.PIN_10, &mut class).await;
            info!("Disconnected");
        }
    };

    join(usb_fut, echo_fut).await;
}

fn bin_5bits(c: u8) -> [char; 5] {
    let v = [
        c,
        c >> 1,
        c >> 2,
        c >> 3,
        c >> 4,
    ].map(|x| if (x&1) == 0 { '_' } else { '+' });

    // trace!("{:02x} {} {} {} {} {} a", c, v[0], v[1], v[2], v[3], v[4]);

    // for x in v.iter_mut().take_while(|x| **x == '_') {
    //     *x = ' '
    // }

    // for i in 0..5 {
    //     let i = 5 - i;
    //     // trace!("{} {} {}", i, v[i-1], v[i]);
    //     if v[i] == '-' && v[i-1] == '-' {
    //         // trace!("sp");
    //         v[i] = ' '
    //     } else {
    //         // trace!("br");
    //         break
    //     }
    // }
    // trace!("{:02x} {} {} {} {} {} b", c, v[0], v[1], v[2], v[3], v[4]);
    v
}

async fn run<'d, T: Instance + 'd>(pin: &mut impl Pin, class: &mut CdcAcmClass<'d, Driver<'d, T>>) -> Result<(), ()> {

    // max packet is 64 bytes. we encode and add newline.
    const CHUNK: usize = 9;


    let low_cycles = caprand::cap::best_low_time(pin, 10..=90u32).unwrap();
    trace!("low_cycles = {}", low_cycles);
    let low_cycles = 1;
    let mut noise = caprand::cap::RawNoise::new(pin, low_cycles)?;

    // let mut noise = noise.filter(|v| {
    //     if let Ok(v) = v {
    //         if v & 1 == 1 {
    //             false
    //         } else if v & 0b11110 == 0 {
    //             false
    //         } else {
    //             true
    //         }
    //     } else {
    //         true
    //     }
    // });
    loop {
        let mut buf = [b'A'; (CHUNK*(5+1))];
        let mut b = buf.iter_mut();

        // discard one value after the delay of usb write
        noise.next().unwrap()?;

        for _ in 0..CHUNK {
            let v = noise.next().unwrap()?;
            for x in bin_5bits(v) {
                *b.next().unwrap() = x as u8;
            }
            *b.next().unwrap() = b'\n';
        }
        class.write_packet(&buf).await.unwrap();
    }
}
