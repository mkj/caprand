//! Prints raw samples from the capacitor random number generator.
//! These would not usually be used directly, instead use `CapRng`.
//! Raw samples are useful to analyse the entropy.

#![no_std]
#![no_main]

#[allow(unused_imports)]
use defmt::{debug, info, warn, error};
use {defmt_rtt as _, panic_probe as _};

use embassy_rp::gpio;
use embassy_executor::Spawner;
use embassy_rp::uart::{BufferedInterruptHandler, BufferedUart, Config};
use embassy_rp::peripherals::UART0;
use embassy_rp::bind_interrupts;
use embedded_io_async::Write as _;

bind_interrupts!(struct Irqs {
    UART0_IRQ => BufferedInterruptHandler<UART0>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // dummy
    // let mut led = gpio::Output::new(p.PIN_14, gpio::Level::Low);
    let mut led = gpio::Output::new(p.PIN_25, gpio::Level::Low);

    let (tx_pin, rx_pin, uart) = (p.PIN_0, p.PIN_1, p.UART0);

    let mut tx_buf = [0u8; 16];
    let mut rx_buf = [0u8; 16];
    let mut conf = Config::default();
    conf.baudrate = 2500000;
    let uart = BufferedUart::new(uart, Irqs, tx_pin, rx_pin, &mut tx_buf, &mut rx_buf, conf);
    let (_rx, mut tx) = uart.split();


    let mut cap_pin = p.PIN_10;

    let mut noise = caprand::cap::RawNoise::new(&mut cap_pin, 1);

    loop {
        led.toggle();

        let mut buf = [0u8; 20000];
        for b in buf.iter_mut() {
            (*b, _) = noise.next().unwrap();
        }

        for b in buf {
            let b = caprand::cap::lsb(b);
            let line = [nibble_hex(b >> 4), nibble_hex(b & 0xf), b'\r', b'\n'];
            // let line = [nibble_hex(b >> 4), nibble_hex(b & 0xf), b'\r', b'\n'];
            tx.write_all(&line).await.unwrap();
        }
    }

    // // Do stuff with the class!
    // let echo_fut = async {
    //     loop {
    //         class.wait_connection().await;
    //         info!("Connected");
    //         // let pin = &mut p.PIN_10;
    //         let pin = &mut p.PIN_13;
    //         let _ = run(pin, &mut class).await;
    //         info!("Disconnected");
    //     }
    // };

    // join(usb_fut, echo_fut).await;
}

fn nibble_hex(c: u8) -> u8 {
    debug_assert!(c <= 0xf);
    if c < 10 {
        b'0' + c
    } else {
        b'a' - 0xa + c
    }
}

// async fn run<'d, D: embassy_usb_driver::Driver<'d>>(pin: &mut impl Pin, class: &mut CdcAcmClass<'d, D>) -> Result<(), ()> {

//     let low_cycles = 1;
//     let mut noise = caprand::cap::RawNoise::new(pin, low_cycles);

//     // discard one sample
//     noise.next();

//     loop {
//         // usb packet has 64 limit
//         let mut b = heapless::String::<64>::new();
//         while b.len() <= b.capacity() - 2 {
//             let (c, valid) = noise.next().unwrap();
//             write!(b, "{:02x}\n", c).unwrap();
//             // if valid {
//             //     write!(b, "{}\n", caprand::cap::lsb(c)).unwrap();
//             // }
//         }
//         class.write_packet(b.as_bytes()).await.unwrap()
//     }
// }
