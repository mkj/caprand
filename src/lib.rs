use core::num::NonZeroU32;
use core::cell::RefCell;
use core::ops::DerefMut;

#[cfg(feature="log")]
use log::error;

use embassy_rp::gpio::{Flex, Pull, Pin};
use cortex_m::peripheral::SYST;

use critical_section::Mutex;
use sha2::{Sha256, Digest};
use rand_chacha::ChaCha20Rng;

use rand_chacha::rand_core::{RngCore, SeedableRng};

const CHARGE_CYCLES: u32 = 600;
// 1 bit per iteration is a rough assumption, give a factor of 4 leeway
const NUM_LOOPS: usize = 1024;
const MIN_DEL: u32 = 300_000;
// Assuming at least a 125mhz clock. A slower clock could possibly be used, though
// we are assuming that the clock is fast enough to pick up variations due to
// thermal noise.
// Other constants are also assuming a 125mhz clock.
const MIN_10MS_CYCLES: u32 = 1_250_000;

// arbitrary constant
const CAPRAND_ERR: u32 = getrandom::Error::CUSTOM_START + 510132368;

pub fn error() -> getrandom::Error {
    // OK unwrap: const value is nonzero
    let c: NonZeroU32 = CAPRAND_ERR.try_into().unwrap();
    c.into()
}


static RNG: Mutex<RefCell<Option<CapRng>>> = Mutex::new(RefCell::new(None));

pub fn random(buf: &mut [u8]) -> Result<(), getrandom::Error> {
    critical_section::with(|cs| {
        let mut rng = RNG.borrow_ref_mut(cs);
        let rng = rng.deref_mut();
        if let Some(rng) = rng {
            rng.0.fill_bytes(buf);
            Ok(())
        } else {
            error!("setup() not called");
            Err(error())
        }
    })
}

pub(crate) fn setup<P: Pin>(pin: &mut Flex<P>, syst: &mut SYST) -> Result<(), getrandom::Error> {
    let r = CapRng::new(pin, syst)?;

    critical_section::with(|cs| {
        let mut rng = RNG.borrow_ref_mut(cs);
        rng.insert(r);
    });
    Ok(())
}

// TODO: this is another impl of chacha20, can it use chacha20 crate instead? Is the size much?
// TODO: have some kind of fast erasure RNG instead?
struct CapRng(ChaCha20Rng);

impl CapRng {
    /// Disables interrupts, will block. Call this at startup before any time-sensitive code
    /// `syst` will be modified.
    fn new<P: Pin>(pin: &mut Flex<P>, syst: &mut SYST) -> Result<Self, getrandom::Error> {
        let mut h = Sha256::new();
        for _ in 0..NUM_LOOPS {
            let del = Self::sample(pin, syst)?;
            h.update(del.to_be_bytes());
        }
        let seed: [u8; 32] = h.finalize().into();
        Ok(Self(ChaCha20Rng::from_seed(seed)))
    }

    /// Returns individual samples from the capacitor discharge. Not normally used,
    /// can be used to analyse the random number generation.
    pub fn sample<P: Pin>(pin: &mut Flex<P>, syst: &mut SYST) -> Result<u32, getrandom::Error> {
        syst.set_clock_source(cortex_m::peripheral::syst::SystClkSource::Core);
        if SYST::get_ticks_per_10ms() != MIN_10MS_CYCLES {
            error!("System clock not 125mhz");
            return Err(error())
        }

        // This sequence is necessary
        syst.set_reload(0x00ffffff);
        syst.clear_current();
        syst.enable_counter();

        pin.set_pull(Pull::Down);
        pin.set_as_output();
        pin.set_high();
        // allow capacitor to charge
        cortex_m::asm::delay(CHARGE_CYCLES);

        let (t1, t2, wr) = critical_section::with(|_cs| {
            syst.clear_current();
            let t1 = SYST::get_current();
            // allow to drain through pull down
            pin.set_as_input();
            while pin.is_high() {}

            let t2 = SYST::get_current();
            let wr = syst.has_wrapped();
            (t1, t2, wr)
        });
        // Don't leave enabled after completion.
        syst.enable_counter();

        if wr {
            error!("Timer wrapper, capacitor too large?");
            return Err(error())
        }

        let del = t1 - t2;
        if del > MIN_DEL {
            Ok(del)
        } else {
            error!("Capacitor seems too small?");
            Err(error())
        }
    }
}
